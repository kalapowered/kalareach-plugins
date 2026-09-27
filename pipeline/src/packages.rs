//! Reading and validating the packages in the repository.
//!
//! Every package under `plugins/<publisher>/<plugin-id>/` is validated with `kr-plugin-sdk`, which
//! is the same code a host runs before it trusts a package. Running the host's validator rather
//! than a pipeline-specific check is the point: a package this repository publishes is a package a
//! host accepts, and the two cannot drift apart.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kr_plugin_sdk::catalogue::PublisherRecord;
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::ids::PublisherId;
use kr_plugin_sdk::package::{MANIFEST_FILE, Package};
use kr_plugin_sdk::validate::{Report, validate_package_directory};
use serde::{Deserialize, Serialize};

use crate::{Error, Result, read_json};

/// One publisher's record on disk.
///
/// The record carries a format version the index entry does not need, so it is read here and
/// narrowed to the [`PublisherRecord`] the index publishes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PublisherFile {
    /// The record format version.
    pub record_version: u32,
    /// The publisher identifier.
    pub id: PublisherId,
    /// The name a person reads.
    pub display_name: kr_plugin_sdk::text::Label,
    /// Where the publisher's own source and contact details live.
    pub homepage: String,
    /// Whether this publisher ships with KalaReach.
    pub first_party: bool,
    /// How to reach the publisher.
    pub contact: String,
}

impl PublisherFile {
    /// The record format version this pipeline reads.
    pub const CURRENT_VERSION: u32 = 1;

    /// Narrows the file to the record the index publishes.
    #[must_use]
    pub fn to_record(&self) -> PublisherRecord {
        PublisherRecord {
            id: self.id.clone(),
            display_name: self.display_name.clone(),
            homepage: self.homepage.clone(),
            first_party: self.first_party,
        }
    }
}

/// One validated package and where it came from.
#[derive(Debug)]
pub struct LoadedPackage {
    /// The package directory.
    pub directory: PathBuf,
    /// The package's path relative to the repository root, as the catalogue addresses it.
    pub relative: String,
    /// The parsed package.
    pub package: Package,
    /// The digest of `plugin.json`.
    pub manifest_digest: PayloadDigest,
    /// The exact length of `plugin.json`.
    pub manifest_size_bytes: u64,
}

/// One withdrawn release, keyed to the exact package it withdraws.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WithdrawalFile {
    /// The record format version.
    pub record_version: u32,
    /// The publisher.
    pub publisher_id: PublisherId,
    /// The plugin name under that publisher.
    pub plugin_name: kr_plugin_sdk::ids::PluginName,
    /// The version withdrawn.
    pub version: kr_plugin_sdk::version::PackageVersion,
    /// The manifest digest of the release withdrawn.
    ///
    /// A revocation names the bytes it withdraws. Without the digest it would apply to whatever is
    /// under that version now, which is the opposite of what a revocation is for.
    pub manifest_digest: PayloadDigest,
    /// What the index carries.
    pub record: kr_plugin_sdk::catalogue::RevocationRecord,
}

impl WithdrawalFile {
    /// The record format version this pipeline reads.
    pub const CURRENT_VERSION: u32 = 1;
}

/// The withdrawn releases, by publisher, plugin name and version.
pub type Revocations = BTreeMap<(String, String, String), WithdrawalFile>;

/// Everything the repository holds.
#[derive(Debug, Default)]
pub struct Repository {
    /// The publishers, by identifier.
    pub publishers: BTreeMap<String, PublisherFile>,
    /// The packages, ordered by their relative path.
    pub packages: Vec<LoadedPackage>,
    /// The withdrawn releases.
    pub revocations: Revocations,
    /// The executable builds each release is qualified against, and the pinned builds left out.
    pub builds: crate::builds::Builds,
}

/// One package that did not validate.
#[derive(Debug)]
pub struct Rejected {
    /// The package's path relative to the repository root.
    pub relative: String,
    /// Everything wrong with it.
    pub report: Report,
}

