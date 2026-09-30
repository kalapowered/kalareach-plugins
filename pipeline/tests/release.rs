//! The index build is deterministic, and the signed generation verifies.

use std::path::{Path, PathBuf};

use jiff::{Span, Timestamp};
use kalareach_catalogue::keys::SigningDirectory;
use kalareach_catalogue::tuf::{Expiries, INDEX_TARGET, METADATA_DIR, TARGETS_DIR};
use kalareach_catalogue::{index, keys, packages, repository_root, tuf};
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::plugin::PayloadRole;
use kr_plugin_sdk::scalars::TimestampMs;

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

/// The committed development generation.
fn development_generation() -> PathBuf {
    root().join("snapshots/development")
}

/// The target path one package payload has in a generation, as the index names it.
///
/// Derived from the package the generation was built from rather than written out here, so a
/// package that publishes a new version does not silently stop being the target a test edits.
fn target_path(plugin_id: &str, role: PayloadRole) -> String {
    let loaded = packages::load(&root()).expect("the repository loads");
    let package = loaded
        .repository
        .packages
        .iter()
        .find(|package| package.package.manifest.plugin_id().as_str() == plugin_id)
        .unwrap_or_else(|| panic!("{plugin_id} is published by this repository"));
    let manifest = &package.package.manifest;
    let payload = manifest
        .payload(role)
        .unwrap_or_else(|| panic!("{plugin_id} declares a {role:?} payload"));
    format!(
        "packages/{}/{}/{}/{}",
        manifest.publisher_id, manifest.plugin_name, manifest.version, payload.path
    )
}

fn fixed_time() -> TimestampMs {
    TimestampMs::new(1_760_000_000_000)
}

#[test]
fn the_index_build_is_byte_identical_on_repeat() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let first = index::build(
        &loaded.repository,
        RepositoryGeneration::new(1),
        fixed_time(),
    );
    let second = index::build(
        &loaded.repository,
        RepositoryGeneration::new(1),
        fixed_time(),
    );
    let first = first.canonical_json().expect("the index renders");
    let second = second.canonical_json().expect("the index renders");
    assert_eq!(first, second);
    assert!(first.ends_with('\n'));
}

#[test]
fn the_index_carries_every_package_with_its_payload_hashes() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let index = index::build(
        &loaded.repository,
        RepositoryGeneration::new(1),
        fixed_time(),
    );
    assert_eq!(index.entries.len(), loaded.repository.packages.len());
    for entry in &index.entries {
        assert!(
            !entry.payloads.is_empty(),
            "{} has no payloads",
            entry.plugin_id
        );
        assert!(entry.total_size_bytes.get() > 0);
        assert!(entry.accepts_new_bindings());
    }
    // Entries are ordered by publisher, plugin name and version.
    let keys: Vec<_> = index
        .entries
        .iter()
        .map(kr_plugin_sdk::catalogue::IndexEntry::sort_key)
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted);
}

/// KR-REQ-25.21: the generation this repository commits verifies as a host verifies it, before any
/// package of it is read.
#[tokio::test]
async fn the_committed_development_generation_verifies() {
    let verified = tuf::verify(&development_generation(), true)
        .await
        .expect("the development generation verifies");
    assert!(verified.target_count > 0);
    assert!(!verified.index.entries.is_empty());
    assert_eq!(verified.index.generation, RepositoryGeneration::new(1));
}

#[tokio::test]
async fn the_development_generation_matches_the_packages_it_was_built_from() {
    let verified = tuf::verify(&development_generation(), true)
        .await
        .expect("the development generation verifies");
    let loaded = packages::load(&root()).expect("the repository loads");
    let rebuilt = index::build(
        &loaded.repository,
        verified.index.generation,
        verified.index.produced_at,
    );
    assert_eq!(
        rebuilt.canonical_json().expect("the index renders"),
        verified.index.canonical_json().expect("the index renders"),
        "snapshots/development is stale; rebuild it as snapshots/README.md describes"
    );
}

