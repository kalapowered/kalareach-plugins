//! What a catalogue entry pins, what a host admits from, and what cannot change after signing.
//!
//! | Row | What proves it |
//! | --- | --- |
//! | KR-REQ-04.13 | every entry of the index the repository builds and of the committed development generation names a publisher this repository holds a record for, pins its source at a repository and a 40-character revision that its manifest names too, and carries the digest and length of the `plugin.json` the package holds |
//! | KR-REQ-18.06 | every entry carries the compatibility data a host admits from (bounded SDK and WIT ranges its own versions fall in, its platforms, and builds only on those platforms), and an entry that lacks any of it is refused |
//! | KR-REQ-25.21 | a generation altered after it was signed fails verification whatever the alteration is, a revocation stripped or reworded included |
//!
//! A control follows each property: the same input with the property removed is refused, so a
//! check that stopped looking would fail here.

use std::path::{Path, PathBuf};

use jiff::{Span, Timestamp};
use kalareach_catalogue::keys::SigningDirectory;
use kalareach_catalogue::packages::WithdrawalFile;
use kalareach_catalogue::tuf::{Expiries, INDEX_TARGET, TARGETS_DIR};
use kalareach_catalogue::{index, keys, packages, repository_root, tuf};
use kr_plugin_sdk::catalogue::{
    CatalogueIndex, IndexEntry, QualifiedBuild, RevocationReason, RevocationRecord,
};
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::matching::{Architecture, OperatingSystem};
use kr_plugin_sdk::scalars::TimestampMs;
use kr_plugin_sdk::text::{Label, Summary};
use kr_plugin_sdk::version::{PackageVersion, VersionRange, sdk_version, wit_version};
use serde_json::{Value, json};

/// A change made to an index, an entry (with an operating system its release does not list) or an
/// index document (with the plugin it is about), so that a control names what it removes.
type IndexChange = fn(&mut CatalogueIndex);
type EntryChange = fn(&mut IndexEntry, OperatingSystem);
type DocumentChange = fn(&mut Value, &str);

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

fn development_generation() -> PathBuf {
    root().join("snapshots/development")
}

fn fixed_time() -> TimestampMs {
    TimestampMs::new(1_760_000_000_000)
}

/// The index this repository builds from the packages it holds.
fn built_index() -> CatalogueIndex {
    let loaded = packages::load(&root()).expect("the repository loads");
    index::build(
        &loaded.repository,
        RepositoryGeneration::new(1),
        fixed_time(),
    )
}

/// Everything wrong with how the entries of `index` pin their releases, read against the
/// repository at `root` by its own files and not by anything the pipeline derived.
fn pin_problems(root: &Path, index: &CatalogueIndex) -> Vec<String> {
    let mut problems = Vec::new();
    for entry in &index.entries {
        let subject = format!("{} {}", entry.plugin_id, entry.version);
        if !index
            .publishers
            .iter()
            .any(|publisher| publisher.id == entry.publisher_id)
        {
            problems.push(format!(
                "{subject}: the index carries no record of its publisher"
            ));
        }
        let record = root
            .join("publishers")
            .join(format!("{}.json", entry.publisher_id));
        match std::fs::read(&record) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(value) if value["id"] == entry.publisher_id.as_str() => {}
                _ => problems.push(format!(
                    "{subject}: {} is not the record of {}",
                    record.display(),
                    entry.publisher_id
                )),
            },
            Err(_) => problems.push(format!(
                "{subject}: the repository holds no record of {} at {}",
                entry.publisher_id,
                record.display()
            )),
        }
        let revision = &entry.source.revision;
        if revision.len() != 40
            || !revision
                .chars()
                .all(|character| matches!(character, '0'..='9' | 'a'..='f'))
        {
            problems.push(format!(
                "{subject}: its source is pinned at {revision:?}, which is not a 40-character \
                 revision"
            ));
        }
        if !url::Url::parse(&entry.source.repository).is_ok_and(|url| url.scheme() == "https") {
            problems.push(format!(
                "{subject}: its source names {:?}, which is not an https repository",
                entry.source.repository
            ));
        }
        let manifest = root
            .join("plugins")
            .join(entry.publisher_id.as_str())
            .join(entry.plugin_name.as_str())
            .join("plugin.json");
        match std::fs::read(&manifest) {
            Ok(bytes) => {
                if PayloadDigest::of(&bytes) != entry.manifest_digest
                    || bytes.len() as u64 != entry.manifest_size_bytes.get()
                {
                    problems.push(format!(
                        "{subject}: its digest and length are not those of {}",
                        manifest.display()
                    ));
                }
                let own: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
                if own["source"]["repository"] != entry.source.repository.as_str()
                    || own["source"]["revision"] != entry.source.revision.as_str()
                {
                    problems.push(format!(
                        "{subject}: its source does not match the source {} names",
                        manifest.display()
                    ));
                }
            }
            Err(_) => problems.push(format!("{subject}: {} is not there", manifest.display())),
        }
    }
    problems
}

