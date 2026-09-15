//! Building the catalogue index.
//!
//! The index is the complete signed metadata snapshot a host synchronises. It carries what a host
//! needs to search, match and decide, and nothing it would only need after deciding.
//!
//! The build is deterministic. Entries are ordered by publisher, plugin name and version, object
//! keys are sorted and the rendering is fixed, so the same packages always produce the same bytes.
//! Without that, a signature over an index would only prove which run produced it.

use kr_plugin_sdk::catalogue::{CatalogueIndex, INDEX_VERSION, IndexEntry};
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::scalars::TimestampMs;

use crate::packages::Repository;

/// Builds the index for one generation.
///
/// `produced_at` is supplied rather than read from the clock so a caller can reproduce a
/// generation byte for byte.
#[must_use]
pub fn build(
    repository: &Repository,
    generation: RepositoryGeneration,
    produced_at: TimestampMs,
) -> CatalogueIndex {
    let mut index = CatalogueIndex {
        index_version: INDEX_VERSION,
        generation,
        produced_at,
        publishers: repository
            .publishers
            .values()
            .map(super::packages::PublisherFile::to_record)
            .collect(),
        entries: repository
            .packages
            .iter()
            .map(|loaded| {
                IndexEntry::from_manifest(&loaded.package.manifest, loaded.manifest_digest)
            })
            .collect(),
    };
    index.sort();
    index
}
