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

use std::path::{Path, PathBuf};

use kr_plugin_sdk::catalogue::CatalogueIndex;
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
) -> Result<BuildOutcome> {
    let targets_dir = out_dir.join(TARGETS_DIR);
    let metadata_dir = out_dir.join(METADATA_DIR);
    let staged = stage_targets(repository, index, &targets_dir)?;

    let root_source = signing.root_path();
    let root_bytes = read(&root_source)?;
    write(&out_dir.join("root.json"), &root_bytes)?;

    let mut editor = RepositoryEditor::new(&root_source).await?;
    editor
        .targets_version(one())?
        .targets_expires(expiries.targets)?
        .snapshot_version(one())
        .snapshot_expires(expiries.snapshot)
        .timestamp_version(one())
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

fn one() -> std::num::NonZeroU64 {
    std::num::NonZeroU64::new(1).expect("one is not zero")
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

    let target_count = repository.targets().signed.targets.len();
    let name = TargetName::new(INDEX_TARGET)?;
    let bytes = repository
        .read_target(&name)
        .await?
        .ok_or_else(|| Error::Layout {
            detail: format!("the generation does not carry {INDEX_TARGET}"),
        })?
        .into_vec()
        .await?;
    let index: CatalogueIndex = serde_json::from_slice(&bytes).map_err(|source| Error::Json {
        path: PathBuf::from(INDEX_TARGET),
        source,
    })?;

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
