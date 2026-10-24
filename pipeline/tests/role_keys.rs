//! No two roles of a trust root name one key, in whatever spelling the verifier reads the key.
//!
//! The library that checks a signature reads one key in more than one spelling, and each spelling
//! has an identifier of its own, so comparing identifiers alone leaves a key shared by two roles
//! unfound. The comparison reduces the spellings it knows to one value, and refuses a spelling it
//! cannot reduce, so a key is either found or its root is refused.

use std::collections::HashMap;
use std::num::NonZeroU64;

use kalareach_catalogue::development::DerivedKeySource;
use kalareach_catalogue::keys::check_role_keys;
use tough::key_source::KeySource as _;
use tough::schema::key::{Key, RsaKey, RsaScheme};
use tough::schema::{RoleKeys, RoleType, Root};

/// A root that names `named` for its roles. Nothing signs it: the comparison reads only the keys.
fn root_naming(named: Vec<(RoleType, Vec<Key>)>) -> Root {
    let mut keys = HashMap::new();
    let mut roles = HashMap::new();
    for (role, role_keys) in named {
        let mut identifiers = Vec::new();
        for key in role_keys {
            let identifier = key.key_id().expect("a key identifier");
            keys.insert(identifier.clone(), key);
            identifiers.push(identifier);
        }
        roles.insert(
            role,
            RoleKeys {
                keyids: identifiers,
                threshold: NonZeroU64::MIN,
                _extra: HashMap::new(),
            },
        );
    }
    Root {
        spec_version: "1.0.0".to_owned(),
        consistent_snapshot: false,
        version: NonZeroU64::MIN,
        expires: "2046-10-01T00:00:00Z".parse().expect("a time"),
        keys,
        roles,
        _extra: HashMap::new(),
    }
}

/// The four roles named by the keys given, one each, in the order root, targets, snapshot,
/// timestamp, with `extra` also named by the timestamp role.
fn four(keys: [Key; 4], extra: Option<Key>) -> Root {
    let [root, targets, snapshot, timestamp] = keys;
    root_naming(vec![
        (RoleType::Root, vec![root]),
        (RoleType::Targets, vec![targets]),
        (RoleType::Snapshot, vec![snapshot]),
        (
            RoleType::Timestamp,
            std::iter::once(timestamp).chain(extra).collect(),
        ),
    ])
}

async fn derived(role: RoleType) -> Key {
    DerivedKeySource::new(role, 0)
        .as_sign()
        .await
        .expect("a derived key")
        .tuf_key()
}

/// Four Ed25519 keys, one for each role.
async fn ed25519_keys() -> [Key; 4] {
    [
        derived(RoleType::Root).await,
        derived(RoleType::Targets).await,
        derived(RoleType::Snapshot).await,
        derived(RoleType::Timestamp).await,
    ]
}

/// The key's public value with `prefix` before it and `suffix` after, as the key table spells it.
fn respelled(key: &Key, prefix: &[u8], suffix: &[u8]) -> Key {
    let mut value = serde_json::to_value(key).expect("a description");
    let public = value["keyval"]["public"]
        .as_str()
        .expect("hexadecimal")
        .to_owned();
    let bytes: Vec<u8> = (0..public.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&public[at..at + 2], 16).expect("hexadecimal"))
        .collect();
    let spelled: String = [prefix, &bytes, suffix]
        .concat()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    value["keyval"]["public"] = serde_json::json!(spelled);
    serde_json::from_value(value).expect("a key")
}

/// An ASN.1 element, in DER.
fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut element = vec![tag];
    match content.len() {
        length @ 0..=127 => element.push(u8::try_from(length).expect("short")),
        length @ 128..=255 => element.extend([0x81, u8::try_from(length).expect("one byte")]),
        length => element.extend([0x82, (length >> 8) as u8, length as u8]),
    }
    element.extend_from_slice(content);
    element
}

/// An RSA public key as the library reads it from the key table: the `RSAPublicKey` inside the
/// outer `SubjectPublicKeyInfo`, the value `seed` makes. It is a shape and not a key that signs.
fn rsa_public(seed: u8) -> Vec<u8> {
    let modulus: Vec<u8> = std::iter::once(0x00)
        .chain(std::iter::once(0x80 | seed))
        .chain((0..254).map(|at| seed.wrapping_add(at as u8)))
        .collect();
    der(
        0x30,
        &[der(0x02, &modulus), der(0x02, &[0x01, 0x00, 0x01])].concat(),
    )
}

/// `public` wrapped in a `SubjectPublicKeyInfo` of the RSA algorithm.
fn rsa_wrapped(public: &[u8]) -> Vec<u8> {
    let algorithm = der(
        0x30,
        &[
            der(
                0x06,
                &[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01, 0x01],
            ),
            vec![0x05, 0x00],
        ]
        .concat(),
    );
    let bits = der(0x03, &[&[0x00], public].concat());
    der(0x30, &[algorithm, bits].concat())
}

fn rsa_key(public: Vec<u8>) -> Key {
    Key::Rsa {
        keyval: RsaKey {
            public: public.into(),
            _extra: HashMap::new(),
        },
        scheme: RsaScheme::RsassaPssSha256,
        _extra: HashMap::new(),
    }
}

/// A P-256 key from the point `point` (65 bytes, uncompressed), spelled as the key table spells it.
fn ecdsa_key(point: &[u8]) -> Key {
    let spelled: String = point.iter().map(|byte| format!("{byte:02x}")).collect();
    serde_json::from_value(serde_json::json!({
        "keytype": "ecdsa",
        "keyval": { "public": spelled },
        "scheme": "ecdsa-sha2-nistp256",
    }))
    .expect("a key")
}

