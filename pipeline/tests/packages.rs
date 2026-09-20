//! Every package in the repository validates, and a broken one does not.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kalareach_catalogue::{fixtures, packages, repository_root};
use kr_plugin_sdk::matching::MatchConfidence;

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

#[test]
fn every_package_in_the_repository_validates() {
    let loaded = packages::load(&root()).expect("the repository loads");
    assert!(
        loaded.rejected.is_empty(),
        "{:?}",
        loaded
            .rejected
            .iter()
            .map(|rejected| (&rejected.relative, &rejected.report.findings))
            .collect::<Vec<_>>()
    );
    assert!(!loaded.repository.packages.is_empty());
    assert!(!loaded.repository.publishers.is_empty());
}

#[test]
fn every_package_belongs_to_a_publisher_with_a_record() {
    let loaded = packages::load(&root()).expect("the repository loads");
    for package in &loaded.repository.packages {
        let publisher = package.package.manifest.publisher_id.to_string();
        assert!(
            loaded.repository.publishers.contains_key(&publisher),
            "{publisher} has no record"
        );
    }
}

#[test]
fn every_package_fixture_matches_its_own_predicates() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let mut cases = 0;
    for package in &loaded.repository.packages {
        cases += fixtures::check(&package.relative, &package.directory, &package.package)
            .unwrap_or_else(|error| panic!("{}: {error}", package.relative));
    }
    assert!(cases > 0, "no package carries a fixture");
}

#[test]
fn the_bundled_connectors_claim_only_what_they_carry() {
    let loaded = packages::load(&root()).expect("the repository loads");
    for package in &loaded.repository.packages {
        let manifest = &package.package.manifest;
        if package.package.connector.is_some() {
            continue;
        }
        // Without a declarative native-proxy table there is nothing qualified for the gateway to
        // interpret, so a package that ships no connector claims no upstream or approval trust.
        for request in &manifest.capabilities {
            assert!(
                request.capability.within_default_ceiling(),
                "{} requests {} without a connector table",
                package.relative,
                request.capability
            );
        }
        for action in &manifest.actions {
            assert!(
                !action.effect.is_mutation(),
                "{} declares the mutation {} without a connector table",
                package.relative,
                action.effect
            );
        }
    }
}

#[test]
fn a_native_bridge_grant_discloses_what_the_bridge_runs_as() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let mut recipes = 0;
    for package in &loaded.repository.packages {
        let Some(bridge) = &package.package.manifest.native_bridge.0 else {
            continue;
        };
        recipes += 1;
        // The validator checks that a recipe is complete and removable. It cannot check that the
        // grant a person reads before accepting it says what accepting it means, and bridge code
        // runs under the application's own permissions rather than inside the plugin sandbox.
        let statement = bridge.grant_statement.as_str();
        assert!(
            statement.contains("own permissions"),
            "{}: the grant does not say whose permissions the bridge runs under: {statement}",
            package.relative
        );
        assert!(
            statement.contains("sandbox"),
            "{}: the grant does not say the bridge runs outside the sandbox: {statement}",
            package.relative
        );
        assert!(
            statement.contains("Removal"),
            "{}: the grant does not say what removal takes back out: {statement}",
            package.relative
        );
    }
    assert!(recipes > 0, "no package ships a native bridge");
}

#[test]
fn connector_profiles_stay_separate_and_need_more_than_an_executable_name() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let mut protocols: BTreeMap<String, String> = BTreeMap::new();
    let mut connectors = 0;
    for package in &loaded.repository.packages {
        let Some(connector) = &package.package.connector else {
            continue;
        };
        connectors += 1;
        // One protocol, one profile. Two packages pinned to the same protocol name would leave a
        // host choosing between two qualifications of one upstream.
        if let Some(first) =
            protocols.insert(connector.protocol.name.clone(), package.relative.clone())
        {
            panic!(
                "{} and {} both claim the protocol {}",
                first, package.relative, connector.protocol.name
            );
        }
        // A profile is not selected by a name on disk alone. A package that reads somebody's
        // protocol carries at least one rule that identifies the application by something that
        // cannot be coincidence, and an executable name on its own is always inferred.
        assert!(
            package
                .package
                .manifest
                .match_rules
                .iter()
                .any(|rule| rule.confidence == MatchConfidence::Exact
                    && rule.distribution.0.is_some()),
            "{} recognises its application by name alone",
            package.relative
        );
        for rule in &package.package.manifest.match_rules {
            if rule.distribution.0.is_none() {
                assert_eq!(
                    rule.confidence,
                    MatchConfidence::Inferred,
                    "{}: the rule {} claims an exact match from an executable name",
                    package.relative,
                    rule.id
                );
            }
        }
    }
    assert!(connectors > 0, "no package carries a connector table");
}

#[test]
fn a_tampered_package_is_rejected() {
    let source = root().join("plugins/kalareach/example-declarative");
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path();
    std::fs::create_dir_all(repository.join("publishers")).expect("publishers directory");
    std::fs::copy(
        root().join("publishers/kalareach.json"),
        repository.join("publishers/kalareach.json"),
    )
    .expect("the publisher record copies");
    let destination = repository.join("plugins/kalareach/example-declarative");
    copy_tree(&source, &destination);

    // One byte of the presentation document changes, and nothing else.
    let presentation = destination.join("presentation.json");
    let mut text = std::fs::read_to_string(&presentation).expect("the document reads");
    text.push('\n');
    std::fs::write(&presentation, text).expect("the document writes");

    let loaded = packages::load(repository).expect("the repository loads");
    assert_eq!(loaded.rejected.len(), 1);
    let report = &loaded.rejected[0].report;
    assert!(report.has(kr_plugin_sdk::validate::FindingCode::DigestMismatch));
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