/// The outcome of loading the repository.
#[derive(Debug)]
pub struct Loaded {
    /// The packages that validated.
    pub repository: Repository,
    /// The packages that did not.
    pub rejected: Vec<Rejected>,
}

/// Loads and validates every publisher record and package in the repository, and decides which
/// pinned builds each release names.
///
/// # Errors
///
/// Returns an error when the layout is wrong, a document cannot be read, two packages claim the
/// same identity, or the build list or a qualification record is refused ([`crate::builds::load`]). A package that merely fails validation is reported in [`Loaded::rejected`]
/// rather than ending the run, so one command reports every defect in the repository.
pub fn load(root: &Path) -> Result<Loaded> {
    let mut repository = Repository::default();
    let mut rejected = Vec::new();

    // The layout directories themselves are checked, not only their contents. A link in place of
    // `plugins/` would put a whole tree from outside the repository into a signed release.
    for name in ["plugins", "publishers", "revocations"] {
        let path = root.join(name);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_symlink() => {
                return Err(Error::Layout {
                    detail: format!(
                        "{} is a link; the catalogue holds the files it publishes",
                        path.display()
                    ),
                });
            }
            _ => {}
        }
    }

    for entry in sorted_dir(&root.join("publishers"))? {
        if entry.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let file: PublisherFile = read_json(&entry)?;
        if file.record_version != PublisherFile::CURRENT_VERSION {
            return Err(Error::Layout {
                detail: format!(
                    "{}: publisher record version {} is not version {}",
                    entry.display(),
                    file.record_version,
                    PublisherFile::CURRENT_VERSION
                ),
            });
        }
        let stem = entry
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem != file.id.as_str() {
            return Err(Error::Layout {
                detail: format!(
                    "{} declares the publisher {}; the file name and the identifier must match",
                    entry.display(),
                    file.id
                ),
            });
        }
        repository.publishers.insert(file.id.to_string(), file);
    }

    let revocations_dir = root.join("revocations");
    if revocations_dir.is_dir() {
        for entry in sorted_dir(&revocations_dir)? {
            if entry.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let file: WithdrawalFile = read_json(&entry)?;
            if file.record_version != WithdrawalFile::CURRENT_VERSION {
                return Err(Error::Layout {
                    detail: format!(
                        "{}: withdrawal record version {} is not version {}",
                        entry.display(),
                        file.record_version,
                        WithdrawalFile::CURRENT_VERSION
                    ),
                });
            }
            let key = (
                file.publisher_id.to_string(),
                file.plugin_name.to_string(),
                file.version.to_string(),
            );
            if repository.revocations.insert(key, file).is_some() {
                return Err(Error::Layout {
                    detail: format!("{} withdraws a release twice", entry.display()),
                });
            }
        }
    }

    let mut seen: BTreeMap<(String, String), String> = BTreeMap::new();
    for publisher_dir in sorted_dir(&root.join("plugins"))? {
        if !is_real_dir(&publisher_dir)? {
            continue;
        }
        let publisher = publisher_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        if !repository.publishers.contains_key(&publisher) {
            return Err(Error::Layout {
                detail: format!("plugins/{publisher} has no record in publishers/{publisher}.json"),
            });
        }
        for package_dir in sorted_dir(&publisher_dir)? {
            if !is_real_dir(&package_dir)? {
                continue;
            }
            let name = package_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_owned();
            let relative = format!("plugins/{publisher}/{name}");
            let validated = validate_package_directory(&package_dir);
            let Some(package) = validated.package.filter(|_| validated.report.is_valid()) else {
                rejected.push(Rejected {
                    relative,
                    report: validated.report,
                });
                continue;
            };
            if package.manifest.publisher_id.as_str() != publisher
                || package.manifest.plugin_name.as_str() != name
            {
                return Err(Error::Layout {
                    detail: format!(
                        "{relative} declares {}, which does not match its directory",
                        package.manifest.plugin_id()
                    ),
                });
            }
            check_source_pin(&relative, &package.manifest.source.revision)?;
            let key = (
                package.manifest.plugin_id().to_string(),
                package.manifest.version.to_string(),
            );
            if let Some(first) = seen.get(&key) {
                return Err(Error::DuplicatePackage {
                    plugin_id: key.0,
                    version: key.1,
                    first: first.clone(),
                    second: relative,
                });
            }
            seen.insert(key, relative.clone());
            // The digest and the length come from the validator's own scan rather than from a
            // second read. Reading the file again would let the index combine one document's
            // fields with another document's digest.
            let manifest_path =
                kr_plugin_sdk::paths::PackagePath::new(MANIFEST_FILE).map_err(|rejection| {
                    Error::Layout {
                        detail: rejection.to_string(),
                    }
                })?;
            let Some(manifest_file) = package.file(&manifest_path).cloned() else {
                return Err(Error::Layout {
                    detail: format!("{relative} holds no {MANIFEST_FILE}"),
                });
            };
            repository.packages.push(LoadedPackage {
                directory: package_dir,
                relative,
                package,
                manifest_digest: manifest_file.digest,
                manifest_size_bytes: manifest_file.size_bytes,
            });
        }
    }

    for ((publisher, plugin, version), withdrawal) in &repository.revocations {
        let Some(release) = repository.packages.iter().find(|package| {
            package.package.manifest.publisher_id.as_str() == publisher
                && package.package.manifest.plugin_name.as_str() == plugin
                && package.package.manifest.version.to_string() == *version
        }) else {
            return Err(Error::Layout {
                detail: format!(
                    "a withdrawal names {publisher}/{plugin} {version}, which this repository does not publish"
                ),
            });
        };
        // The digest is what makes a withdrawal apply to the bytes it was written for. A mistyped
        // one would otherwise produce a build that quietly published the release it withdraws.
        if withdrawal.manifest_digest != release.manifest_digest {
            return Err(Error::Layout {
                detail: format!(
                    "the withdrawal of {publisher}/{plugin} {version} names the manifest digest {} \
                     and the release has {}",
                    withdrawal.manifest_digest, release.manifest_digest
                ),
            });
        }
    }

    repository
        .packages
        .sort_by(|left, right| left.relative.cmp(&right.relative));
    // A build is named for a release this repository publishes, so the builds are read once every
    // package has validated. A repository with a package that did not is refused by its caller, with
    // that package's report.
    if rejected.is_empty() {
        repository.builds = crate::builds::load(root, &repository.packages)?;
    }
    Ok(Loaded {
        repository,
        rejected,
    })
}