/// KR-REQ-25.21: an index altered after it was signed does not verify.
#[tokio::test]
async fn a_tampered_target_fails_verification() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let generation = temporary.path().join("generation");
    copy_tree(&development_generation(), &generation);

    // Verification passes before the change, so the failure after it is the change.
    tuf::verify(&generation, true)
        .await
        .expect("the copy verifies");

    let index = generation.join(TARGETS_DIR).join(INDEX_TARGET);
    let mut text = std::fs::read_to_string(&index).expect("the index reads");
    text = text.replace("Recognises", "Replaces");
    std::fs::write(&index, text).expect("the index writes");

    let error = tuf::verify(&generation, true)
        .await
        .expect_err("a tampered target does not verify");
    let message = error.to_string();
    assert!(
        message.contains("Hash mismatch") || message.contains("mismatch"),
        "{message}"
    );
}

/// KR-REQ-25.21: metadata altered after it was signed does not verify.
#[tokio::test]
async fn a_replaced_metadata_signature_fails_verification() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let generation = temporary.path().join("generation");
    copy_tree(&development_generation(), &generation);

    let targets = generation.join(METADATA_DIR).join("targets.json");
    let mut document: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&targets).expect("the metadata reads"))
            .expect("the metadata parses");
    document["signed"]["version"] = serde_json::json!(2);
    std::fs::write(
        &targets,
        serde_json::to_string_pretty(&document).expect("the metadata renders"),
    )
    .expect("the metadata writes");

    tuf::verify(&generation, true)
        .await
        .expect_err("metadata whose signature no longer covers it does not verify");
}

#[tokio::test]
async fn a_generation_signs_and_verifies_end_to_end() {
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

    let loaded = packages::load(&repository_root).expect("the repository loads");
    let catalogue = index::build(
        &loaded.repository,
        RepositoryGeneration::new(7),
        fixed_time(),
    );

    let temporary = tempfile::tempdir().expect("a temporary directory");
    let out = temporary.path().join("generation");
    let expires = Timestamp::now()
        .checked_add(Span::new().hours(24))
        .expect("an hour arithmetic that fits");
    let outcome = tuf::build(
        &loaded.repository,
        &catalogue,
        &signing,
        Expiries {
            targets: expires,
            snapshot: expires,
            timestamp: expires,
        },
        &out,
        false,
    )
    .await
    .expect("the generation signs");

    let verified = tuf::verify(&out, true)
        .await
        .expect("the generation verifies");
    assert_eq!(verified.target_count, outcome.target_count);
    assert_eq!(verified.index.generation, RepositoryGeneration::new(7));

    // The metadata version is the generation, so a client that already trusts generation 7 rejects
    // an older one rather than accepting it as an update.
    let targets: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out.join(METADATA_DIR).join("targets.json"))
            .expect("the metadata reads"),
    )
    .expect("the metadata parses");
    assert_eq!(targets["signed"]["version"], 7);

    // A generation is written once.
    let refused = tuf::build(
        &loaded.repository,
        &catalogue,
        &signing,
        Expiries {
            targets: expires,
            snapshot: expires,
            timestamp: expires,
        },
        &out,
        false,
    )
    .await
    .expect_err("a second build into the same directory is refused");
    assert!(refused.to_string().contains("written once"), "{refused}");
}

/// KR-REQ-25.21: a package file swapped for another signed file of the same generation does not
/// verify, so a package is only ever the bytes its index names.
#[tokio::test]
async fn a_replaced_package_payload_fails_verification() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let generation = temporary.path().join("generation");
    copy_tree(&development_generation(), &generation);
    tuf::verify(&generation, true)
        .await
        .expect("the copy verifies");

    // One package's document replaced by another's. Both are signed targets of this generation, so
    // only reading each target under the name the index declares catches it.
    let targets = generation.join(TARGETS_DIR);
    let from = targets.join(target_path("kalareach/codex", PayloadRole::Presentation));
    let to = targets.join(target_path("kalareach/opencode", PayloadRole::Presentation));
    std::fs::copy(&from, &to).expect("the document copies");

    tuf::verify(&generation, true)
        .await
        .expect_err("a swapped package payload does not verify");
}

/// KR-REQ-25.21: a generation that lacks a file its index names does not verify.
#[tokio::test]
async fn a_missing_package_payload_fails_verification() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let generation = temporary.path().join("generation");
    copy_tree(&development_generation(), &generation);
    std::fs::remove_file(
        generation
            .join(TARGETS_DIR)
            .join(target_path("kalareach/codex", PayloadRole::Asset)),
    )
    .expect("the asset is removed");

    tuf::verify(&generation, true)
        .await
        .expect_err("a missing package payload does not verify");
}

