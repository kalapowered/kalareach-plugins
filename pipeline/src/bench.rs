//! The synthetic catalogue benchmark.
//!
//! Section 21 requires a catalogue of 10,000 small definitions to build, sync and search. This
//! generates that catalogue from one real package, so the entries have the shape a host actually
//! indexes rather than a shape chosen to be fast, and measures the two numbers that decide whether
//! offline search is affordable: how long the index takes to build and render, and how large it
//! is.

use std::time::{Duration, Instant};

use kr_plugin_sdk::catalogue::{CatalogueIndex, INDEX_VERSION, IndexEntry};
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::ids::{PluginName, RepositoryGeneration, plugin_id};
use kr_plugin_sdk::plugin::PluginManifest;
use kr_plugin_sdk::scalars::TimestampMs;

/// What the benchmark measured.
#[derive(Clone, Debug)]
pub struct Measurement {
    /// How many entries the index carries.
    pub entries: usize,
    /// How long building the entries took.
    pub build: Duration,
    /// How long rendering the canonical JSON took.
    pub render: Duration,
    /// How long parsing the rendered index back took.
    pub parse: Duration,
    /// How long one offline executable lookup over the whole index took.
    pub lookup: Duration,
    /// How large the rendered index is.
    pub bytes: usize,
}

impl Measurement {
    /// Renders the measurement as the lines a benchmark log carries.
    ///
    /// The last line is an index-only estimate. A repository has two default limits, 64 MiB of
    /// metadata and 100,000 entries, and whichever binds first is the one a host reports when a
    /// sync runs out of room. The estimate counts the index and nothing else: the TUF metadata over
    /// it grows with the target count too, so the real figure is lower.
    ///
    /// Every entry comes from one template, so the size per entry is the size of that package's
    /// metadata. A catalogue of longer descriptions and more match rules costs more per entry.
    #[must_use]
    pub fn report(&self) -> String {
        let per_entry = if self.entries == 0 {
            0.0
        } else {
            self.bytes as f64 / self.entries as f64
        };
        let fits = if per_entry > 0.0 {
            (kr_plugin_sdk::limits::METADATA_BUDGET_BYTES as f64 / per_entry) as u64
        } else {
            0
        };
        format!(
            "entries              {}\n\
             build                {:.3} s\n\
             render               {:.3} s\n\
             parse                {:.3} s\n\
             offline lookup       {:.3} s\n\
             index size           {} bytes ({:.1} MiB)\n\
             bytes per entry      {per_entry:.0}\n\
             index fits 64 MiB    {fits} entries\n",
            self.entries,
            self.build.as_secs_f64(),
            self.render.as_secs_f64(),
            self.parse.as_secs_f64(),
            self.lookup.as_secs_f64(),
            self.bytes,
            self.bytes as f64 / (1024.0 * 1024.0),
        )
    }
}

/// Builds a synthetic index of `entries` packages from `template` and measures it.
///
/// # Panics
///
/// Panics when the generated index cannot be rendered or parsed, which would mean the index type
/// is malformed rather than a runtime condition.
#[must_use]
pub fn measure(template: &PluginManifest, entries: usize) -> Measurement {
    let manifest_size_bytes = serde_json::to_vec(template)
        .expect("the template manifest is serialisable")
        .len() as u64;
    let started = Instant::now();
    let mut index = CatalogueIndex {
        index_version: INDEX_VERSION,
        generation: RepositoryGeneration::new(1),
        produced_at: TimestampMs::new(1_760_000_000_000),
        publishers: Vec::new(),
        entries: Vec::with_capacity(entries),
    };
    for ordinal in 0..entries {
        let mut manifest = template.clone();
        manifest.plugin_name =
            PluginName::new(format!("synthetic-{ordinal:05}")).expect("a generated slug is valid");
        let mut entry = IndexEntry::from_manifest(
            &manifest,
            PayloadDigest::of(format!("synthetic-{ordinal}").as_bytes()),
            manifest_size_bytes,
        );
        entry.plugin_id = plugin_id(&manifest.publisher_id, &manifest.plugin_name);
        index.entries.push(entry);
    }
    index.sort();
    let build = started.elapsed();

    let started = Instant::now();
    let rendered = index.canonical_json().expect("the index is serialisable");
    let render = started.elapsed();

    let started = Instant::now();
    let parsed: CatalogueIndex =
        serde_json::from_str(&rendered).expect("the rendered index parses back");
    let parse = started.elapsed();

    // The lookup path comes from the template's own match rule, so the measurement is the template
    // finding itself rather than a name this function happens to know.
    // The lookup path comes from the template's own match rule, suffix included, so the
    // measurement is that rule finding itself rather than a name this function happens to know.
    let executable = template.match_rules.first().map_or_else(
        || "/usr/local/bin/example-agent".to_owned(),
        |rule| {
            let mut path = String::from("/usr/local/bin");
            for segment in &rule.executable.path_suffix {
                path.push('/');
                path.push_str(segment);
            }
            path.push('/');
            path.push_str(&rule.executable.file_stem);
            path
        },
    );
    let started = Instant::now();
    let matches = parsed.matching_executable(&executable);
    let lookup = started.elapsed();
    assert_eq!(matches.len(), entries, "every synthetic entry matches");

    Measurement {
        entries,
        build,
        render,
        parse,
        lookup,
        bytes: rendered.len(),
    }
}
