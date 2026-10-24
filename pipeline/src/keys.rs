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

use std::collections::{HashMap, HashSet};
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

/// Directories the fallback walk does not descend into.
///
/// The scan normally asks Git which files could reach a commit, which is exact. The walk below is
/// what runs when Git cannot answer, and these are the directories that hold build output rather
/// than sources: scanning a compiled artefact finds the scanner's own header constants inside it.
const SKIPPED: &[&str] = &[".git", "target", "node_modules"];

/// The start of the PEM header that opens a private key block.
const PEM_HEADER_START: &str = "-----BEGIN ";

/// The end of that header.
///
/// The two halves are joined at compile time rather than written out whole, so this file does not
/// itself look like a key to the scan it implements.
const PEM_HEADER_END: &str = concat!("PRIVATE", " KEY-----");

/// Bytes the key scan reads at a time.
///
/// The scan reads every file to its end in fixed-size chunks, carrying enough of the previous chunk
/// to catch a header that straddles the boundary. Reading to the end is the point: a key pasted
/// into the middle of a long file is still a key, and a scan that stopped early would be a scan
/// somebody could step over.
const SCAN_CHUNK: usize = 64 * 1024;

/// Refuses to continue when a private key is inside the repository.
///
/// The scan is by content as well as by name, and it reads every file to its end: a key renamed to
/// `notes.txt` is a key, and so is one pasted into the middle of a long file or inside a
/// configuration value.
///
/// # Errors
///
/// Returns [`Error::KeyInTree`] naming the first file that looks like a private key, and
/// [`Error::Io`] when the tree cannot be read.
pub fn refuse_keys_in_tree(root: &Path) -> Result<()> {
    match checkout_state(root)? {
        CheckoutState::Checkout => {
            // Inside a checkout, Git decides which files could reach a commit. A command that then
            // fails is a failure rather than a reason to fall back to a coarser scan of the same
            // tree.
            for path in tracked_and_untracked(root)? {
                check_file(&path)?;
            }
            Ok(())
        }
        CheckoutState::NotACheckout => walk(root),
    }
}

/// Whether a directory is inside a Git checkout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CheckoutState {
    /// Git answered that it is.
    Checkout,
    /// Git answered that it is not.
    NotACheckout,
}

/// Returns whether a directory is inside a Git checkout.
///
/// The answer comes from the filesystem rather than from a command's message. A `.git` directory or
/// file at this directory or above it is a checkout, and nothing else is. Asking Git and reading
/// what it said would depend on the wording of one version's diagnostics in one language, and on an
/// inherited `GIT_DIR` that can make a checkout answer that it is not one.
fn checkout_state(root: &Path) -> Result<CheckoutState> {
    let mut current = std::fs::canonicalize(root).map_err(|source| Error::Io {
        path: root.to_path_buf(),
        source,
    })?;
    loop {
        let marker = current.join(".git");
        // `symlink_metadata` reports the entry itself. A `.git` that is a broken link is still a
        // `.git`, and treating it as absent would quietly fall back to the coarser walk.
        match std::fs::symlink_metadata(&marker) {
            Ok(_) => return Ok(CheckoutState::Checkout),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(Error::Io {
                    path: marker,
                    source,
                });
            }
        }
        if !current.pop() {
            return Ok(CheckoutState::NotACheckout);
        }
    }
}

/// Returns every file that could reach a commit, as Git sees them.
///
/// `git ls-files -co --exclude-standard` lists the tracked files and the untracked files Git would
/// add, which is exactly the set a commit can carry without somebody overriding an ignore rule. It
/// includes a tracked file wherever it sits, so a key committed under a directory the fallback walk
/// would skip is still found, and it leaves out build output, where the scan would otherwise find
/// its own header constants compiled into a binary.
///
/// # Errors
///
/// Returns [`Error::Signing`] when Git is inside a checkout and still cannot list it, and when a
/// listed name is not something this platform can turn back into a path. A name the scan cannot
/// open is a file the scan cannot check, and the safe answer to that is to stop.
fn tracked_and_untracked(root: &Path) -> Result<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "-c", "-o", "--exclude-standard"])
        .output()
        .map_err(|source| Error::Signing {
            detail: format!("git could not list {}: {source}", root.display()),
        })?;
    if !output.status.success() {
        return Err(Error::Signing {
            detail: format!(
                "git could not list {}: {}",
                root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    let mut paths = Vec::new();
    for name in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        // The bytes are kept as they are. A file name that is not valid UTF-8 is a real file on
        // Linux, and converting it lossily would produce a path that opens nothing.
        #[cfg(unix)]
        let relative = {
            use std::os::unix::ffi::OsStrExt as _;
            PathBuf::from(std::ffi::OsStr::from_bytes(name))
        };
        #[cfg(not(unix))]
        let relative = match std::str::from_utf8(name) {
            Ok(text) => PathBuf::from(text),
            Err(error) => {
                return Err(Error::Signing {
                    detail: format!("git listed a name this platform cannot open: {error}"),
                });
            }
        };
        paths.push(root.join(relative));
    }
    Ok(paths)
}

/// Walks a tree that Git cannot describe.
fn walk(root: &Path) -> Result<()> {
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
            if metadata.is_file() {
                check_file(&path)?;
            }
        }
    }
    Ok(())
}