#[test]
fn a_private_key_in_the_tree_stops_the_pipeline() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let tree = temporary.path();
    std::fs::write(tree.join("notes.txt"), "nothing to see").expect("the file writes");
    keys::refuse_keys_in_tree(tree).expect("an ordinary tree is fine");

    // A key renamed to look like anything else is still a key: the scan reads the header.
    let header = format!(
        "-----BEGIN {} KEY-----\nMIIB\n-----END {} KEY-----\n",
        "PRIVATE", "PRIVATE"
    );
    std::fs::write(tree.join("notes.txt"), &header).expect("the file writes");
    let error = keys::refuse_keys_in_tree(tree).expect_err("a key in the tree stops the pipeline");
    assert!(error.to_string().contains("private signing key"));

    // A key pasted a long way into a file, and a key pasted inside a value, are both keys.
    let buried = format!("{}{header}", "x\n".repeat(200_000));
    std::fs::write(tree.join("notes.txt"), buried).expect("the file writes");
    keys::refuse_keys_in_tree(tree).expect_err("a key past the first few kilobytes is still a key");

    let embedded = format!(
        "{{\"key\": \"-----BEGIN {} KEY-----MIIB-----END {} KEY-----\"}}\n",
        "PRIVATE", "PRIVATE"
    );
    std::fs::write(tree.join("notes.txt"), embedded).expect("the file writes");
    keys::refuse_keys_in_tree(tree).expect_err("a key inside a value is still a key");

    std::fs::write(tree.join("notes.txt"), "nothing to see").expect("the file writes");
    std::fs::write(tree.join("root.pem"), "not really a key").expect("the file writes");
    keys::refuse_keys_in_tree(tree).expect_err("a .pem file in the tree stops the pipeline");
}

#[tokio::test]
async fn one_key_cannot_hold_every_role() {
    let Some(signing_dir) = std::env::var_os("KALAREACH_SIGNING_DIR") else {
        return;
    };
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let reused = temporary.path().join("signing");
    std::fs::create_dir_all(&reused).expect("the directory creates");
    let source = Path::new(&signing_dir).join("root.pem");
    for role in ["root.pem", "targets.pem", "snapshot.pem", "timestamp.pem"] {
        std::fs::copy(&source, reused.join(role)).expect("the key copies");
    }

    let signing = SigningDirectory::open(&reused, &root()).expect("the directory opens");
    let expires = Timestamp::now()
        .checked_add(Span::new().hours(24))
        .expect("an hour arithmetic that fits");
    let error = keys::build_root(&signing, expires)
        .await
        .expect_err("one key for four roles is refused");
    assert!(error.to_string().contains("same key"), "{error}");
}

#[tokio::test]
async fn a_build_refuses_a_destination_that_is_not_a_generation() {
    let Some(signing_dir) = std::env::var_os("KALAREACH_SIGNING_DIR") else {
        return;
    };
    let repository_root = root();
    let signing = SigningDirectory::open(Path::new(&signing_dir), &repository_root)
        .expect("the signing directory opens");
    let loaded = packages::load(&repository_root).expect("the repository loads");
    let catalogue = index::build(
        &loaded.repository,
        RepositoryGeneration::new(1),
        fixed_time(),
    );
    let expires = Timestamp::now()
        .checked_add(Span::new().hours(24))
        .expect("an hour arithmetic that fits");
    let expiries = Expiries {
        targets: expires,
        snapshot: expires,
        timestamp: expires,
    };

    // A directory that holds something other than a generation is refused rather than emptied.
    // Pointing a build at a signing directory or a checkout is a mistake that stops here.
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let precious = temporary.path().join("precious");
    std::fs::create_dir_all(&precious).expect("the directory creates");
    std::fs::write(precious.join("root.pem"), "a key").expect("the file writes");

    let refused = tuf::build(
        &loaded.repository,
        &catalogue,
        &signing,
        expiries,
        &precious,
        true,
    )
    .await
    .expect_err("a directory that is not a generation is refused");
    assert!(
        refused.to_string().contains("not a generation"),
        "{refused}"
    );
    assert!(
        precious.join("root.pem").is_file(),
        "the directory was emptied"
    );
}

