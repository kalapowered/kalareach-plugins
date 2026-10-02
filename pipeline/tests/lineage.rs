//! The development lineage: keys anybody can derive, one chain of roots, and metadata that is
//! written the same way every time.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use kalareach_catalogue::development::{self, DevelopmentSigning};
use kalareach_catalogue::keys::{self, SigningMaterial};
use kalareach_catalogue::lineage::{self, BREAK_FROM_DIGEST, Lineage};
use kalareach_catalogue::tuf::{self, Expiries, METADATA_DIR};
use kalareach_catalogue::{canonical, index, packages, repository_root};
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::scalars::TimestampMs;
use sha2::{Digest as _, Sha256};
use tough::schema::{RoleType, Root, Signed};

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

fn expires() -> Timestamp {
    development::ROOT_EXPIRES.parse().expect("a time")
}

/// Builds one generation of the repository's packages under `signing` into a fresh directory.
async fn generation(
    temporary: &Path,
    name: &str,
    number: u64,
    signing: &dyn SigningMaterial,
) -> PathBuf {
    let loaded = packages::load(&root()).expect("the repository loads");
    let catalogue = index::build(
        &loaded.repository,
        RepositoryGeneration::new(number),
        TimestampMs::new(1_760_000_000_000),
    );
    let out = temporary.join(name);
    tuf::build(
        &loaded.repository,
        &catalogue,
        signing,
        Expiries {
            targets: expires(),
            snapshot: expires(),
            timestamp: expires(),
        },
        &out,
        false,
    )
    .await
    .expect("the generation builds");
    out
}

/// The roots of the development lineage up to `epochs` rotations: version 1 under epoch 0, and
/// each later one signed under the keys before it and its own.
async fn chain(epochs: u32) -> Vec<Vec<u8>> {
    let mut roots = vec![
        development::version_one_root_bytes()
            .await
            .expect("the first root builds"),
    ];
    for epoch in 0..epochs {
        let previous: Signed<Root> =
            serde_json::from_slice(roots.last().expect("a root")).expect("the root before parses");
        let next = keys::rotate_root(
            &previous,
            &development::key_sources(epoch),
            development::role_sources(epoch + 1),
            expires(),
        )
        .await
        .expect("the rotation signs");
        roots.push(canonical::canonical(&next).expect("the root renders"));
    }
    roots
}

fn signing(epoch: u32, roots: Vec<Vec<u8>>) -> DevelopmentSigning {
    DevelopmentSigning { epoch, roots }
}

/// Every file under a directory, by relative path.
fn files(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![directory.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(&current).expect("the directory reads") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.insert(
                    path.strip_prefix(directory)
                        .expect("inside")
                        .to_string_lossy()
                        .into_owned(),
                    std::fs::read(&path).expect("the file reads"),
                );
            }
        }
    }
    found
}

fn key_id(root: &Signed<Root>, role: RoleType) -> String {
    hex::encode(&root.signed.roles[&role].keyids[0])
}

/// The development keys are the ones the published phrase derives: the identifiers the core
/// repository commits for them are these.
#[tokio::test]
async fn the_development_keys_are_the_ones_the_phrase_derives() {
    let bytes = development::version_one_root_bytes()
        .await
        .expect("the first root builds");
    let signed: Signed<Root> = serde_json::from_slice(&bytes).expect("the root parses");
    assert_eq!(
        [
            key_id(&signed, RoleType::Root),
            key_id(&signed, RoleType::Targets),
            key_id(&signed, RoleType::Snapshot),
            key_id(&signed, RoleType::Timestamp),
        ],
        [
            "22a99bda605726730776823e26da03b178524617baccaee1a75ad8a67fb41eca",
            "5547449f217effa345d4137e813ec466c94b64323476e0cab31e9f7598bb0361",
            "770e80f30b806de2f71dda174de757d6e17afeb9a8c03027cb14699cddea15fb",
            "9cc51d4b21c811ec08795b6a3cb28ff97704a640ec3abf7bfd600021cc80bb51",
        ],
        "the identifiers a host's build commits for the development lineage"
    );
    // The seed is the digest of the phrase and the role, and a later epoch has seeds of its own.
    let expected: [u8; 32] = Sha256::digest(b"kalareach-plugins development key root").into();
    assert_eq!(development::seed(RoleType::Root, 0), expected);
    assert_ne!(
        development::seed(RoleType::Root, 1),
        development::seed(RoleType::Root, 0)
    );
    // The same four keys sign every time: the root is the same bytes on each build.
    assert_eq!(
        bytes,
        development::version_one_root_bytes()
            .await
            .expect("the first root builds again")
    );
}

