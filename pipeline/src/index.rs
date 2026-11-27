//! Building the catalogue index.
//!
//! The index is the complete signed metadata snapshot a host synchronises. It carries what a host
//! needs to search, match and decide, and nothing it would only need after deciding. Each entry
//! names the executable builds its release is qualified against ([`crate::builds`]), which is
//! where a host takes an executable's version from, and carries the qualification results that
//! state what the record of each build shows, one for each capability its release declares.
//!
//! The build is deterministic. Entries are ordered by publisher, plugin name and version, object
//! keys are sorted and the rendering is fixed, so the same packages always produce the same bytes.
//! Without that, a signature over an index would only prove which run produced it.

use kr_plugin_sdk::capability::CapabilityState;
use kr_plugin_sdk::catalogue::{CatalogueIndex, INDEX_VERSION, IndexEntry, RevocationRecord};
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::scalars::TimestampMs;

use kr_plugin_sdk::scalars::Nullable;

use crate::builds;
use crate::packages::{Repository, Revocations};
use crate::records;
use crate::{Error, Result};

/// Builds the index for one generation.
///
/// `produced_at` is supplied rather than read from the clock so a caller can reproduce a
/// generation byte for byte.
#[must_use]
pub fn build(
    repository: &Repository,
    generation: RepositoryGeneration,
    produced_at: TimestampMs,
) -> CatalogueIndex {
    let revocations: &Revocations = &repository.revocations;
    let mut index = CatalogueIndex {
        index_version: INDEX_VERSION,
        generation,
        produced_at,
        publishers: repository
            .publishers
            .values()
            .map(super::packages::PublisherFile::to_record)
            .collect(),
        entries: repository
            .packages
            .iter()
            .map(|loaded| {
                let mut entry = IndexEntry::from_manifest(
                    &loaded.package.manifest,
                    loaded.manifest_digest,
                    loaded.manifest_size_bytes,
                );
                entry.revocation = Nullable(revocation_for(revocations, &entry));
                let release = entry.version.to_string();
                entry.builds = repository
                    .builds
                    .for_release(entry.plugin_id.as_str(), &release);
                entry.qualification = repository
                    .builds
                    .results_for(entry.plugin_id.as_str(), &release);
                entry
            })
            .collect(),
    };
    index.sort();
    index
}

/// Checks what every entry says about its builds, as a host does when it verifies a generation: no
/// more than the SDK's bound, each on a platform the release supports, and one version for each
/// executable.
///
/// # Errors
///
/// Returns [`Error::Builds`] naming the first entry that breaks a rule, in the SDK's words.
pub fn check_builds(index: &CatalogueIndex) -> Result<()> {
    for entry in &index.entries {
        entry.check_builds().map_err(|source| Error::Builds {
            plugin_id: entry.plugin_id.to_string(),
            version: entry.version.to_string(),
            source: Box::new(source),
        })?;
    }
    Ok(())
}

/// Checks that what every entry carries as qualification results agrees with the builds it names,
/// as a host that reads them does: results only with builds, and builds only with results.
///
/// An entry that names builds carries, for each build in the order the entry lists them, one result
/// for each capability the entry declares, in the order of their identifiers: the identifier the
/// pipeline gives the capability, the release's version, the build's application and version as the
/// subject, the state of a version that was qualified, the signed record as the source, and one of
/// the statements the pipeline writes ([`builds::statements`]) for the platform the index gives the
/// build. An entry that names builds and declares no capability is refused, since no result could
/// state them. An entry that names no build carries no result, and in a candidate generation, whose
/// builds were named without a record, no entry carries one.
///
/// The digest each result carries is of the record file, which the index does not hold, so it is
/// checked for its form alone.
///
/// # Errors
///
/// Returns [`Error::Results`] naming the first entry whose results disagree with its builds.
pub fn check_results(index: &CatalogueIndex, candidate: bool) -> Result<()> {
    for entry in &index.entries {
        results_agree(entry, candidate).map_err(|detail| Error::Results {
            plugin_id: entry.plugin_id.to_string(),
            version: entry.version.to_string(),
            detail,
        })?;
    }
    Ok(())
}