/// A catalogue entry pins its publisher, its source and its package: the publisher has a record in
/// this repository and in the index, the source is a repository and an exact revision, and the
/// digest and the source are those of the package's own `plugin.json`. Checked on the index the
/// packages build and on the generation this repository commits, and each property has a control
/// that removes it, the ones read from disk included.
///
/// KR-REQ-04.13.
#[tokio::test]
async fn kr_req_04_13_every_entry_pins_its_publisher_source_and_package_digest() {
    let built = built_index();
    assert!(!built.entries.is_empty());
    assert_eq!(pin_problems(&root(), &built), Vec::<String>::new());

    let committed = tuf::verify(&development_generation(), true)
        .await
        .expect("the development generation verifies")
        .index;
    assert!(!committed.entries.is_empty());
    assert_eq!(pin_problems(&root(), &committed), Vec::<String>::new());

    // Controls: each change, made to a copy of one entry, is refused for what it changes.
    let changes: [(&str, IndexChange); 5] = [
        ("carries no record of its publisher", |index| {
            index.publishers.clear();
        }),
        ("is not a 40-character revision", |index| {
            index.entries[0].source.revision = "main".to_owned();
        }),
        ("is not a 40-character revision", |index| {
            index.entries[0].source.revision = "A".repeat(40);
        }),
        ("is not an https repository", |index| {
            index.entries[0].source.repository = "kalareach-plugins".to_owned();
        }),
        ("are not those of", |index| {
            index.entries[0].manifest_digest = PayloadDigest::of(b"another manifest");
        }),
    ];
    for (why, change) in changes {
        let mut altered = built.clone();
        change(&mut altered);
        let problems = pin_problems(&root(), &altered);
        assert!(
            problems.iter().any(|problem| problem.contains(why)),
            "{why}: {problems:?}"
        );
    }
    // Controls for what is read from disk: a repository with no record of the publisher, with
    // another publisher's, or whose manifests name another source than the entries do.
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let elsewhere = temporary.path();
    for entry in &built.entries {
        let directory = elsewhere
            .join("plugins")
            .join(entry.publisher_id.as_str())
            .join(entry.plugin_name.as_str());
        std::fs::create_dir_all(&directory).expect("a package directory");
        std::fs::copy(
            root()
                .join("plugins")
                .join(entry.publisher_id.as_str())
                .join(entry.plugin_name.as_str())
                .join("plugin.json"),
            directory.join("plugin.json"),
        )
        .expect("the manifest copies");
    }
    let record = elsewhere.join("publishers/kalareach.json");
    let problems = pin_problems(elsewhere, &built);
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("holds no record of")),
        "{problems:?}"
    );
    std::fs::create_dir_all(record.parent().expect("a parent")).expect("a directory");
    std::fs::write(&record, r#"{ "id": "another" }"#).expect("the record writes");
    let problems = pin_problems(elsewhere, &built);
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("is not the record of")),
        "{problems:?}"
    );
    std::fs::copy(root().join("publishers/kalareach.json"), &record).expect("the record copies");
    assert_eq!(pin_problems(elsewhere, &built), Vec::<String>::new());
    let first = &built.entries[0];
    let manifest = elsewhere
        .join("plugins")
        .join(first.publisher_id.as_str())
        .join(first.plugin_name.as_str())
        .join("plugin.json");
    let mut own: Value =
        serde_json::from_slice(&std::fs::read(&manifest).expect("reads")).expect("JSON");
    own["source"]["revision"] = json!("0".repeat(40));
    std::fs::write(&manifest, serde_json::to_vec_pretty(&own).expect("renders")).expect("writes");
    let problems = pin_problems(elsewhere, &built);
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("its source does not match")),
        "{problems:?}"
    );

    let mut short = built.clone();
    short.entries[0].manifest_size_bytes =
        kr_plugin_sdk::digest::ByteSize::new(short.entries[0].manifest_size_bytes.get() + 1);
    assert!(
        pin_problems(&root(), &short)
            .iter()
            .any(|problem| problem.contains("are not those of")),
        "a manifest of another length"
    );
}

