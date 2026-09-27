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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use futures::StreamExt as _;
use kr_plugin_sdk::catalogue::CatalogueIndex;
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::package::MANIFEST_FILE;
use sha2::{Digest as _, Sha256};
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
/// Returns an error when an entry names builds an index may not carry, a target cannot be staged,
/// a key cannot be read, or the metadata cannot be signed or written.
pub async fn build(
    repository: &Repository,
    index: &CatalogueIndex,
    signing: &SigningDirectory,
    expiries: Expiries,
    out_dir: &Path,
    replace: bool,
) -> Result<BuildOutcome> {
    // Everything that can be checked is checked before anything is written or removed: what the
    // entries say about their builds, the generation number, the destination, and the keys. A build
    // that fails afterwards leaves the destination as it found it.
    crate::index::check_builds(index)?;
    let version = generation_version(index)?;
    let existing = describe_destination(out_dir)?;
    if existing == Destination::Occupied && !replace {
        return Err(Error::Layout {
            detail: format!(
                "{} already holds a generation; a generation is written once, and --replace says to \
                 write over this one",
                out_dir.display()
            ),
        });
    }
    let key_sources = signing.key_sources()?;
    let root_bytes = read(&signing.root_path())?;

    // The generation is assembled beside its destination and moved into place once it verifies, so
    // the destination is never a half-written generation and never a mixture of two. The staging
    // directory is created rather than emptied: something already there was left by an interrupted
    // build, and deleting it would be deleting evidence somebody may want.
    let staging = staging_dir(out_dir);
    if let Some(parent) = staging.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::create_dir(&staging).map_err(|source| Error::Io {
        path: staging.clone(),
        source,
    })?;
    let targets_dir = staging.join(TARGETS_DIR);
    let metadata_dir = staging.join(METADATA_DIR);
    let staged = stage_targets(repository, index, &targets_dir)?;
    let declared = report_paths(index)?;
    let index_bytes = report_index(index)?;
    check_staged_against_validation(&staged, &declared, &index_bytes)?;

    write(&staging.join("root.json"), &root_bytes)?;

    let mut editor = RepositoryEditor::new(&staging.join("root.json")).await?;
    editor
        .targets_version(version)?
        .targets_expires(expiries.targets)?
        .snapshot_version(version)
        .snapshot_expires(expiries.snapshot)
        .timestamp_version(version)
        .timestamp_expires(expiries.timestamp);
    for target in &staged {
        let built = Target::from_path(&target.path).await.map_err(Error::from)?;
        check_target_against_validation(&target.name, &built, &declared, &index_bytes)?;
        editor.add_target(target.name.as_str(), built)?;
    }

    let signed = editor.sign(&key_sources).await?;
    signed.write(&metadata_dir).await?;
    write(&metadata_dir.join("root.json"), &root_bytes)?;

    // The generation verifies where it was assembled. Only then does it become the one at the
    // destination, so a generation that does not verify never replaces one that did.
    verify(&staging, false).await?;
    let published = publish(&staging, out_dir, existing)?;

    Ok(BuildOutcome {
        target_count: staged.len(),
        metadata_dir: out_dir.join(METADATA_DIR),
        targets_dir: out_dir.join(TARGETS_DIR),
        retired: published.retired,
    })
}

/// What is already at a build's destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Destination {
    /// Nothing is there.
    Absent,
    /// A directory is there and it is empty.
    Empty,
    /// A generation is there.
    Occupied,
}