#[tokio::test]
async fn a_replaced_generation_is_kept_rather_than_removed() {
    let Some(signing_dir) = std::env::var_os("KALAREACH_SIGNING_DIR") else {
        return;
    };
    let repository_root = root();
    let signing = SigningDirectory::open(Path::new(&signing_dir), &repository_root)
        .expect("the signing directory opens");
    let loaded = packages::load(&repository_root).expect("the repository loads");
    let expires = Timestamp::now()
        .checked_add(Span::new().hours(24))
        .expect("an hour arithmetic that fits");
    let expiries = Expiries {
        targets: expires,
        snapshot: expires,
        timestamp: expires,
    };
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let out = temporary.path().join("generation");

    let first_index = index::build(
        &loaded.repository,
        RepositoryGeneration::new(1),
        fixed_time(),
    );
    let first = tuf::build(
        &loaded.repository,
        &first_index,
        &signing,
        expiries,
        &out,
        false,
    )
    .await
    .expect("the first generation signs");
    assert!(first.retired.is_none());

    let second_index = index::build(
        &loaded.repository,
        RepositoryGeneration::new(2),
        fixed_time(),
    );
    let second = tuf::build(
        &loaded.repository,
        &second_index,
        &signing,
        expiries,
        &out,
        true,
    )
    .await
    .expect("the second generation signs");

    // The generation it replaced is still there, under its own name, with its metadata intact.
    let retired = second.retired.expect("the replaced generation was kept");
    assert!(retired.join("root.json").is_file(), "{}", retired.display());
    let verified = tuf::verify(&out, true).await.expect("the new one verifies");
    assert_eq!(verified.index.generation, RepositoryGeneration::new(2));
    let previous = tuf::verify(&retired, true)
        .await
        .expect("the replaced one still verifies");
    assert_eq!(previous.index.generation, RepositoryGeneration::new(1));
}

#[tokio::test]
async fn a_build_refuses_generation_zero_before_it_writes() {
    let Some(signing_dir) = std::env::var_os("KALAREACH_SIGNING_DIR") else {
        return;
    };
    let repository_root = root();
    let signing = SigningDirectory::open(Path::new(&signing_dir), &repository_root)
        .expect("the signing directory opens");
    let loaded = packages::load(&repository_root).expect("the repository loads");
    let catalogue = index::build(
        &loaded.repository,
        RepositoryGeneration::new(0),
        fixed_time(),
    );
    let expires = Timestamp::now()
        .checked_add(Span::new().hours(24))
        .expect("an hour arithmetic that fits");
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let out = temporary.path().join("generation");

    let refused = tuf::build(
        &loaded.repository,
        &catalogue,
        &signing,
        Expiries {
            targets: expires,
            snapshot: expires,
            timestamp: expires,
        },
        &out,
        true,
    )
    .await
    .expect_err("generation zero is refused");
    assert!(refused.to_string().contains("starts at 1"), "{refused}");
    assert!(!out.exists(), "the destination was written to");
}

#[cfg(unix)]
#[test]
fn a_broken_checkout_marker_is_still_a_checkout() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let tree = temporary.path().join("tree");
    std::fs::create_dir_all(&tree).expect("the directory creates");
    // A `.git` that points at nothing. Reading it fails; its presence still says this is a
    // checkout, and the scan must ask Git rather than fall back to the coarser walk.
    std::os::unix::fs::symlink(tree.join("missing"), tree.join(".git")).expect("the link is made");

    let error =
        keys::refuse_keys_in_tree(&tree).expect_err("a checkout Git cannot list stops the scan");
    assert!(error.to_string().contains("git could not list"), "{error}");
}

#[test]
fn a_signing_directory_inside_the_repository_is_refused() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path().join("repository");
    let inside = repository.join("signing");
    std::fs::create_dir_all(&inside).expect("the directories create");
    let error = SigningDirectory::open(&inside, &repository)
        .expect_err("a signing directory inside the repository is refused");
    assert!(error.to_string().contains("outside the repository"));
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the destination directory");
    for entry in std::fs::read_dir(from).expect("the source directory reads") {
        let entry = entry.expect("a readable entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("the file copies");
        }
    }
}
