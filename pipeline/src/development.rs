//! The development signing keys.
//!
//! A development generation is signed with keys that anybody can derive, so that a host built for
//! development can seed itself from the committed generation and so that a rebuild of the same
//! inputs signs with the same keys. Nothing here is secret and nothing here is a trust boundary:
//! a host trusts a release only for the production root key identifiers its own build commits, and
//! a build that carries the development identifiers refuses a bundle in a release.
//!
//! Each key is an Ed25519 key whose 32-byte seed is the SHA-256 of a published phrase and the
//! role's name. No key file and no private-key block is written anywhere in the tree, so the key
//! scan and push protection have nothing to find.
//!
//! The first set of keys, epoch 0, signs the version-1 root. A later epoch has its own seeds and is
//! what a rotation moves the root to.

use aws_lc_rs::signature::Ed25519KeyPair;
use sha2::{Digest as _, Sha256};
use tough::key_source::KeySource;
use tough::schema::RoleType;
use tough::sign::Sign;

use crate::keys::ROLE_KEY_FILES;

/// The phrase every development seed starts with.
pub const KEY_PHRASE: &str = "kalareach-plugins development key ";

/// When the development root expires, so that the same keys always sign the same root. A host
/// never trusts a development root without the owner in a release, so a long life costs nothing.
pub const ROOT_EXPIRES: &str = "2046-10-01T00:00:00Z";

/// Returns the seed of one role's key in one epoch.
///
/// Epoch 0 is `SHA-256(KEY_PHRASE || role)`. A later epoch adds ` rotation <epoch>` after the
/// role's name.
#[must_use]
pub fn seed(role: RoleType, epoch: u32) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KEY_PHRASE.as_bytes());
    hasher.update(role.to_string().as_bytes());
    if epoch > 0 {
        hasher.update(format!(" rotation {epoch}").as_bytes());
    }
    hasher.finalize().into()
}

/// A signing key derived from a seed, which is what a development generation is signed with.
#[derive(Debug)]
pub struct DerivedKeySource {
    seed: [u8; 32],
}

impl DerivedKeySource {
    /// The key for one role in one epoch.
    #[must_use]
    pub fn new(role: RoleType, epoch: u32) -> Self {
        Self {
            seed: seed(role, epoch),
        }
    }
}

#[tough::async_trait]
impl KeySource for DerivedKeySource {
    async fn as_sign(
        &self,
    ) -> Result<Box<dyn Sign>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let pair = Ed25519KeyPair::from_seed_unchecked(&self.seed)?;
        Ok(Box::new(pair))
    }

    async fn write(
        &self,
        _value: &str,
        _key_id_hex: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        Err("a development key is derived and is never written".into())
    }
}

/// The four role keys of one epoch, in the order the pipeline's roles are listed.
#[must_use]
pub fn role_sources(epoch: u32) -> Vec<(RoleType, Box<dyn KeySource>)> {
    ROLE_KEY_FILES
        .iter()
        .map(|(role, _)| {
            (
                *role,
                Box::new(DerivedKeySource::new(*role, epoch)) as Box<dyn KeySource>,
            )
        })
        .collect()
}

/// The four role keys of one epoch, as the editor signs with them.
#[must_use]
pub fn key_sources(epoch: u32) -> Vec<Box<dyn KeySource>> {
    role_sources(epoch)
        .into_iter()
        .map(|(_, source)| source)
        .collect()
}

/// The signing material of the development lineage: derived keys, and the roots a generation
/// ships.
#[derive(Clone, Debug)]
pub struct DevelopmentSigning {
    /// The epoch whose keys sign.
    pub epoch: u32,
    /// Every root from version 1, as written.
    pub roots: Vec<Vec<u8>>,
}

impl DevelopmentSigning {
    /// The first lineage: the version-1 root alone, signed by epoch 0.
    ///
    /// # Errors
    ///
    /// Returns an error when the root cannot be built.
    pub async fn first() -> crate::Result<Self> {
        Ok(Self {
            epoch: 0,
            roots: vec![version_one_root_bytes().await?],
        })
    }

    /// The lineage a directory of numbered roots holds, signed by `epoch`.
    ///
    /// # Errors
    ///
    /// Returns an error when the roots cannot be read.
    pub fn from_directory(epoch: u32, directory: &std::path::Path) -> crate::Result<Self> {
        Ok(Self {
            epoch,
            roots: crate::lineage::numbered_roots(directory)?
                .into_values()
                .collect(),
        })
    }
}

impl crate::keys::SigningMaterial for DevelopmentSigning {
    fn key_sources(&self) -> crate::Result<Vec<Box<dyn KeySource>>> {
        Ok(key_sources(self.epoch))
    }

    fn roots(&self) -> crate::Result<Vec<Vec<u8>>> {
        Ok(self.roots.clone())
    }
}

/// The version-1 root of the development lineage, as it is written: epoch 0 keys, a threshold of
/// one for each role and the fixed expiry.
///
/// # Errors
///
/// Returns an error when the root cannot be built.
pub async fn version_one_root_bytes() -> crate::Result<Vec<u8>> {
    let expires: jiff::Timestamp = ROOT_EXPIRES.parse().map_err(|error| crate::Error::Layout {
        detail: format!("the development root's expiry is not a time: {error}"),
    })?;
    let signed = crate::keys::build_root_from(
        role_sources(0),
        std::num::NonZeroU64::new(1).expect("one is not zero"),
        expires,
    )
    .await?;
    crate::canonical::canonical(signed.signed())
}
