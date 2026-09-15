//! Producing and verifying the TUF repository.
//!
//! A generation is written as one immutable directory:
//!
//! ```text
//! <generation>/
//!   root.json          the trust root a host ships or adopts out of band
//!   metadata/          root, timestamp, snapshot and targets metadata
//!   targets/           index.json and every package payload, by target name
//! ```
//!
//! Target names carry the package identity and version, so one repository can hold several
//! versions of the same package and a host can pin one of them:
//!
//! ```text
//! index.json
//! packages/kalareach/example-declarative/0.1.0/plugin.json
//! packages/kalareach/example-declarative/0.1.0/presentation.json
//! ```
//!
//! Verification uses the `tough` client against the trust root, which is the same client a host
//! uses. A tampered target fails the digest check the targets metadata pins, not a check this
//! pipeline invented.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kr_plugin_sdk::catalogue::CatalogueIndex;
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::package::MANIFEST_FILE;
use tough::editor::RepositoryEditor;
use tough::schema::Target;
use tough::{
    ExpirationEnforcement, FilesystemTransport, IntoVec as _, RepositoryLoader, TargetName,
};

use crate::keys::SigningDirectory;
use crate::packages::Repository;
use crate::{Error, Result, read, write};

/// The target name of the catalogue index.
pub const INDEX_TARGET: &str = "index.json";

/// The metadata directory inside a generation.
pub const METADATA_DIR: &str = "metadata";

/// The targets directory inside a generation.
pub const TARGETS_DIR: &str = "targets";

/// How long each role's metadata is valid for.
#[derive(Clone, Copy, Debug)]
pub struct Expiries {
    /// When the targets metadata expires.
    pub targets: jiff::Timestamp,
    /// When the snapshot metadata expires.
    pub snapshot: jiff::Timestamp,
    /// When the timestamp metadata expires.
    pub timestamp: jiff::Timestamp,
}

/// One staged target: the name the metadata pins it under, and where its bytes are.
#[derive(Clone, Debug)]
pub struct StagedTarget {
    /// The target name, which is the path a host fetches it by.
    pub name: String,
    /// Where the bytes were written.
    pub path: PathBuf,
}

/// Lays out one generation's targets on disk.
///
/// The staged tree is what the editor signs and what the generation ships, so the digests in the
/// targets metadata are digests of exactly these files.
///
/// Each target is named by its whole path rather than its file name. Every package carries a
/// `plugin.json`, so names taken from file names alone would collapse seven packages into one
/// target.
///
/// # Errors
///
/// Returns an error when a package payload cannot be read or a target cannot be written.
pub fn stage_targets(
    repository: &Repository,
    index: &CatalogueIndex,
    targets_dir: &Path,
) -> Result<Vec<StagedTarget>> {
    let mut staged = Vec::new();

    let rendered = index.canonical_json().map_err(|source| Error::Json {
        path: PathBuf::from(INDEX_TARGET),
        source,
    })?;
    let index_path = targets_dir.join(INDEX_TARGET);
    write(&index_path, rendered.as_bytes())?;
    staged.push(StagedTarget {
        name: INDEX_TARGET.to_owned(),
        path: index_path,
    });

    for loaded in &repository.packages {
        let prefix = format!(
            "packages/{}/{}/{}",
            loaded.package.manifest.publisher_id,
            loaded.package.manifest.plugin_name,
            loaded.package.manifest.version
        );
        let mut files: Vec<String> = loaded
            .package
            .manifest
            .payloads
            .iter()
            .map(|payload| payload.path.to_string())
            .collect();
        files.push(MANIFEST_FILE.to_owned());
        files.sort();
        for relative in files {
            let source = loaded.directory.join(&relative);
            let destination = targets_dir.join(&prefix).join(&relative);
            write(&destination, &read(&source)?)?;
            staged.push(StagedTarget {
                name: format!("{prefix}/{relative}"),
                path: destination,
            });
        }
    }
    staged.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(staged)
}