/// Returns what is at a destination, refusing one that holds something other than a generation.
///
/// `--replace` removes what it finds, so it only ever looks at a directory that is recognisably a
/// generation: a root, its metadata and its targets. Pointing a build at a signing directory or a
/// checkout is a mistake that would otherwise delete them.
fn describe_destination(out_dir: &Path) -> Result<Destination> {
    if !out_dir.exists() {
        return Ok(Destination::Absent);
    }
    if !out_dir.is_dir() {
        return Err(Error::Layout {
            detail: format!("{} is not a directory", out_dir.display()),
        });
    }
    let empty = std::fs::read_dir(out_dir)
        .map_err(|source| Error::Io {
            path: out_dir.to_path_buf(),
            source,
        })?
        .next()
        .is_none();
    if empty {
        return Ok(Destination::Empty);
    }
    // A generation holds exactly three things. A directory that holds anything else is somebody's
    // data, and `--replace` removes what it finds.
    let mut names: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(out_dir).map_err(|source| Error::Io {
        path: out_dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| Error::Io {
            path: out_dir.to_path_buf(),
            source,
        })?;
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    let expected = [
        "root.json".to_owned(),
        METADATA_DIR.to_owned(),
        TARGETS_DIR.to_owned(),
    ];
    let mut expected: Vec<String> = expected.into();
    expected.sort();
    if names == expected
        && out_dir.join("root.json").is_file()
        && out_dir.join(METADATA_DIR).is_dir()
        && out_dir.join(TARGETS_DIR).is_dir()
    {
        Ok(Destination::Occupied)
    } else {
        Err(Error::Layout {
            detail: format!(
                "{} holds something that is not a generation; a build writes generations",
                out_dir.display()
            ),
        })
    }
}

/// Returns the staging directory a build assembles a generation in.
fn staging_dir(out_dir: &Path) -> PathBuf {
    let name = out_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("generation");
    out_dir.with_file_name(format!(".{name}.building"))
}

/// Moves a finished generation into its destination.
///
/// The previous generation is moved aside under its own name and left there. Nothing this did not
/// write is removed: a directory that was checked before a build started is not the same directory
/// when the build finishes, and the only safe answer to that is to keep it. An empty destination is
/// removed, because removing an empty directory destroys nothing.
fn publish(staging: &Path, out_dir: &Path, existing: Destination) -> Result<PublishOutcome> {
    // What is at the destination now, rather than what was there when the build started.
    if describe_destination(out_dir)? != existing {
        return Err(Error::Layout {
            detail: format!(
                "{} changed while the generation was being built; nothing was replaced",
                out_dir.display()
            ),
        });
    }

    let mut retired = None;
    match existing {
        Destination::Absent => {}
        Destination::Empty => {
            std::fs::remove_dir(out_dir).map_err(|source| Error::Io {
                path: out_dir.to_path_buf(),
                source,
            })?;
        }
        Destination::Occupied => {
            let aside = retired_dir(out_dir)?;
            std::fs::rename(out_dir, &aside).map_err(|source| Error::Io {
                path: out_dir.to_path_buf(),
                source,
            })?;
            retired = Some(aside);
        }
    }

    if let Some(parent) = out_dir.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    if let Err(source) = std::fs::rename(staging, out_dir) {
        // Put the previous generation back rather than leaving the destination empty.
        if let Some(aside) = &retired {
            let _ = std::fs::rename(aside, out_dir);
        }
        return Err(Error::Io {
            path: out_dir.to_path_buf(),
            source,
        });
    }
    Ok(PublishOutcome { retired })
}

/// Where a publication put the generation it replaced.
#[derive(Clone, Debug)]
struct PublishOutcome {
    /// The directory the previous generation was moved to, where there was one.
    retired: Option<PathBuf>,
}

/// Returns an unused name beside the destination for the generation being replaced.
fn retired_dir(out_dir: &Path) -> Result<PathBuf> {
    let name = out_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("generation");
    for ordinal in 0..1000u32 {
        let candidate = out_dir.with_file_name(format!(".{name}.retired.{ordinal}"));
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(Error::Layout {
        detail: format!(
            "every name beside {} is taken by a generation this replaced; remove the ones you no \
             longer want",
            out_dir.display()
        ),
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

/// What the index declares for each target, by target name.
type Declared = BTreeMap<String, (PayloadDigest, u64)>;

/// Returns the digest and length the index declares for every package payload.
fn report_paths(index: &CatalogueIndex) -> Result<Declared> {
    let mut declared = Declared::new();
    for entry in &index.entries {
        let prefix = format!(
            "packages/{}/{}/{}",
            entry.publisher_id, entry.plugin_name, entry.version
        );
        declared.insert(
            format!("{prefix}/{}", kr_plugin_sdk::package::MANIFEST_FILE),
            (entry.manifest_digest, entry.manifest_size_bytes.get()),
        );
        for payload in &entry.payloads {
            declared.insert(
                format!("{prefix}/{}", payload.path),
                (payload.digest, payload.size_bytes.get()),
            );
        }
    }
    Ok(declared)
}

/// Checks that every staged byte is a byte validation saw.
///
/// Staging reopens the package files, so a file edited between validation and signing would
/// otherwise be signed without ever being checked. Every staged target is hashed again and compared
/// with what the index declares, and the index itself is compared with its canonical rendering.
fn check_staged_against_validation(
    staged: &[StagedTarget],
    declared: &Declared,
    index_bytes: &[u8],
) -> Result<()> {
    for target in staged {
        let bytes = read(&target.path)?;
        check_bytes_against_validation(&target.name, &bytes, declared, index_bytes)?;
    }
    let names: BTreeSet<&str> = staged.iter().map(|target| target.name.as_str()).collect();
    for name in declared.keys() {
        if !names.contains(name.as_str()) {
            return Err(Error::Layout {
                detail: format!("the index declares {name} and nothing staged it"),
            });
        }
    }
    Ok(())
}

/// Checks one target's bytes against what the index declares.
fn check_bytes_against_validation(
    name: &str,
    bytes: &[u8],
    declared: &Declared,
    index_bytes: &[u8],
) -> Result<()> {
    if name == INDEX_TARGET {
        if bytes != index_bytes {
            return Err(Error::Layout {
                detail: "the index target is not the index that was built".to_owned(),
            });
        }
        return Ok(());
    }
    let Some((digest, size)) = declared.get(name) else {
        return Err(Error::Layout {
            detail: format!("{name} is a target the index does not declare"),
        });
    };
    if bytes.len() as u64 != *size || &PayloadDigest::of(bytes) != digest {
        return Err(Error::Layout {
            detail: format!("{name} is not the payload the index declares"),
        });
    }
    Ok(())
}

/// Checks the descriptor the editor built against what the index declares.
///
/// The editor reads each staged file itself, so this compares the digest and length it recorded
/// rather than trusting that the file was the same one a moment earlier. A file changed between the
/// two reads is refused rather than signed.
fn check_target_against_validation(
    name: &str,
    target: &Target,
    declared: &Declared,
    index_bytes: &[u8],
) -> Result<()> {
    let digest = PayloadDigest::parse(&hex::encode(&target.hashes.sha256)).map_err(|error| {
        Error::Layout {
            detail: format!("{name}: {error}"),
        }
    })?;
    if name == INDEX_TARGET {
        if digest != PayloadDigest::of(index_bytes) || target.length != index_bytes.len() as u64 {
            return Err(Error::Layout {
                detail: "the index target changed between staging and signing".to_owned(),
            });
        }
        return Ok(());
    }
    let Some((expected, size)) = declared.get(name) else {
        return Err(Error::Layout {
            detail: format!("{name} is a target the index does not declare"),
        });
    };
    if &digest != expected || target.length != *size {
        return Err(Error::Layout {
            detail: format!("{name} changed between staging and signing; it was not signed"),
        });
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
    /// Where the generation this replaced was moved to, where it replaced one.
    ///
    /// It is left there. Removing it is the operator's decision, because a directory the pipeline
    /// checked before a build is not necessarily the same directory when the build finishes.
    pub retired: Option<PathBuf>,
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
/// does not match, the index cannot be read, or an entry names builds an index may not carry.
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
    //
    // One target is held at a time. A release is as large as the packages in it, and holding all of
    // them to compare them afterwards would make verification cost as much memory as the release.
    let index_name = TargetName::new(INDEX_TARGET)?;
    // The index is held whole, because it is parsed. Its signed length is checked against the
    // repository metadata budget first, so what is held is what a host would be willing to hold.
    let index_length = repository
        .targets()
        .signed
        .targets
        .get(&index_name)
        .map(|target| target.length)
        .ok_or_else(|| Error::Layout {
            detail: format!("the metadata does not pin {INDEX_TARGET}"),
        })?;
    if index_length > kr_plugin_sdk::limits::METADATA_BUDGET_BYTES {
        return Err(Error::Layout {
            detail: format!(
                "the index is {index_length} bytes, over the {} byte metadata budget",
                kr_plugin_sdk::limits::METADATA_BUDGET_BYTES
            ),
        });
    }
    let index_bytes = repository
        .read_target(&index_name)
        .await?
        .ok_or_else(|| Error::Layout {
            detail: format!("the generation does not carry {INDEX_TARGET}"),
        })?
        .into_vec()
        .await?;
    let index: CatalogueIndex =
        serde_json::from_slice(&index_bytes).map_err(|source| Error::Json {
            path: PathBuf::from(INDEX_TARGET),
            source,
        })?;
    // What an entry says about its builds is a signed statement a host takes an executable's version
    // from, and a host refuses a generation whose entries break the SDK's rules for them.
    crate::index::check_builds(&index)?;

    // The metadata proves the targets are the bytes it signed. The index says which bytes each
    // package consists of. Comparing the two is what makes a target name mean one package's file
    // rather than whatever happens to be under that name.
    let declared = report_paths(&index)?;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for name in names {
        let raw = name.raw().to_owned();
        if raw == INDEX_TARGET {
            seen.insert(raw);
            continue;
        }
        // A payload is hashed as it arrives and never held whole. A release is as large as the
        // packages in it, and reading one into memory to hash it would make verification cost as
        // much memory as the largest package.
        let Some((expected, size)) = declared.get(&raw).copied() else {
            return Err(Error::Layout {
                detail: format!("the generation carries {raw} and the index does not declare it"),
            });
        };
        let mut stream = repository
            .read_target(&name)
            .await?
            .ok_or_else(|| Error::Layout {
                detail: format!("the metadata pins {raw} and the generation does not carry it"),
            })?;
        let mut hasher = Sha256::new();
        let mut length: u64 = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            length = length.saturating_add(chunk.len() as u64);
            if length > size {
                return Err(Error::Layout {
                    detail: format!("{raw} is longer than the index declares"),
                });
            }
            hasher.update(&chunk);
        }
        let digest = PayloadDigest::from_bytes(hasher.finalize().into());
        if length != size || digest != expected {
            return Err(Error::Layout {
                detail: format!("{raw} is not the payload the index declares"),
            });
        }
        seen.insert(raw);
    }
    for name in declared.keys() {
        if !seen.contains(name) {
            return Err(Error::Layout {
                detail: format!("the index declares {name} and the metadata does not pin it"),
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
