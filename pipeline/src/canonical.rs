//! Writing signed metadata the same way every time.
//!
//! The TUF library writes each signed role as pretty JSON over maps whose order changes from one
//! process to the next, and it pins the length and digest of those bytes in the role above. A
//! rebuild of the same inputs would then differ in every metadata file. Here each signed role is
//! written with its members in name order and its signatures in key identifier order, and the
//! snapshot and the timestamp are made over those exact bytes. A client checks a signature over
//! the canonical form of the role's content, never over the bytes of the file, so nothing about
//! trust changes: only the bytes of the files are fixed.

use std::collections::HashMap;

use aws_lc_rs::rand::SystemRandom;
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use tough::editor::signed::SignedRole;
use tough::key_source::KeySource;
use tough::schema::{
    Hashes, KeyHolder, Metafile, Role, Root, Signed, Snapshot, Targets, Timestamp,
};

use crate::{Error, Result};

/// The bytes one signed role is written as: pretty JSON with every object's members in name order,
/// the signatures in key identifier order, and a closing newline.
///
/// # Errors
///
/// Returns [`Error::Layout`] when the role cannot be rendered.
pub fn canonical<T: Serialize>(signed: &Signed<T>) -> Result<Vec<u8>> {
    let mut value = serde_json::to_value(signed).map_err(|source| Error::Layout {
        detail: format!("a signed role could not be rendered: {source}"),
    })?;
    if let Some(serde_json::Value::Array(signatures)) = value.get_mut("signatures") {
        signatures.sort_by(|left, right| {
            left.get("keyid")
                .and_then(serde_json::Value::as_str)
                .cmp(&right.get("keyid").and_then(serde_json::Value::as_str))
        });
    }
    let mut bytes = serde_json::to_vec_pretty(&value).map_err(|source| Error::Layout {
        detail: format!("a signed role could not be rendered: {source}"),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// The metadata files of one generation, as they are written.
#[derive(Clone, Debug)]
pub struct Metadata {
    /// `targets.json`.
    pub targets: Vec<u8>,
    /// `snapshot.json`.
    pub snapshot: Vec<u8>,
    /// `timestamp.json`.
    pub timestamp: Vec<u8>,
}

/// Re-renders the roles the library signed, and signs the snapshot and the timestamp again over
/// the bytes that are written.
///
/// `targets`, `snapshot` and `timestamp` are the roles as the editor produced them. The targets
/// role keeps its signatures: they are over its content, which does not change.
///
/// # Errors
///
/// Returns an error when a role cannot be rendered or signed, or the snapshot or the timestamp
/// pins something other than the one role it should.
pub async fn render(
    root: &Root,
    keys: &[Box<dyn KeySource>],
    targets: &Signed<Targets>,
    mut snapshot: Snapshot,
    mut timestamp: Timestamp,
) -> Result<Metadata> {
    let targets_bytes = canonical(targets)?;
    pin(
        &mut snapshot.meta,
        "targets.json",
        &targets_bytes,
        targets.signed.version(),
    )?;
    let rng = SystemRandom::new();
    let holder = KeyHolder::Root(root.clone());
    let signed_snapshot = SignedRole::new(snapshot, &holder, keys, &rng).await?;
    let snapshot_bytes = canonical(signed_snapshot.signed())?;
    pin(
        &mut timestamp.meta,
        "snapshot.json",
        &snapshot_bytes,
        signed_snapshot.signed().signed.version(),
    )?;
    let signed_timestamp = SignedRole::new(timestamp, &holder, keys, &rng).await?;
    let timestamp_bytes = canonical(signed_timestamp.signed())?;
    Ok(Metadata {
        targets: targets_bytes,
        snapshot: snapshot_bytes,
        timestamp: timestamp_bytes,
    })
}

/// Records the length and digest of `bytes` as what the role above pins under `name`.
fn pin(
    meta: &mut HashMap<String, Metafile>,
    name: &str,
    bytes: &[u8],
    version: std::num::NonZeroU64,
) -> Result<()> {
    if meta.len() != 1 || !meta.contains_key(name) {
        return Err(Error::Layout {
            detail: format!("a role pins more than {name}; this pipeline signs one targets role"),
        });
    }
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    meta.insert(
        name.to_owned(),
        Metafile {
            length: Some(bytes.len() as u64),
            hashes: Some(Hashes {
                sha256: digest.to_vec().into(),
                _extra: HashMap::new(),
            }),
            version,
            _extra: HashMap::new(),
        },
    );
    Ok(())
}
