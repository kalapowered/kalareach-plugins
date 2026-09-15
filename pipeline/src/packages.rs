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

use crate::{Error, Result, read, read_json};

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

/// Everything the repository holds.
#[derive(Debug, Default)]
pub struct Repository {
    /// The publishers, by identifier.
    pub publishers: BTreeMap<String, PublisherFile>,
    /// The packages, ordered by their relative path.
    pub packages: Vec<LoadedPackage>,
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

/// Loads and validates every publisher record and package in the repository.
///
/// # Errors
///
/// Returns an error when the layout is wrong, a document cannot be read, or two packages claim the
/// same identity. A package that merely fails validation is reported in [`Loaded::rejected`]
/// rather than ending the run, so one command reports every defect in the repository.
pub fn load(root: &Path) -> Result<Loaded> {
    let mut repository = Repository::default();
    let mut rejected = Vec::new();

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

    let mut seen: BTreeMap<(String, String), String> = BTreeMap::new();
    for publisher_dir in sorted_dir(&root.join("plugins"))? {
        if !publisher_dir.is_dir() {
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
            if !package_dir.is_dir() {
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
            let manifest_bytes = read(&package_dir.join(MANIFEST_FILE))?;
            repository.packages.push(LoadedPackage {
                directory: package_dir,
                relative,
                package,
                manifest_digest: PayloadDigest::of(&manifest_bytes),
                manifest_size_bytes: manifest_bytes.len() as u64,
            });
        }
    }

    repository
        .packages
        .sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(Loaded {
        repository,
        rejected,
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
        paths.push(entry.path());
    }
    paths.sort();
    Ok(paths)
}