/// Builds and signs one generation into `out_dir`.
///
/// # Errors
///
/// Returns an error when a target cannot be staged, a key cannot be read, or the metadata cannot
/// be signed or written.
pub async fn build(
    repository: &Repository,
    index: &CatalogueIndex,
    signing: &SigningDirectory,
    expiries: Expiries,
    out_dir: &Path,
    replace: bool,
) -> Result<BuildOutcome> {
    // A generation is immutable. Writing into a directory that already holds one would leave a
    // mixture of two if anything failed part-way, and the metadata would then describe targets
    // that are not there.
    if out_dir.exists() {
        let empty = std::fs::read_dir(out_dir)
            .map_err(|source| Error::Io {
                path: out_dir.to_path_buf(),
                source,
            })?
            .next()
            .is_none();
        if !empty {
            if !replace {
                return Err(Error::Layout {
                    detail: format!(
                        "{} already holds a generation; a generation is written once",
                        out_dir.display()
                    ),
                });
            }
            std::fs::remove_dir_all(out_dir).map_err(|source| Error::Io {
                path: out_dir.to_path_buf(),
                source,
            })?;
        }
    }

    let targets_dir = out_dir.join(TARGETS_DIR);
    let metadata_dir = out_dir.join(METADATA_DIR);
    let staged = stage_targets(repository, index, &targets_dir)?;
    check_staged_against_validation(
        repository,
        &staged,
        report_paths(index)?,
        report_index(index)?,
    )?;

    let root_source = signing.root_path();
    let root_bytes = read(&root_source)?;
    write(&out_dir.join("root.json"), &root_bytes)?;

    // Every role's version is the generation. A client rejects metadata whose version is lower
    // than the one it already trusts, and versions that stayed at one would make every generation
    // look like the same one, which is how a rollback goes unnoticed.
    let version = generation_version(index)?;
    let mut editor = RepositoryEditor::new(&root_source).await?;
    editor
        .targets_version(version)?
        .targets_expires(expiries.targets)?
        .snapshot_version(version)
        .snapshot_expires(expiries.snapshot)
        .timestamp_version(version)
        .timestamp_expires(expiries.timestamp);
    for target in &staged {
        let built = Target::from_path(&target.path).await.map_err(Error::from)?;
        editor.add_target(target.name.as_str(), built)?;
    }

    let signed = editor.sign(&signing.key_sources()?).await?;
    signed.write(&metadata_dir).await?;
    write(&metadata_dir.join("root.json"), &root_bytes)?;

    Ok(BuildOutcome {
        target_count: staged.len(),
        metadata_dir,
        targets_dir,
    })
}

/// Returns the metadata version for one generation.
///
/// # Errors
///
/// Returns [`Error::Layout`] for generation zero, which TUF does not have a version for.
fn generation_version(index: &CatalogueIndex) -> Result<std::num::NonZeroU64> {
    std::num::NonZeroU64::new(index.generation.get()).ok_or_else(|| Error::Layout {
        detail: "a generation starts at 1".to_owned(),
    })
}

/// Returns the canonical index bytes, for comparing what was staged against what was validated.
fn report_index(index: &CatalogueIndex) -> Result<Vec<u8>> {
    index
        .canonical_json()
        .map(String::into_bytes)
        .map_err(|source| Error::Json {
            path: PathBuf::from(INDEX_TARGET),
            source,
        })
}

/// Returns the digest and length the index declares for every package payload.
fn report_paths(index: &CatalogueIndex) -> Result<Vec<(String, PayloadDigest, u64)>> {
    let mut declared = Vec::new();
    for entry in &index.entries {
        let prefix = format!(
            "packages/{}/{}/{}",
            entry.publisher_id, entry.plugin_name, entry.version
        );
        declared.push((
            format!("{prefix}/{}", kr_plugin_sdk::package::MANIFEST_FILE),
            entry.manifest_digest,
            entry.manifest_size_bytes.get(),
        ));
        for payload in &entry.payloads {
            declared.push((
                format!("{prefix}/{}", payload.path),
                payload.digest,
                payload.size_bytes.get(),
            ));
        }
    }
    Ok(declared)
}

/// Checks that every staged byte is a byte validation saw.
///
/// Staging reopens the package files, so a file edited between validation and signing would
/// otherwise be signed without ever being checked. Every staged target is hashed again and
/// compared with what the index declares, and the index itself is compared with its canonical
/// rendering.
fn check_staged_against_validation(
    _repository: &Repository,
    staged: &[StagedTarget],
    declared: Vec<(String, PayloadDigest, u64)>,
    index_bytes: Vec<u8>,
) -> Result<()> {
    for target in staged {
        let bytes = read(&target.path)?;
        if target.name == INDEX_TARGET {
            if bytes != index_bytes {
                return Err(Error::Layout {
                    detail: "the staged index is not the index that was built".to_owned(),
                });
            }
            continue;
        }
        let Some((_, digest, size)) = declared.iter().find(|(name, _, _)| name == &target.name)
        else {
            return Err(Error::Layout {
                detail: format!(
                    "{} was staged and the index does not declare it",
                    target.name
                ),
            });
        };
        if bytes.len() as u64 != *size || &PayloadDigest::of(&bytes) != digest {
            return Err(Error::Layout {
                detail: format!(
                    "{} changed between validation and signing; it was not signed",
                    target.name
                ),
            });
        }
    }
    for (name, _, _) in &declared {
        if !staged.iter().any(|target| &target.name == name) {
            return Err(Error::Layout {
                detail: format!("the index declares {name} and nothing staged it"),
            });
        }
    }
    Ok(())
}

