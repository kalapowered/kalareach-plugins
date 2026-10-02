//! The chain of roots a generation ships, and the rule for accepting a new generation.
//!
//! A host adopts one root and follows signed rotations from it, so every root from version 1 to the
//! highest has to be in the generation, and a new generation may only move the chain forward.
//! A new generation is accepted when its number is higher and one of these holds:
//!
//! * its highest root is its predecessor's, byte for byte;
//! * every root its predecessor shipped is unchanged and each root after them carries a threshold
//!   of the root before it and a threshold of its own;
//! * its predecessor's highest root is the one root this pipeline's development history began with
//!   (the pinned digest below) and the new chain starts at the development lineage's version-1
//!   root: the one break, which works from that predecessor only.
//!
//! While the lineage is made of public development keys this is a consistency check and not a
//! trust boundary: anybody can sign a valid rotation out of the development root. A host trusts a
//! release only for root keys its own build commits.

use std::collections::BTreeMap;
use std::path::Path;

use sha2::{Digest as _, Sha256};
use tough::schema::{Root, Signed};

use crate::{Error, Result, read};

/// The SHA-256 of the root the development generation had before its first Ed25519 root: the one
/// predecessor the break is accepted from.
pub const BREAK_FROM_DIGEST: &str =
    "dbe05446305bfc750ebf4cb3942228f4c7ce75b3f6cb528cae8aa040f2e805a5";

/// How a new generation's roots relate to its predecessor's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lineage {
    /// The highest root is the same file.
    Same,
    /// Every earlier root is unchanged and the chain goes on to a higher root.
    Chain {
        /// The predecessor's highest version.
        from: u64,
        /// The new highest version.
        to: u64,
    },
    /// The development lineage's one break.
    Break,
}

/// Reads the numbered roots under a metadata directory, as `N.root.json`, by version.
///
/// # Errors
///
/// Returns an error when the directory cannot be read.
pub fn numbered_roots(directory: &Path) -> Result<BTreeMap<u64, Vec<u8>>> {
    let mut found = BTreeMap::new();
    for entry in std::fs::read_dir(directory).map_err(|source| Error::Io {
        path: directory.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| Error::Io {
            path: directory.to_path_buf(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(version) = name
            .strip_suffix(".root.json")
            .filter(|number| {
                !number.is_empty()
                    && !number.starts_with('0')
                    && number.bytes().all(|byte| byte.is_ascii_digit())
            })
            .and_then(|number| number.parse::<u64>().ok())
        {
            found.insert(version, read(&entry.path())?);
        }
    }
    Ok(found)
}

/// The roots one generation ships: every `N.root.json` from 1 with no gap, with `root.json` in the
/// metadata directory and at the top of the generation both the highest.
fn shipped(generation: &Path) -> Result<BTreeMap<u64, Vec<u8>>> {
    let metadata = generation.join(crate::tuf::METADATA_DIR);
    let roots = numbered_roots(&metadata)?;
    let Some((&highest, last)) = roots.iter().next_back() else {
        return Err(Error::Layout {
            detail: format!("{} ships no numbered root", metadata.display()),
        });
    };
    for expected in 1..=highest {
        if !roots.contains_key(&expected) {
            return Err(Error::Layout {
                detail: format!(
                    "{} ships roots up to version {highest} and not {expected}; a host that \
                     adopted an earlier root could not follow the chain",
                    metadata.display()
                ),
            });
        }
    }
    for current in [metadata.join("root.json"), generation.join("root.json")] {
        if &read(&current)? != last {
            return Err(Error::Layout {
                detail: format!(
                    "{} is not {highest}.root.json, the highest root the generation ships",
                    current.display()
                ),
            });
        }
    }
    Ok(roots)
}

fn parse(version: u64, bytes: &[u8]) -> Result<Signed<Root>> {
    let root: Signed<Root> = serde_json::from_slice(bytes).map_err(|source| Error::Json {
        path: format!("{version}.root.json").into(),
        source,
    })?;
    if root.signed.version.get() != version {
        return Err(Error::Layout {
            detail: format!("{version}.root.json is version {}", root.signed.version),
        });
    }
    Ok(root)
}

/// Checks that each root from `from` on is signed under a threshold of the root before it and a
/// threshold of its own.
fn check_rotations(roots: &BTreeMap<u64, Vec<u8>>, from: u64, to: u64) -> Result<()> {
    for version in from..to {
        let before = parse(version, &roots[&version])?;
        let after = parse(version + 1, &roots[&(version + 1)])?;
        before
            .signed
            .verify_role(&after)
            .map_err(|source| Error::Layout {
                detail: format!(
                    "{}.root.json does not carry a threshold of {version}.root.json's root keys: \
                 {source}",
                    version + 1
                ),
            })?;
        after
            .signed
            .verify_role(&after)
            .map_err(|source| Error::Layout {
                detail: format!(
                    "{}.root.json does not carry a threshold of its own root keys: {source}",
                    version + 1
                ),
            })?;
    }
    Ok(())
}

/// Accepts a new generation after its predecessor, or says why not.
///
/// Both generations are verified as a host verifies them, with expiry left to the host: this
/// checks which generation follows which, not for how long either is valid.
///
/// # Errors
///
/// Returns an error when either generation does not verify, the new generation's number is not
/// higher, or its roots do not continue the predecessor's.
pub async fn check_generation(new: &Path, previous: &Path) -> Result<Lineage> {
    check_generation_from(new, previous, BREAK_FROM_DIGEST).await
}

/// [`check_generation`] with the digest of the one predecessor the break is accepted from named,
/// so a test can stand in a predecessor of its own.
///
/// # Errors
///
/// Returns what [`check_generation`] returns.
pub async fn check_generation_from(
    new: &Path,
    previous: &Path,
    break_from: &str,
) -> Result<Lineage> {
    let newer = crate::tuf::verify(new, false).await?;
    let older = crate::tuf::verify(previous, false).await?;
    if newer.index.generation <= older.index.generation {
        return Err(Error::Layout {
            detail: format!(
                "generation {} does not follow generation {}: a generation is higher than the one \
                 before it",
                newer.index.generation, older.index.generation
            ),
        });
    }
    let after = shipped(new)?;
    let before = shipped(previous)?;
    let (&highest_after, _) = after.iter().next_back().expect("a generation ships a root");
    let (&highest_before, last_before) = before
        .iter()
        .next_back()
        .expect("a generation ships a root");

    let continues = before
        .iter()
        .all(|(version, bytes)| after.get(version) == Some(bytes));
    if continues {
        if highest_after == highest_before {
            return Ok(Lineage::Same);
        }
        check_rotations(&after, highest_before, highest_after)?;
        return Ok(Lineage::Chain {
            from: highest_before,
            to: highest_after,
        });
    }

    // A chain that does not continue is the one break or nothing.
    let digest = hex::encode(Sha256::digest(last_before));
    if digest != break_from {
        return Err(Error::Layout {
            detail: format!(
                "the roots of generation {} do not continue those of generation {}: it rewrites a \
                 root its predecessor shipped, and the predecessor's root ({digest}) is not the \
                 one the lineage may break from",
                newer.index.generation, older.index.generation
            ),
        });
    }
    let derived = crate::development::version_one_root_bytes().await?;
    if after.get(&1) != Some(&derived) {
        return Err(Error::Layout {
            detail: "the lineage may break only to the development lineage's version-1 root, \
                     which the new generation does not start with"
                .to_owned(),
        });
    }
    check_rotations(&after, 1, highest_after)?;
    Ok(Lineage::Break)
}