/// The committed generation is the development lineage's: its version-1 root is the derived one,
/// and building it again from the packages writes every file as committed.
#[tokio::test]
async fn the_committed_development_generation_is_what_the_pipeline_writes() {
    let committed = root().join("snapshots/development");
    let derived = development::version_one_root_bytes()
        .await
        .expect("the first root builds");
    assert_eq!(
        std::fs::read(committed.join(METADATA_DIR).join("1.root.json")).expect("the root reads"),
        derived
    );
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let verified = tuf::verify(&committed, true)
        .await
        .expect("the committed generation verifies");
    let rebuilt = generation(
        temporary.path(),
        "rebuilt",
        verified.index.generation.get(),
        &signing(0, vec![derived]),
    )
    .await;
    assert_eq!(
        files(&rebuilt),
        files(&committed),
        "snapshots/development is stale; rebuild it as snapshots/README.md describes"
    );
}

/// KR-REQ-25.21: a rebuild of the same inputs writes the same bytes, in separate processes, so a
/// reviewer can rebuild a generation and compare it with what is committed.
#[test]
fn a_rebuild_in_another_process_writes_the_same_files() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let build = |name: &str| {
        let out = temporary.path().join(name);
        let status = std::process::Command::new(env!("CARGO_BIN_EXE_kalareach-catalogue"))
            // A signing directory in the environment would name a second thing to sign with.
            .env_remove("KALAREACH_SIGNING_DIR")
            .args(["--repository"])
            .arg(root())
            .args([
                "build",
                "--development",
                "--generation",
                "2",
                "--produced-at",
                "1760000000000",
                "--expires-at",
                development::ROOT_EXPIRES,
                "--out",
            ])
            .arg(&out)
            .output()
            .expect("the pipeline runs");
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        files(&out)
    };
    let first = build("first");
    let second = build("second");
    assert_eq!(first, second);
    assert!(first.contains_key("metadata/targets.json"));
}

/// KR-REQ-25.21: every signed role is written with its members in name order and its signatures in
/// key identifier order, so the bytes do not depend on the process that wrote them.
#[tokio::test]
async fn metadata_is_written_in_name_order_with_signatures_in_key_order() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let roots = chain(1).await;
    let built = generation(temporary.path(), "g", 2, &signing(1, roots.clone())).await;
    let mut checked = 0;
    for name in [
        "targets.json",
        "snapshot.json",
        "timestamp.json",
        "1.root.json",
        "2.root.json",
        "root.json",
    ] {
        let bytes = std::fs::read(built.join(METADATA_DIR).join(name)).expect("the file reads");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("the file parses");
        let mut again = serde_json::to_vec_pretty(&value).expect("the file renders");
        again.push(b'\n');
        assert_eq!(bytes, again, "{name} is not in name order");
        let keyids: Vec<&str> = value["signatures"]
            .as_array()
            .expect("signatures")
            .iter()
            .map(|signature| signature["keyid"].as_str().expect("a key identifier"))
            .collect();
        let mut sorted = keyids.clone();
        sorted.sort_unstable();
        assert_eq!(keyids, sorted, "{name}'s signatures are not in key order");
        checked += 1;
    }
    assert_eq!(checked, 6);
    // The rotation carries a signature of each root, and the files pin each other's bytes.
    let second: serde_json::Value = serde_json::from_slice(
        &std::fs::read(built.join(METADATA_DIR).join("2.root.json")).expect("the root reads"),
    )
    .expect("the root parses");
    assert_eq!(
        second["signatures"].as_array().expect("signatures").len(),
        2
    );
    tuf::verify(&built, true)
        .await
        .expect("the generation verifies as a host verifies it");
}

/// A generation that follows with the same root is accepted.
#[tokio::test]
async fn a_generation_with_the_same_root_follows_its_predecessor() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let roots = chain(0).await;
    let first = generation(temporary.path(), "one", 1, &signing(0, roots.clone())).await;
    let second = generation(temporary.path(), "two", 2, &signing(0, roots)).await;
    assert_eq!(
        lineage::check_generation_from(&second, &first, "none")
            .await
            .expect("it follows"),
        Lineage::Same
    );
    // A generation is higher than the one before it.
    let refused = lineage::check_generation_from(&second, &second, "none")
        .await
        .expect_err("the same generation does not follow itself");
    assert!(refused.to_string().contains("higher"), "{refused}");
    let refused = lineage::check_generation_from(&first, &second, "none")
        .await
        .expect_err("an older generation does not follow a newer one");
    assert!(refused.to_string().contains("higher"), "{refused}");
}

