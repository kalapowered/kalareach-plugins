//! Signing keys and the trust root.
//!
//! Signing keys never live in this repository. The pipeline reads them from a directory the
//! operator names, and before it reads anything it walks the working tree and refuses to run if a
//! private key is inside it. A key that reaches a commit is a key that has to be rotated, so the
//! check runs on every command that signs, not only when someone remembers.
//!
//! The four TUF roles each have their own key: root, targets, snapshot and timestamp. Separating
//! them is the point of the design. A compromised timestamp key lets an attacker hold a client on
//! an old generation; it does not let them publish a package.

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};

use aws_lc_rs::rand::SystemRandom;
use tough::editor::signed::SignedRole;
use tough::key_source::{KeySource, LocalKeySource};
use tough::schema::key::Key;
use tough::schema::{KeyHolder, RoleKeys, RoleType, Root};

use crate::{Error, Result};

/// The file name of each role's key inside the signing directory.
pub const ROLE_KEY_FILES: &[(RoleType, &str)] = &[
    (RoleType::Root, "root.pem"),
    (RoleType::Targets, "targets.pem"),
    (RoleType::Snapshot, "snapshot.pem"),
    (RoleType::Timestamp, "timestamp.pem"),
];

/// The file name of the trust root inside the signing directory.
pub const ROOT_FILE: &str = "root.json";

/// Directories the key scan never descends into.
const SKIPPED: &[&str] = &[".git", "target", "node_modules"];

/// The start of the PEM header that opens a private key block.
const PEM_HEADER_START: &str = "-----BEGIN ";

/// The end of that header.
///
/// The two halves are joined at compile time rather than written out whole, so this file does not
/// itself look like a key to the scan it implements.
const PEM_HEADER_END: &str = concat!("PRIVATE", " KEY-----");

/// Maximum bytes of a file the key scan reads looking for the PEM marker.
const SCAN_BYTES: usize = 4096;

/// Refuses to continue when a private key is inside the repository.
///
/// The scan is by content as well as by name. A key renamed to `notes.txt` is still a key, and the
/// PEM header is in the first few bytes of every one of them.
///
/// # Errors
///
/// Returns [`Error::KeyInTree`] naming the first file that looks like a private key, and
/// [`Error::Io`] when the tree cannot be walked.
pub fn refuse_keys_in_tree(root: &Path) -> Result<()> {
    let mut queue = vec![root.to_path_buf()];
    while let Some(directory) = queue.pop() {
        let entries = std::fs::read_dir(&directory).map_err(|source| Error::Io {
            path: directory.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io {
                path: directory.clone(),
                source,
            })?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = std::fs::symlink_metadata(&path).map_err(|source| Error::Io {
                path: path.clone(),
                source,
            })?;
            if metadata.is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                if !SKIPPED.contains(&name.as_str()) {
                    queue.push(path);
                }
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            if name.ends_with(".pem") || name.ends_with(".key") {
                return Err(Error::KeyInTree { path });
            }
            if looks_like_private_key(&path)? {
                return Err(Error::KeyInTree { path });
            }
        }
    }
    Ok(())
}

fn looks_like_private_key(path: &Path) -> Result<bool> {
    use std::io::Read as _;
    let mut file = std::fs::File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut buffer = vec![0u8; SCAN_BYTES];
    let read = file.read(&mut buffer).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    buffer.truncate(read);
    Ok(String::from_utf8_lossy(&buffer).lines().any(|line| {
        let line = line.trim();
        line.starts_with(PEM_HEADER_START) && line.ends_with(PEM_HEADER_END)
    }))
}

/// A directory of signing keys outside the repository.
#[derive(Clone, Debug)]
pub struct SigningDirectory {
    /// Where the keys are.
    pub path: PathBuf,
}

impl SigningDirectory {
    /// Names a signing directory, refusing one inside the repository.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Signing`] when the directory is missing, is not a directory, or sits
    /// inside the repository working tree.
    pub fn open(path: &Path, repository_root: &Path) -> Result<Self> {
        let path = canonical(path)?;
        let repository_root = canonical(repository_root)?;
        if path.starts_with(&repository_root) {
            return Err(Error::Signing {
                detail: format!(
                    "{} is inside {}; signing keys live outside the repository",
                    path.display(),
                    repository_root.display()
                ),
            });
        }
        if !path.is_dir() {
            return Err(Error::Signing {
                detail: format!("{} is not a directory", path.display()),
            });
        }
        Ok(Self { path })
    }

    /// Returns the path of one role's key.
    #[must_use]
    pub fn key_path(&self, role: RoleType) -> PathBuf {
        let name = ROLE_KEY_FILES
            .iter()
            .find(|(candidate, _)| *candidate == role)
            .map_or("unknown.pem", |(_, name)| *name);
        self.path.join(name)
    }

    /// Returns the path of the trust root.
    #[must_use]
    pub fn root_path(&self) -> PathBuf {
        self.path.join(ROOT_FILE)
    }

    /// Returns a key source per role, checking that every key is present.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Signing`] naming the first missing key.
    pub fn key_sources(&self) -> Result<Vec<Box<dyn KeySource>>> {
        let mut sources: Vec<Box<dyn KeySource>> = Vec::new();
        for (role, _) in ROLE_KEY_FILES {
            let path = self.key_path(*role);
            if !path.is_file() {
                return Err(Error::Signing {
                    detail: format!("{} is missing", path.display()),
                });
            }
            sources.push(Box::new(LocalKeySource { path }));
        }
        Ok(sources)
    }
}

fn canonical(path: &Path) -> Result<PathBuf> {
    std::fs::canonicalize(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Builds and signs a trust root over the four role keys.
///
/// Each role gets its own key and a threshold of one. A production root raises the root threshold
/// and holds its keys offline; this builds the object either way, and the caller supplies the keys.
///
/// # Errors
///
/// Returns an error when a key cannot be read or the root cannot be signed.
pub async fn build_root(
    directory: &SigningDirectory,
    expires: jiff::Timestamp,
) -> Result<SignedRole<Root>> {
    let mut keys: HashMap<tough::schema::decoded::Decoded<tough::schema::decoded::Hex>, Key> =
        HashMap::new();
    let mut roles: HashMap<RoleType, RoleKeys> = HashMap::new();

    for (role, _) in ROLE_KEY_FILES {
        let source = LocalKeySource {
            path: directory.key_path(*role),
        };
        let sign = source.as_sign().await.map_err(|source| Error::Signing {
            detail: format!("{}: {source}", directory.key_path(*role).display()),
        })?;
        let key: Key = sign.tuf_key();
        let key_id = key.key_id()?;
        keys.insert(key_id.clone(), key);
        roles.insert(
            *role,
            RoleKeys {
                keyids: vec![key_id],
                threshold: NonZeroU64::new(1).expect("one is not zero"),
                _extra: HashMap::new(),
            },
        );
    }

    let root = Root {
        spec_version: "1.0.0".to_owned(),
        consistent_snapshot: false,
        version: NonZeroU64::new(1).expect("one is not zero"),
        expires,
        keys,
        roles,
        _extra: HashMap::new(),
    };
    let sources = directory.key_sources()?;
    let signed = SignedRole::new(
        root.clone(),
        &KeyHolder::Root(root),
        &sources,
        &SystemRandom::new(),
    )
    .await?;
    Ok(signed)
}