/// Lists a directory's entries in a stable order, refusing links.
///
/// A symbolic link in the layout, whether it stands in for a publisher record, a publisher
/// directory or a package directory, would put files from outside the repository into a signed
/// release. The layout is checked with `symlink_metadata`, which reports the link rather than what
/// it points at.
/// Checks that a package's source revision names something that cannot move.
///
/// Section 4 has a reviewed catalogue entry pin the publisher, the source revision and the released
/// package digest. Only a commit identifier is a pin: a branch names whatever it points at now, and
/// a tag can be moved to point somewhere else. The release process resolves whatever reference a
/// publisher works with into the commit identifier that goes in the manifest.
fn check_source_pin(relative: &str, revision: &str) -> Result<()> {
    let is_commit = revision.len() == 40
        && revision
            .chars()
            .all(|character| character.is_ascii_digit() || matches!(character, 'a'..='f'));
    if is_commit {
        return Ok(());
    }
    Err(Error::Layout {
        detail: format!(
            "{relative} pins its source at {revision:?}; a source pin is a 40-character commit \
             identifier, because a branch and a tag both move"
        ),
    })
}

fn sorted_dir(directory: &Path) -> Result<Vec<PathBuf>> {
    let entries = std::fs::read_dir(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path).map_err(|source| Error::Io {
            path: path.clone(),
            source,
        })?;
        if metadata.is_symlink() {
            return Err(Error::Layout {
                detail: format!(
                    "{} is a link; the catalogue holds the files it publishes",
                    path.display()
                ),
            });
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

/// Returns true when a path is a directory and not a link to one.
fn is_real_dir(path: &Path) -> Result<bool> {
    let metadata = std::fs::symlink_metadata(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(metadata.is_dir())
}