/// A rotation signed under a threshold of the root before it and a threshold of its own is
/// accepted, and so is the chain after it.
#[tokio::test]
async fn a_rotation_signed_under_both_roots_is_accepted() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let roots = chain(2).await;
    let first = generation(temporary.path(), "one", 1, &signing(0, roots[..1].to_vec())).await;
    let second = generation(temporary.path(), "two", 2, &signing(1, roots[..2].to_vec())).await;
    let third = generation(temporary.path(), "three", 3, &signing(2, roots.clone())).await;
    assert_eq!(
        lineage::check_generation_from(&second, &first, "none")
            .await
            .expect("a rotation follows"),
        Lineage::Chain { from: 1, to: 2 }
    );
    assert_eq!(
        lineage::check_generation_from(&third, &second, "none")
            .await
            .expect("the next rotation follows"),
        Lineage::Chain { from: 2, to: 3 }
    );
    // Two rotations at once, from the first, are accepted too.
    assert_eq!(
        lineage::check_generation_from(&third, &first, "none")
            .await
            .expect("both rotations follow"),
        Lineage::Chain { from: 1, to: 3 }
    );
}

/// A root that lacks either signature is refused: one under the keys before it only, and one under
/// its own only.
#[tokio::test]
async fn a_rotation_without_the_signature_of_either_root_is_refused() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let first_roots = chain(0).await;
    let first = generation(temporary.path(), "one", 1, &signing(0, first_roots.clone())).await;
    let previous: Signed<Root> = serde_json::from_slice(&first_roots[0]).expect("the root parses");

    // Signed by its own keys alone: a host that trusts the root before would not follow it.
    let own_only = keys::build_root_from(
        development::role_sources(1),
        std::num::NonZeroU64::new(2).expect("two"),
        expires(),
    )
    .await
    .expect("the root builds");
    let mut roots = first_roots.clone();
    roots.push(canonical::canonical(own_only.signed()).expect("the root renders"));
    let second = generation(temporary.path(), "own", 2, &signing(1, roots)).await;
    let refused = lineage::check_generation_from(&second, &first, "none")
        .await
        .expect_err("a rotation the root before did not sign is refused");
    assert!(
        refused
            .to_string()
            .contains("does not carry a threshold of 1.root.json's root keys"),
        "{refused}"
    );

    // Signed by the keys before it alone: it does not carry its own. The library refuses to build
    // a generation under it, so a valid generation's roots are replaced by it.
    let full = chain(1).await;
    let next_root = {
        let signed: Signed<Root> = serde_json::from_slice(&full[1]).expect("the root parses");
        signed.signed
    };
    let under_previous = tough::editor::signed::SignedRole::new(
        next_root,
        &tough::schema::KeyHolder::Root(previous.signed.clone()),
        &development::key_sources(0),
        &aws_lc_rs::rand::SystemRandom::new(),
    )
    .await
    .expect("the root signs");
    let third = generation(temporary.path(), "previous", 2, &signing(1, full)).await;
    let old_only = canonical::canonical(under_previous.signed()).expect("the root renders");
    for path in [
        third.join(METADATA_DIR).join("2.root.json"),
        third.join(METADATA_DIR).join("root.json"),
        third.join("root.json"),
    ] {
        std::fs::write(path, &old_only).expect("the root is replaced");
    }
    lineage::check_generation_from(&third, &first, "none")
        .await
        .expect_err("a rotation that does not carry its own signature is refused");
}

/// A generation that ships roots up to a version and not one before it is refused: a host that
/// adopted an earlier root could not follow the chain.
#[tokio::test]
async fn a_chain_with_a_missing_root_is_refused() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let roots = chain(2).await;
    let first = generation(temporary.path(), "one", 1, &signing(0, roots[..1].to_vec())).await;
    let third = generation(temporary.path(), "three", 3, &signing(2, roots)).await;
    std::fs::remove_file(third.join(METADATA_DIR).join("2.root.json")).expect("the root goes");
    let refused = lineage::check_generation_from(&third, &first, "none")
        .await
        .expect_err("a chain with a gap is refused");
    assert!(
        refused
            .to_string()
            .contains("ships roots up to version 3 and not 2"),
        "{refused}"
    );
}