/// Refuses one file that looks like a private key.
fn check_file(path: &Path) -> Result<()> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    if name.ends_with(".pem") || name.ends_with(".key") {
        return Err(Error::KeyInTree {
            path: path.to_path_buf(),
        });
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Ok(());
    }
    if looks_like_private_key(path)? {
        return Err(Error::KeyInTree {
            path: path.to_path_buf(),
        });
    }
    Ok(())
}

fn looks_like_private_key(path: &Path) -> Result<bool> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    // The carry is the longest header the scan looks for, less one byte, so a header split across
    // two chunks is still seen whole. Memory stays at one chunk whatever the file's size.
    let carry = PEM_HEADER_START.len() + PEM_HEADER_END.len() + 64;
    let mut buffer = vec![0u8; SCAN_CHUNK + carry];
    let mut held = 0usize;
    loop {
        let read = file.read(&mut buffer[held..]).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            return Ok(false);
        }
        let filled = held + read;
        if contains_pem_header(&buffer[..filled]) {
            return Ok(true);
        }
        held = filled.min(carry);
        buffer.copy_within(filled - held..filled, 0);
    }
}

/// Returns true when a window of bytes carries a private key header.
///
/// The header is looked for anywhere in a line rather than at its start, because a key pasted into
/// a source file, a JSON document or a configuration value is still a key.
fn contains_pem_header(window: &[u8]) -> bool {
    String::from_utf8_lossy(window).lines().any(|line| {
        line.find(PEM_HEADER_START)
            .is_some_and(|start| line[start + PEM_HEADER_START.len()..].contains(PEM_HEADER_END))
    })
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

/// What signs a generation: the four role keys and the roots a host walks from its own.
///
/// A directory of signing keys is one source and the derived development keys are another. The
/// pipeline asks for nothing else of either.
pub trait SigningMaterial {
    /// Returns a key source per role, in the order the roles are listed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Signing`] when a key is missing.
    fn key_sources(&self) -> Result<Vec<Box<dyn KeySource>>>;

    /// Returns every root a generation ships, as the files they are written as, from version 1 to
    /// the highest. The last is the root the generation is signed under.
    ///
    /// # Errors
    ///
    /// Returns an error when a root cannot be read.
    fn roots(&self) -> Result<Vec<Vec<u8>>>;
}

impl SigningMaterial for SigningDirectory {
    fn key_sources(&self) -> Result<Vec<Box<dyn KeySource>>> {
        Self::key_sources(self)
    }

    /// The roots in the directory: `1.root.json` to the highest where the directory holds them,
    /// and `root.json` alone where it holds one root. `root.json` is the highest in either case.
    fn roots(&self) -> Result<Vec<Vec<u8>>> {
        let current = crate::read(&self.root_path())?;
        let mut numbered: Vec<(u64, PathBuf)> = Vec::new();
        for entry in std::fs::read_dir(&self.path).map_err(|source| Error::Io {
            path: self.path.clone(),
            source,
        })? {
            let entry = entry.map_err(|source| Error::Io {
                path: self.path.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(version) = name
                .strip_suffix(".root.json")
                .filter(|number| {
                    !number.starts_with('0') && number.bytes().all(|b| b.is_ascii_digit())
                })
                .and_then(|number| number.parse::<u64>().ok())
            {
                numbered.push((version, entry.path()));
            }
        }
        if numbered.is_empty() {
            return Ok(vec![current]);
        }
        numbered.sort();
        let mut roots = Vec::new();
        for (_, path) in &numbered {
            roots.push(crate::read(path)?);
        }
        if roots.last() != Some(&current) {
            return Err(Error::Signing {
                detail: format!(
                    "{} is not the highest of the numbered roots beside it",
                    self.root_path().display()
                ),
            });
        }
        Ok(roots)
    }
}

impl SigningDirectory {
    /// Returns each role's key source, named by the role.
    #[must_use]
    pub fn role_sources(&self) -> Vec<(RoleType, Box<dyn KeySource>)> {
        ROLE_KEY_FILES
            .iter()
            .map(|(role, _)| {
                (
                    *role,
                    Box::new(LocalKeySource {
                        path: self.key_path(*role),
                    }) as Box<dyn KeySource>,
                )
            })
            .collect()
    }
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
    // Every key is checked present before any is read, so the first missing one is named.
    directory.key_sources()?;
    build_root_from(
        directory.role_sources(),
        NonZeroU64::new(1).expect("one is not zero"),
        expires,
    )
    .await
}

/// Builds and signs a trust root of `version` over the four role keys of `sources`.
///
/// # Errors
///
/// Returns an error when a key cannot be read, two roles share a key, or the root cannot be
/// signed.
pub async fn build_root_from(
    sources: Vec<(RoleType, Box<dyn KeySource>)>,
    version: NonZeroU64,
    expires: jiff::Timestamp,
) -> Result<SignedRole<Root>> {
    let (roles, keys): (Vec<RoleType>, Vec<Box<dyn KeySource>>) = sources.into_iter().unzip();
    let root = root_over(&roles, &keys, version, expires).await?;
    Ok(SignedRole::new(
        root.clone(),
        &KeyHolder::Root(root),
        &keys,
        &SystemRandom::new(),
    )
    .await?)
}

/// The root object over the four role keys, unsigned.
async fn root_over(
    roles_in_order: &[RoleType],
    sources: &[Box<dyn KeySource>],
    version: NonZeroU64,
    expires: jiff::Timestamp,
) -> Result<Root> {
    let mut keys: HashMap<tough::schema::decoded::Decoded<tough::schema::decoded::Hex>, Key> =
        HashMap::new();
    let mut roles: HashMap<RoleType, RoleKeys> = HashMap::new();

    for (role, source) in roles_in_order.iter().zip(sources) {
        let sign = source.as_sign().await.map_err(|source| Error::Signing {
            detail: format!("the {role} key: {source}"),
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

    // One key per role. A root that named one key for two roles would grant whoever holds it both,
    // and a root file cannot say afterwards which of the two the key was meant for.
    let root = Root {
        spec_version: "1.0.0".to_owned(),
        consistent_snapshot: false,
        version,
        expires,
        keys,
        roles,
        _extra: HashMap::new(),
    };
    check_role_keys(&root)?;
    Ok(root)
}

/// A key as it is compared with the keys of other roles: its family, and its public value in the
/// one spelling every spelling this comparison accepts reduces to; `None` for a spelling it cannot
/// reduce.
///
/// The library that checks a signature reads a key in more spellings than one, and each has an
/// identifier of its own, so two identifiers can name one key. Where a spelling cannot be compared
/// by its bytes the key is refused, since a key the comparison cannot place is a key it cannot
/// say it does not share:
///
/// - An Ed25519 key is its 32 raw bytes, or those bytes in a `SubjectPublicKeyInfo` of exactly 44
///   bytes. The library reads a longer value as a `SubjectPublicKeyInfo` and ignores what follows
///   it, so a longer value is refused.
/// - An elliptic-curve key is a point: 65 bytes opening `04`, or 33 bytes opening `02` or `03`,
///   compared by its X coordinate (a key and its negation share it, and a holder of either can sign
///   as both). A whole `SubjectPublicKeyInfo`, which the library also reads, is refused.
/// - An RSA key is one `RSAPublicKey`: a DER sequence that opens with an integer and ends where the
///   value ends. The library also reads a `SubjectPublicKeyInfo` in its place, which is refused.
///
/// Every key this pipeline writes has one of these spellings. The same value under two key families
/// is not compared: one key has one family.
fn comparable(key: &Key) -> Option<(&'static str, Vec<u8>)> {
    /// The twelve bytes that wrap an Ed25519 public key in a `SubjectPublicKeyInfo`.
    const ED25519_WRAPPER: [u8; 12] = [
        0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
    ];
    match key {
        Key::Ed25519 { keyval, .. } => {
            let public: &[u8] = &keyval.public;
            let raw = if public.len() == 32 {
                public
            } else if public.len() == 44 && public.starts_with(&ED25519_WRAPPER) {
                &public[ED25519_WRAPPER.len()..]
            } else {
                return None;
            };
            Some(("ed25519", raw.to_vec()))
        }
        Key::Rsa { keyval, .. } => {
            is_rsa_public_key(&keyval.public).then(|| ("rsa", keyval.public.to_vec()))
        }
        Key::Ecdsa { keyval, .. } | Key::EcdsaOld { keyval, .. } => {
            let point: &[u8] = &keyval.public;
            let x = match point {
                [0x04, rest @ ..] if rest.len() == 64 => &rest[..32],
                [0x02 | 0x03, rest @ ..] if rest.len() == 32 => rest,
                _ => return None,
            };
            Some(("ecdsa", x.to_vec()))
        }
    }
}

/// Whether `der` is one DER `RSAPublicKey`: a sequence that opens with an integer and ends where
/// `der` ends. A `SubjectPublicKeyInfo` is a sequence that opens with a sequence.
fn is_rsa_public_key(der: &[u8]) -> bool {
    let [0x30, rest @ ..] = der else {
        return false;
    };
    let (length, body) = match rest {
        [length @ 0..=0x7f, body @ ..] => (usize::from(*length), body),
        [0x81, length, body @ ..] => (usize::from(*length), body),
        [0x82, high, low, body @ ..] => (usize::from(u16::from_be_bytes([*high, *low])), body),
        _ => return false,
    };
    length == body.len() && body.first() == Some(&0x02)
}

/// The keys one role names, each in the form [`comparable`] gives it.
type Material = HashSet<(&'static str, Vec<u8>)>;

/// Checks that no two roles of `root` name the same key.
///
/// A role holds the authority of its keys, so a key two roles share lets whoever holds it act for
/// both. A compromised timestamp key is meant to hold a client on an old generation and do nothing
/// more, which is not so when the same key also signs the targets.
///
/// # Errors
///
/// Returns [`Error::Signing`] naming the first two roles, in the order of their names, that name
/// one key, and the first role that names a key in a spelling the comparison cannot reduce (see
/// `comparable` for the spellings it accepts).
pub fn check_role_keys(root: &Root) -> Result<()> {
    let mut roles: Vec<_> = root.roles.iter().collect();
    roles.sort_by_key(|(role, _)| role.to_string());
    let mut named: Vec<(String, Material)> = Vec::with_capacity(roles.len());
    for (role, role_keys) in roles {
        let mut material = Material::new();
        for identifier in &role_keys.keyids {
            material.insert(match root.keys.get(identifier) {
                Some(key) => comparable(key).ok_or_else(|| Error::Signing {
                    detail: format!(
                        "the {role} role names a key in a spelling this check cannot compare with \
                         the other roles' keys: an Ed25519 key is its 32 raw bytes, an \
                         elliptic-curve key a point and an RSA key one RSAPublicKey"
                    ),
                })?,
                // A key the root does not describe is named by nothing a signature could be
                // counted against, so it stands for itself.
                None => ("identifier", identifier.to_vec()),
            });
        }
        named.push((role.to_string(), material));
    }
    for (at, (first, keys)) in named.iter().enumerate() {
        for (second, others) in &named[at + 1..] {
            if !keys.is_disjoint(others) {
                return Err(Error::Signing {
                    detail: format!(
                        "the {first} and {second} roles name the same key; each role has its own"
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Builds the root that follows `previous`, over the keys of `next`, and signs it under both.
///
/// A host that trusts `previous` follows the chain only when the new root carries a threshold of
/// `previous`'s own root keys, and the new root carries a threshold of its own, so the signatures
/// of both are in the file. They are written in key identifier order.
///
/// # Errors
///
/// Returns an error when a key cannot be read, the keys repeat, or either threshold is not met.
pub async fn rotate_root(
    previous: &tough::schema::Signed<Root>,
    previous_sources: &[Box<dyn KeySource>],
    next: Vec<(RoleType, Box<dyn KeySource>)>,
    expires: jiff::Timestamp,
) -> Result<tough::schema::Signed<Root>> {
    let version = previous
        .signed
        .version
        .checked_add(1)
        .ok_or_else(|| Error::Signing {
            detail: "the root's version cannot go higher".to_owned(),
        })?;
    let (roles, next_sources): (Vec<RoleType>, Vec<Box<dyn KeySource>>) = next.into_iter().unzip();
    let root = root_over(&roles, &next_sources, version, expires).await?;
    let rng = SystemRandom::new();
    let under_previous = SignedRole::new(
        root.clone(),
        &KeyHolder::Root(previous.signed.clone()),
        previous_sources,
        &rng,
    )
    .await?;
    let under_next = SignedRole::new(
        root.clone(),
        &KeyHolder::Root(root.clone()),
        &next_sources,
        &rng,
    )
    .await?;
    let mut signed = tough::schema::Signed {
        signed: root,
        signatures: Vec::new(),
    };
    signed
        .signatures
        .extend(under_previous.signed().signatures.iter().cloned());
    for signature in &under_next.signed().signatures {
        if !signed
            .signatures
            .iter()
            .any(|held| held.keyid == signature.keyid)
        {
            signed.signatures.push(signature.clone());
        }
    }
    signed
        .signatures
        .sort_by(|left, right| left.keyid[..].cmp(&right.keyid[..]));
    previous.signed.verify_role(&signed)?;
    signed.signed.verify_role(&signed)?;
    Ok(signed)
}