/// Everything wrong with the compatibility data the entries of `index` carry: a host admits an
/// adapter from its SDK and WIT ranges, its platforms and the builds it names.
fn compatibility_problems(index: &CatalogueIndex) -> Vec<String> {
    let mut problems = Vec::new();
    for entry in &index.entries {
        let subject = format!("{} {}", entry.plugin_id, entry.version);
        for (name, range, own) in [
            ("sdk_range", &entry.sdk_range, sdk_version()),
            ("wit_range", &entry.wit_range, wit_version()),
        ] {
            if range.is_unbounded() {
                problems.push(format!("{subject}: {name} {range} has no upper bound"));
            } else if !range.admits(&own) {
                problems.push(format!("{subject}: {name} {range} excludes {own}"));
            }
        }
        if entry.platforms.is_empty()
            || entry
                .platforms
                .iter()
                .any(|platform| platform.architectures.is_empty())
        {
            problems.push(format!("{subject}: it lists no platform to run on"));
        }
        if let Err(source) = entry.check_builds() {
            problems.push(format!("{subject}: {source}"));
        }
    }
    problems
}

/// An entry carries what a host admits an adapter from: SDK and WIT ranges with an upper bound that
/// this SDK and WIT are inside, the platforms it runs on, and builds only on those platforms with
/// one version for each executable. Checked on the index the packages build and on the committed
/// generation; an entry that lacks any of it is refused.
///
/// KR-REQ-18.06.
#[tokio::test]
async fn kr_req_18_06_every_entry_carries_the_compatibility_data_a_host_admits_from() {
    let built = built_index();
    assert_eq!(compatibility_problems(&built), Vec::<String>::new());
    let committed = tuf::verify(&development_generation(), true)
        .await
        .expect("the development generation verifies")
        .index;
    assert_eq!(compatibility_problems(&committed), Vec::<String>::new());

    let build = |os, architecture, version: &str| QualifiedBuild {
        application: Label::new("example-agent").expect("a label"),
        distribution: Label::new("npm @kalareach/example-agent").expect("a label"),
        version: PackageVersion::parse(version).expect("a version"),
        os,
        architecture,
        executable_digest: PayloadDigest::of(version.as_bytes()),
    };
    let entry = built.entries[0].clone();
    let listed = &entry.platforms[0];
    let (os, architecture) = (listed.os, listed.architectures[0]);

    // A build on a platform the entry lists is fine.
    let mut with_build = entry.clone();
    with_build.builds = vec![build(os, architecture, "1.4.0")];
    assert_eq!(
        compatibility_problems(&CatalogueIndex {
            entries: vec![with_build],
            ..built.clone()
        }),
        Vec::<String>::new()
    );

    // Controls: each is refused for what it lacks.
    let unlisted_os = if entry
        .platforms
        .iter()
        .any(|platform| platform.os == OperatingSystem::Windows)
    {
        OperatingSystem::MacOs
    } else {
        OperatingSystem::Windows
    };
    let changes: [(&str, EntryChange); 5] = [
        ("has no upper bound", |entry, _| {
            entry.sdk_range = VersionRange::parse(">=0.1.0").expect("a range");
        }),
        ("excludes", |entry, _| {
            entry.wit_range = VersionRange::parse(">=0.0.1, <0.0.2").expect("a range");
        }),
        ("lists no platform", |entry, _| entry.platforms.clear()),
        ("does not list among its platforms", |entry, unlisted| {
            entry.builds = vec![QualifiedBuild {
                application: Label::new("example-agent").expect("a label"),
                distribution: Label::new("npm @kalareach/example-agent").expect("a label"),
                version: PackageVersion::parse("1.4.0").expect("a version"),
                os: unlisted,
                architecture: Architecture::X86_64,
                executable_digest: PayloadDigest::of(b"1.4.0"),
            }];
        }),
        ("is named as version", |entry, _| {
            let first = entry.platforms[0].clone();
            let same = |version: &str| QualifiedBuild {
                application: Label::new("example-agent").expect("a label"),
                distribution: Label::new("npm @kalareach/example-agent").expect("a label"),
                version: PackageVersion::parse(version).expect("a version"),
                os: first.os,
                architecture: first.architectures[0],
                executable_digest: PayloadDigest::of(b"one executable"),
            };
            entry.builds = vec![same("1.4.0"), same("1.4.1")];
        }),
    ];
    for (why, change) in changes {
        let mut altered = entry.clone();
        change(&mut altered, unlisted_os);
        let problems = compatibility_problems(&CatalogueIndex {
            entries: vec![altered],
            ..built.clone()
        });
        assert!(
            problems.iter().any(|problem| problem.contains(why)),
            "{why}: {problems:?}"
        );
    }
}