/// A generation that rewrites a root its predecessor shipped is refused, whichever root.
#[tokio::test]
async fn a_generation_that_rewrites_an_earlier_root_is_refused() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let roots = chain(0).await;
    let first = generation(temporary.path(), "one", 1, &signing(0, roots)).await;
    // Another lineage entirely, from its own first root.
    let other = keys::build_root_from(
        development::role_sources(5),
        std::num::NonZeroU64::new(1).expect("one"),
        expires(),
    )
    .await
    .expect("the root builds");
    let other_roots = vec![canonical::canonical(other.signed()).expect("the root renders")];
    let second = generation(temporary.path(), "two", 2, &signing(5, other_roots)).await;
    let refused = lineage::check_generation_from(&second, &first, "none")
        .await
        .expect_err("a different lineage is refused");
    assert!(refused.to_string().contains("rewrites a root"), "{refused}");
}

/// The break works from the one predecessor the pipeline names, and to the development lineage's
/// version-1 root, and from nothing else.
#[tokio::test]
async fn the_break_is_accepted_from_its_predecessor_and_to_the_derived_root_only() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    // A predecessor of another lineage, whose digest this test names as the one to break from.
    let before = keys::build_root_from(
        development::role_sources(7),
        std::num::NonZeroU64::new(1).expect("one"),
        expires(),
    )
    .await
    .expect("the root builds");
    let before_bytes = canonical::canonical(before.signed()).expect("the root renders");
    let previous = generation(
        temporary.path(),
        "previous",
        1,
        &signing(7, vec![before_bytes.clone()]),
    )
    .await;
    let digest = hex::encode(Sha256::digest(&before_bytes));

    // To the derived version-1 root.
    let derived = chain(0).await;
    let to_derived = generation(temporary.path(), "derived", 2, &signing(0, derived)).await;
    assert_eq!(
        lineage::check_generation_from(&to_derived, &previous, &digest)
            .await
            .expect("the break is accepted"),
        Lineage::Break
    );
    // A first generation that also rotates is accepted from the same predecessor.
    let rotated = chain(1).await;
    let and_rotates = generation(temporary.path(), "rotates", 2, &signing(1, rotated)).await;
    assert_eq!(
        lineage::check_generation_from(&and_rotates, &previous, &digest)
            .await
            .expect("the break and a rotation are accepted"),
        Lineage::Break
    );
    // From any other predecessor it is refused.
    let refused = lineage::check_generation_from(&to_derived, &previous, BREAK_FROM_DIGEST)
        .await
        .expect_err("the break works from one predecessor only");
    assert!(
        refused
            .to_string()
            .contains("not the one the lineage may break from"),
        "{refused}"
    );
    // And to nothing but the derived root.
    let elsewhere = keys::build_root_from(
        development::role_sources(8),
        std::num::NonZeroU64::new(1).expect("one"),
        expires(),
    )
    .await
    .expect("the root builds");
    let elsewhere_roots = vec![canonical::canonical(elsewhere.signed()).expect("the root renders")];
    let to_elsewhere = generation(
        temporary.path(),
        "elsewhere",
        2,
        &signing(8, elsewhere_roots),
    )
    .await;
    let refused = lineage::check_generation_from(&to_elsewhere, &previous, &digest)
        .await
        .expect_err("the break goes to the derived root only");
    assert!(refused.to_string().contains("version-1 root"), "{refused}");
}

/// The digest the pipeline names for the break is the root the development generation had before
/// its first Ed25519 root, which this repository keeps a copy of.
#[test]
fn the_pinned_predecessor_is_the_root_it_names() {
    let bytes = std::fs::read(root().join("fixtures/lineage/predecessor-root.json"))
        .expect("the predecessor's root reads");
    assert_eq!(hex::encode(Sha256::digest(&bytes)), BREAK_FROM_DIGEST);
}

/// The signatures of a role are written in key identifier order, whatever order they were made in,
/// and a role's members in name order.
#[tokio::test]
async fn signatures_are_written_in_key_order_whatever_order_they_were_made_in() {
    let roots = chain(1).await;
    let mut signed: Signed<Root> = serde_json::from_slice(&roots[1]).expect("the root parses");
    assert_eq!(signed.signatures.len(), 2);
    signed
        .signatures
        .sort_by(|left, right| right.keyid[..].cmp(&left.keyid[..]));
    let written = canonical::canonical(&signed).expect("the root renders");
    assert_eq!(written, roots[1], "the same root, written in key order");
}