/// What a build produced.
#[derive(Clone, Debug)]
pub struct BuildOutcome {
    /// How many targets the generation carries.
    pub target_count: usize,
    /// Where the metadata was written.
    pub metadata_dir: PathBuf,
    /// Where the targets were written.
    pub targets_dir: PathBuf,
}

/// What verification found.
#[derive(Clone, Debug)]
pub struct Verified {
    /// How many targets the metadata pins.
    pub target_count: usize,
    /// The index the generation carries.
    pub index: CatalogueIndex,
}

/// Verifies a generation with the `tough` client and reads its index back.
///
/// `enforce_expiry` is the client's ordinary behaviour. Passing false reads a generation whose
/// metadata has expired, which a host never does: expired metadata blocks new generations, and
/// only already-installed pinned packages stay usable.
///
/// # Errors
///
/// Returns an error when the metadata does not verify against the trust root, a target's digest
/// does not match, or the index cannot be read.
pub async fn verify(generation_dir: &Path, enforce_expiry: bool) -> Result<Verified> {
    let root_bytes = read(&generation_dir.join("root.json"))?;
    let absolute = std::fs::canonicalize(generation_dir).map_err(|source| Error::Io {
        path: generation_dir.to_path_buf(),
        source,
    })?;
    let metadata_url = directory_url(&absolute.join(METADATA_DIR))?;
    let targets_url = directory_url(&absolute.join(TARGETS_DIR))?;

    let repository = RepositoryLoader::new(&root_bytes, metadata_url, targets_url)
        .transport(FilesystemTransport)
        .expiration_enforcement(if enforce_expiry {
            ExpirationEnforcement::Safe
        } else {
            ExpirationEnforcement::Unsafe
        })
        .load()
        .await?;

    let names: Vec<TargetName> = repository
        .targets()
        .signed
        .targets
        .keys()
        .cloned()
        .collect();
    let target_count = names.len();

    // Every target is read through the client, which checks each one's digest and length as it
    // streams. Reading only the index would verify the metadata and leave a replaced, truncated or
    // absent package payload undiscovered until a host fetched it.
    let mut contents: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for name in names {
        let bytes = repository
            .read_target(&name)
            .await?
            .ok_or_else(|| Error::Layout {
                detail: format!(
                    "the metadata pins {} and the generation does not carry it",
                    name.raw()
                ),
            })?
            .into_vec()
            .await?;
        contents.insert(name.raw().to_owned(), bytes);
    }

    let index_bytes = contents.get(INDEX_TARGET).ok_or_else(|| Error::Layout {
        detail: format!("the generation does not carry {INDEX_TARGET}"),
    })?;
    let index: CatalogueIndex =
        serde_json::from_slice(index_bytes).map_err(|source| Error::Json {
            path: PathBuf::from(INDEX_TARGET),
            source,
        })?;

    // The metadata proves the targets are the bytes it signed. The index says which bytes each
    // package consists of. Comparing the two is what makes a target name mean one package's file
    // rather than whatever happens to be under that name.
    let declared = report_paths(&index)?;
    for (name, digest, size) in &declared {
        let Some(bytes) = contents.get(name) else {
            return Err(Error::Layout {
                detail: format!("the index declares {name} and the generation does not carry it"),
            });
        };
        if bytes.len() as u64 != *size || &PayloadDigest::of(bytes) != digest {
            return Err(Error::Layout {
                detail: format!("{name} is not the payload the index declares"),
            });
        }
    }
    for name in contents.keys() {
        if name != INDEX_TARGET && !declared.iter().any(|(declared, _, _)| declared == name) {
            return Err(Error::Layout {
                detail: format!("the generation carries {name} and the index does not declare it"),
            });
        }
    }

    Ok(Verified {
        target_count,
        index,
    })
}

fn directory_url(path: &Path) -> Result<url::Url> {
    url::Url::from_directory_path(path).map_err(|()| Error::Layout {
        detail: format!("{} is not an absolute path", path.display()),
    })
}