fn point(seed: u8) -> Vec<u8> {
    std::iter::once(0x04)
        .chain((0..64).map(|at| seed.wrapping_add(at)))
        .collect()
}

/// The twelve bytes that wrap an Ed25519 public key in a `SubjectPublicKeyInfo`.
const ED25519_WRAPPER: [u8; 12] = [
    0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
];

/// The bytes that open a `SubjectPublicKeyInfo` of a P-256 key, before its 65-byte point.
const P256_WRAPPER: [u8; 26] = [
    0x30, 0x59, 0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a,
    0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07, 0x03, 0x42, 0x00,
];

fn refusal(root: &Root) -> String {
    check_role_keys(root)
        .expect_err("the root is refused")
        .to_string()
}

/// KR-REQ-26.40: the keys of the roles of a root are distinct. A key named by two roles is found
/// whether the two spell it alike or not, for each family of key the library reads, and a root
/// of one key for each role is the control.
#[tokio::test]
async fn a_key_two_roles_name_is_found_in_each_spelling_the_comparison_reduces() {
    // The control: a key for each role, of three families.
    let control = root_naming(vec![
        (RoleType::Root, vec![derived(RoleType::Root).await]),
        (RoleType::Targets, vec![rsa_key(rsa_public(1))]),
        (RoleType::Snapshot, vec![ecdsa_key(&point(2))]),
        (
            RoleType::Timestamp,
            vec![derived(RoleType::Timestamp).await],
        ),
    ]);
    check_role_keys(&control).expect("a key of its own for each role is accepted");

    let [root, targets, snapshot, timestamp] = ed25519_keys().await;
    let wrapped = respelled(&snapshot, &ED25519_WRAPPER, &[]);
    let spelled_alike = four(
        [
            root.clone(),
            targets.clone(),
            snapshot.clone(),
            snapshot.clone(),
        ],
        None,
    );
    let spelled_apart = four(
        [
            root.clone(),
            targets.clone(),
            snapshot.clone(),
            timestamp.clone(),
        ],
        Some(wrapped),
    );
    for (what, shared) in [
        ("Ed25519, spelled alike", spelled_alike),
        ("Ed25519, raw and wrapped", spelled_apart),
        (
            "RSA, the same key twice",
            four(
                [
                    root.clone(),
                    targets.clone(),
                    rsa_key(rsa_public(3)),
                    timestamp.clone(),
                ],
                Some(rsa_key(rsa_public(3))),
            ),
        ),
        (
            "an elliptic-curve key, uncompressed and compressed",
            root_naming(vec![
                (RoleType::Root, vec![root.clone()]),
                (RoleType::Targets, vec![targets.clone()]),
                (RoleType::Snapshot, vec![ecdsa_key(&point(4))]),
                (
                    RoleType::Timestamp,
                    vec![
                        timestamp.clone(),
                        ecdsa_key(&[&[0x02], &point(4)[1..33]].concat()),
                    ],
                ),
            ]),
        ),
    ] {
        let said = refusal(&shared);
        assert!(
            said.contains("same key") && said.contains("snapshot") && said.contains("timestamp"),
            "{what}: {said}"
        );
    }
}

/// Says that a root whose timestamp role names `odd` beside its own key is refused for the spelling
/// of `odd`, naming the role.
async fn refused_for_its_spelling(odd: Key) {
    let [root, targets, snapshot, timestamp] = ed25519_keys().await;
    let said = refusal(&four([root, targets, snapshot, timestamp], Some(odd)));
    assert!(
        said.contains("timestamp") && said.contains("spelling"),
        "{said}"
    );
}

/// KR-REQ-26.40: a spelling the comparison cannot reduce to one value is refused, and so is the
/// root that names it, since the library that checks a signature reads such a key and the
/// comparison could not say whom it shares it with. An Ed25519 key with a byte after its
/// `SubjectPublicKeyInfo`:
#[tokio::test]
async fn an_ed25519_key_with_a_byte_after_its_wrapper_is_refused() {
    let [_, _, snapshot, _] = ed25519_keys().await;
    refused_for_its_spelling(respelled(&snapshot, &ED25519_WRAPPER, &[0x00])).await;
}

/// An elliptic-curve hybrid point (65 bytes opening `06` or `07`), which the library also reads, is
/// not one of the two forms the comparison reduces.
#[tokio::test]
async fn an_elliptic_curve_hybrid_point_is_refused() {
    let mut hybrid = point(7);
    hybrid[0] = 0x06 | (hybrid[64] & 1);
    refused_for_its_spelling(ecdsa_key(&hybrid)).await;
}

/// An elliptic-curve key as a whole `SubjectPublicKeyInfo`, which the library reads, is not a point.
#[tokio::test]
async fn an_elliptic_curve_key_as_a_whole_wrapper_is_refused() {
    refused_for_its_spelling(ecdsa_key(&[&P256_WRAPPER[..], &point(5)].concat())).await;
}

/// An RSA key whose `RSAPublicKey` is wrapped in a second `SubjectPublicKeyInfo`, which the library
/// also reads, is not one `RSAPublicKey`.
#[tokio::test]
async fn an_rsa_key_inside_two_wrappers_is_refused() {
    refused_for_its_spelling(rsa_key(rsa_wrapped(&rsa_public(6)))).await;
}