/// What one entry's results break, in words.
fn results_agree(entry: &IndexEntry, candidate: bool) -> std::result::Result<(), String> {
    let carried = entry.qualification.len();
    if candidate {
        return if carried == 0 {
            Ok(())
        } else {
            Err(format!(
                "the generation is a candidate, so its entries carry no qualification result, and \
                 this one carries {carried}"
            ))
        };
    }
    if entry.builds.is_empty() {
        return if carried == 0 {
            Ok(())
        } else {
            Err(format!(
                "the entry names no build, so it carries no qualification result, and it carries \
                 {carried}"
            ))
        };
    }
    let named = counted(entry.builds.len(), "build", "builds");
    if entry.capabilities.is_empty() {
        return Err(format!(
            "the entry names {named} and declares no capability, so no qualification result can \
             state what a record shows of its builds"
        ));
    }
    let expected = entry.builds.len() * entry.capabilities.len();
    if carried != expected {
        return Err(format!(
            "the entry names {named} and declares {}, so it carries {}, and it carries {carried}",
            counted(entry.capabilities.len(), "capability", "capabilities"),
            counted(expected, "qualification result", "qualification results"),
        ));
    }
    let mut ids: Vec<String> = entry
        .capabilities
        .iter()
        .map(|request| builds::capability_id(request.capability))
        .collect();
    ids.sort();
    let mut results = entry.qualification.iter().enumerate();
    for build in &entry.builds {
        let platform = records::platform_name(build.os, build.architecture);
        let statements = builds::statements(&platform);
        let subject = builds::subject(build);
        for id in &ids {
            let (position, result) = results.next().expect("the count was checked");
            let wrong = |reason: String| format!("qualification result {}: {reason}", position + 1);
            result
                .validate()
                .map_err(|source| wrong(source.to_string()))?;
            if result.capability_id.as_str() != id {
                return Err(wrong(format!(
                    "the capability is {}, and this position is for {id}",
                    result.capability_id.as_str()
                )));
            }
            if result.capability_version != entry.version {
                return Err(wrong(format!(
                    "the capability version is {}, and the release is {}",
                    result.capability_version, entry.version
                )));
            }
            if result.subject.as_str() != subject {
                return Err(wrong(format!(
                    "the subject is {:?}, and this position's build is {subject:?}",
                    result.subject.as_str()
                )));
            }
            if result.state != CapabilityState::VersionQualified {
                return Err(wrong(format!(
                    "the state is {:?}, and a qualified build's result states VersionQualified",
                    result.state
                )));
            }
            if !statements
                .iter()
                .any(|written| written == result.statement.as_str())
            {
                return Err(wrong(format!(
                    "the statement {:?} is not one the pipeline writes for a build on {platform}",
                    result.statement.as_str()
                )));
            }
        }
    }
    Ok(())
}

/// A count and what it counts, in the singular for one.
fn counted(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

/// Returns the revocation record for one entry, where the repository carries one.
///
/// A revocation is a committed input like the package it withdraws, keyed to the exact release and
/// its manifest digest. Keeping it in the repository is what makes a rebuild produce the same
/// index: a record that lived only in a published generation would be lost the next time one was
/// built.
fn revocation_for(revocations: &Revocations, entry: &IndexEntry) -> Option<RevocationRecord> {
    let key = (
        entry.publisher_id.to_string(),
        entry.plugin_name.to_string(),
        entry.version.to_string(),
    );
    // The digest was matched when the repository loaded, so a record that is here is a record for
    // these exact bytes.
    revocations
        .get(&key)
        .map(|withdrawal| withdrawal.record.clone())
}
