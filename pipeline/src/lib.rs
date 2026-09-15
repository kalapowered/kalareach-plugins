//! The KalaReach catalogue pipeline.
//!
//! A catalogue release is three things produced in order.
//!
//! 1. **Validation.** Every package in `plugins/` is checked with the same validator a host runs,
//!    and every package fixture is evaluated against the package's own control predicates. A
//!    package that a host would reject never reaches an index.
//! 2. **The index.** One signed metadata snapshot carrying every package's compact description,
//!    match rules, capability declarations and payload hashes and sizes, rendered as canonical
//!    JSON so the same inputs always produce the same bytes.
//! 3. **The TUF repository.** Root, timestamp, snapshot and targets metadata over the index and
//!    every package payload, signed with keys the pipeline reads from a directory outside the
//!    repository and refuses to read from inside it.
//!
//! The output is immutable. A generation is written once, under its own directory, and a rebuild
//! of the same inputs produces the same index bytes and the same target digests.

pub mod bench;
pub mod fixtures;
pub mod index;
pub mod keys;
pub mod packages;
pub mod tuf;

use std::path::{Path, PathBuf};

/// Anything that stops the pipeline.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    Io {
        /// The file involved.
        path: PathBuf,
        /// What went wrong.
        source: std::io::Error,
    },
    /// A JSON document could not be read or written.
    #[error("{path}: {source}")]
    Json {
        /// The document involved.
        path: PathBuf,
        /// What went wrong.
        source: serde_json::Error,
    },
    /// One or more packages failed validation.
    #[error("{count} package(s) failed validation")]
    InvalidPackages {
        /// How many packages failed.
        count: usize,
    },
    /// A package fixture did not match what the package's own predicates produce.
    #[error("{package}: fixture {case}: {detail}")]
    FixtureMismatch {
        /// The package whose fixture failed.
        package: String,
        /// The case that failed.
        case: String,
        /// What differed.
        detail: String,
    },
    /// A private key was found inside the repository.
    #[error(
        "{path} looks like a private signing key inside the repository; signing keys live outside the tree"
    )]
    KeyInTree {
        /// Where the key was found.
        path: PathBuf,
    },
    /// The signing directory is missing something.
    #[error("{detail}")]
    Signing {
        /// What is missing.
        detail: String,
    },
    /// The TUF library reported a problem.
    ///
    /// Boxed because the library's error type is large, and every fallible call in this crate
    /// would otherwise carry that size in its success path too.
    #[error("{0}")]
    Tuf(Box<tough::error::Error>),
    /// A TUF metadata object was not what it claimed to be.
    #[error("{0}")]
    TufSchema(Box<tough::schema::Error>),
    /// Two packages claim the same identity.
    #[error("{plugin_id} {version} is defined in both {first} and {second}")]
    DuplicatePackage {
        /// The identity claimed twice.
        plugin_id: String,
        /// The version claimed twice.
        version: String,
        /// The first directory.
        first: String,
        /// The second directory.
        second: String,
    },
    /// The repository layout is not what the pipeline expects.
    #[error("{detail}")]
    Layout {
        /// What is wrong.
        detail: String,
    },
}

impl From<tough::error::Error> for Error {
    fn from(source: tough::error::Error) -> Self {
        Self::Tuf(Box::new(source))
    }
}

impl From<tough::schema::Error> for Error {
    fn from(source: tough::schema::Error) -> Self {
        Self::TufSchema(Box::new(source))
    }
}

/// The pipeline's result type.
pub type Result<T> = std::result::Result<T, Error>;

/// Reads a file, naming it in any error.
///
/// # Errors
///
/// Returns [`Error::Io`] when the file cannot be read.
pub fn read(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Reads and parses a JSON document, naming it in any error.
///
/// # Errors
///
/// Returns [`Error::Io`] or [`Error::Json`] when the document cannot be read or parsed.
pub fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = read(path)?;
    serde_json::from_slice(&bytes).map_err(|source| Error::Json {
        path: path.to_path_buf(),
        source,
    })
}

/// Writes a file, creating its parent directory and naming it in any error.
///
/// # Errors
///
/// Returns [`Error::Io`] when the file cannot be written.
pub fn write(path: &Path, contents: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    std::fs::write(path, contents).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Returns the repository root that contains `plugins/` and `publishers/`.
///
/// # Errors
///
/// Returns [`Error::Layout`] when neither the given path nor any of its parents holds both.
pub fn repository_root(start: &Path) -> Result<PathBuf> {
    let mut current = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|source| Error::Io {
                path: start.to_path_buf(),
                source,
            })?
            .join(start)
    };
    loop {
        if current.join("plugins").is_dir() && current.join("publishers").is_dir() {
            return Ok(current);
        }
        if !current.pop() {
            return Err(Error::Layout {
                detail: format!(
                    "no directory at or above {} holds both plugins/ and publishers/",
                    start.display()
                ),
            });
        }
    }
}