/// A generation of the repository's packages, with one release withdrawn, signed with the keys the
/// run names and written under `out`.
async fn generation_with_a_revocation(out: &Path) -> (PathBuf, String) {
    let signing_dir = std::env::var_os("KALAREACH_SIGNING_DIR").unwrap_or_else(|| {
        panic!(
            "set KALAREACH_SIGNING_DIR to a directory outside the repository; \
             scripts/generate-development-keys.sh writes one"
        )
    });
    let repository_root = root();
    keys::refuse_keys_in_tree(&repository_root).expect("no key is in the tree");
    let signing = SigningDirectory::open(Path::new(&signing_dir), &repository_root)
        .expect("the signing directory opens");

    let mut loaded = packages::load(&repository_root).expect("the repository loads");
    let release = loaded
        .repository
        .packages
        .iter()
        .find(|package| package.package.manifest.plugin_name.as_str() == "example-declarative")
        .expect("the example package is published");
    let manifest = &release.package.manifest;
    let withdrawal = WithdrawalFile {
        record_version: WithdrawalFile::CURRENT_VERSION,
        publisher_id: manifest.publisher_id.clone(),
        plugin_name: manifest.plugin_name.clone(),
        version: manifest.version.clone(),
        manifest_digest: release.manifest_digest,
        record: RevocationRecord {
            reason: RevocationReason::Vulnerable,
            revoked_at: TimestampMs::new(1_760_000_100_000),
            statement: Summary::new("Withdrawn until a fixed release is published")
                .expect("a statement"),
        },
    };
    let plugin_id = manifest.plugin_id().to_string();
    loaded.repository.revocations.insert(
        (
            manifest.publisher_id.to_string(),
            manifest.plugin_name.to_string(),
            manifest.version.to_string(),
        ),
        withdrawal,
    );
    let catalogue = index::build(
        &loaded.repository,
        RepositoryGeneration::new(3),
        fixed_time(),
    );
    let expires = Timestamp::now()
        .checked_add(Span::new().hours(24))
        .expect("an hour arithmetic that fits");
    let generation = out.join("generation");
    tuf::build(
        &loaded.repository,
        &catalogue,
        &signing,
        Expiries {
            targets: expires,
            snapshot: expires,
            timestamp: expires,
        },
        &generation,
        false,
    )
    .await
    .expect("the generation signs");
    (generation, plugin_id)
}

/// A generation that carries a revocation verifies; once it is altered after it was signed it does
/// not, whether the revocation is stripped, so a withdrawn release could be installed again, or
/// reworded, or the index is changed in any other way. The control is the same document written
/// out again unedited, which verifies, so what each refusal names, a hash that is not the signed
/// one, is the alteration.
///
/// KR-REQ-25.21.
#[tokio::test]
async fn kr_req_25_21_a_revocation_altered_after_signing_fails_verification() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let (generation, plugin_id) = generation_with_a_revocation(temporary.path()).await;

    let verified = tuf::verify(&generation, true)
        .await
        .expect("the generation as it was signed verifies");
    let withdrawn = verified
        .index
        .entries
        .iter()
        .find(|entry| entry.plugin_id.as_str() == plugin_id)
        .expect("the release is in the index");
    assert!(
        !withdrawn.accepts_new_bindings(),
        "the signed index withdraws it"
    );

    let index_path = generation.join(TARGETS_DIR).join(INDEX_TARGET);
    let signed = std::fs::read(&index_path).expect("the index reads");
    let alterations: [(&str, DocumentChange); 3] = [
        ("stripped", |document, plugin| {
            for entry in document["entries"].as_array_mut().expect("entries") {
                if entry["plugin_id"] == plugin {
                    entry["revocation"] = Value::Null;
                }
            }
        }),
        ("reworded", |document, plugin| {
            for entry in document["entries"].as_array_mut().expect("entries") {
                if entry["plugin_id"] == plugin {
                    entry["revocation"]["statement"] = json!("Nothing is wrong with this release");
                }
            }
        }),
        ("with another generation number", |document, _| {
            document["generation"] = json!(4);
        }),
    ];
    // The index written out again, unedited and with the newline its rendering ends in, is the
    // signed bytes and verifies. So what each alteration below is refused for is the alteration,
    // and not a difference in how the document is written.
    let render = |document: &Value| {
        let mut bytes = serde_json::to_vec(document).expect("serialisable");
        bytes.push(b'\n');
        bytes
    };
    let unedited: Value = serde_json::from_slice(&signed).expect("an index");
    assert_eq!(
        render(&unedited),
        signed,
        "an unedited document is the signed one"
    );
    std::fs::write(&index_path, render(&unedited)).expect("the index writes");
    tuf::verify(&generation, true)
        .await
        .expect("the index written out again, unedited, verifies");
    for (what, alter) in alterations {
        let mut document: Value = serde_json::from_slice(&signed).expect("an index");
        alter(&mut document, &plugin_id);
        assert_ne!(render(&document), signed, "{what} changes the document");
        std::fs::write(&index_path, render(&document)).expect("the index writes");
        let refusal = tuf::verify(&generation, true)
            .await
            .expect_err(what)
            .to_string();
        assert!(
            refusal.contains("mismatch") || refusal.contains("Hash"),
            "{what}: {refusal}"
        );
    }

    std::fs::write(&index_path, &signed).expect("the index writes");
    tuf::verify(&generation, true)
        .await
        .expect("the generation as it was signed verifies again");
}
