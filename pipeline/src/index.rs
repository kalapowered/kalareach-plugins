//! Building the catalogue index.
//!
//! The index is the complete signed metadata snapshot a host synchronises. It carries what a host
//! needs to search, match and decide, and nothing it would only need after deciding. Each entry
//! names the executable builds its release is qualified against ([`crate::builds`]), which is
//! where a host takes an executable's version from.
//!
//! The build is deterministic. Entries are ordered by publisher, plugin name and version, object
//! keys are sorted and the rendering is fixed, so the same packages always produce the same bytes.
//! Without that, a signature over an index would only prove which run produced it.

use kr_plugin_sdk::catalogue::{CatalogueIndex, INDEX_VERSION, IndexEntry, RevocationRecord};
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::scalars::TimestampMs;

use kr_plugin_sdk::scalars::Nullable;

use crate::packages::{Repository, Revocations};
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
                entry.builds = repository
                    .builds
                    .for_release(entry.plugin_id.as_str(), &entry.version.to_string());
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
