//! A candidate generation names the pinned builds that have no qualification record yet, for one
//! run on one machine, and for nothing a release is made of.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use kalareach_catalogue::{builds, development, packages, repository_root, tuf};

const CODEX: &str = "kalareach/codex";

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

/// The SHA-256 the build list pins for the Codex build, which no record yet shows passing.
fn pinned_codex_digest() -> String {
    let list: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root().join("fixtures/agents/builds.json")).expect("the build list reads"),
    )
    .expect("the build list is JSON");
    list["builds"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|build| build["package"] == CODEX)
        .and_then(|build| build["sha256"].as_str())
        .expect("the build list pins a Codex build")
        .to_owned()
}

/// The digests of the executables the Codex release `release` names in `builds`.
fn named_for(builds: &builds::Builds, release: &str) -> Vec<String> {
    builds
        .for_release(CODEX, release)
        .iter()
        .map(|build| build.executable_digest.to_string())
        .collect()
}

/// The release of the Codex package this repository publishes.
fn codex_release(loaded: &packages::Loaded) -> String {
    loaded
        .repository
        .packages
        .iter()
        .find(|package| package.package.manifest.plugin_id().to_string() == CODEX)
        .map(|package| package.package.manifest.version.to_string())
        .expect("the repository publishes the Codex package")
}

/// Runs `kalareach-catalogue build` over this repository with `arguments`, in an environment with no
/// signing directory and none of the variables a hosted job sets, except those in `environment`.
fn build(arguments: &[&str], environment: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_kalareach-catalogue"));
    command
        .env_remove("KALAREACH_SIGNING_DIR")
        .env_remove("GITHUB_ACTIONS")
        .env_remove("CI")
        .arg("--repository")
        .arg(root())
        .args([
            "build",
            "--generation",
            "2",
            "--produced-at",
            "1760000000000",
            "--expires-at",
            development::ROOT_EXPIRES,
        ])
        .args(arguments);
    for (name, value) in environment {
        command.env(name, value);
    }
    command.output().expect("the pipeline runs")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A pinned build with no record is left out of a release's entry, and a candidate names it.
#[test]
fn a_pinned_build_with_no_record_is_named_by_a_candidate_and_by_no_other_load() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let release = codex_release(&loaded);
    let digest = pinned_codex_digest();
    assert!(
        !named_for(&loaded.repository.builds, &release).contains(&digest),
        "the Codex build has a record for release {release}, so a candidate shows nothing new"
    );
    assert!(!loaded.repository.builds.is_candidate());
    let candidate = builds::load_candidate(&root(), &loaded.repository.packages)
        .expect("a candidate set of builds loads");
    assert!(candidate.is_candidate());
    assert!(
        named_for(&candidate, &release).contains(&digest),
        "a candidate names the pinned Codex build whose digest is {digest}"
    );
}

/// The generation a candidate build writes lists the pinned build in its index, says it is a
/// candidate in its signed metadata, and verifies; a build that is not one says neither.
#[tokio::test]
async fn a_candidate_generation_lists_the_pinned_build_and_says_candidate_in_its_metadata() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let candidate = temporary.path().join("candidate");
    let output = build(
        &[
            "--development",
            "--candidate",
            "--out",
            candidate.to_str().expect("a path"),
        ],
        &[],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    // What the build prints tells a person that the generation is a candidate, and which builds it
    // names that no record shows passing.
    let printed = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        printed.contains("candidate generation: it names"),
        "{printed}"
    );
    assert!(printed.contains(&pinned_codex_digest()), "{printed}");
    let verified = tuf::verify(&candidate, false)
        .await
        .expect("the candidate generation verifies");
    assert!(verified.candidate, "the signed metadata says candidate");
    let digest = pinned_codex_digest();
    let listed = verified
        .index
        .entries
        .iter()
        .filter(|entry| format!("{}/{}", entry.publisher_id, entry.plugin_name) == CODEX)
        .flat_map(|entry| entry.builds.iter())
        .any(|build| build.executable_digest.to_string() == digest);
    assert!(listed, "the index lists the pinned build {digest}");

    let ordinary = temporary.path().join("ordinary");
    let output = build(
        &["--development", "--out", ordinary.to_str().expect("a path")],
        &[],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let verified = tuf::verify(&ordinary, false)
        .await
        .expect("the ordinary generation verifies");
    assert!(
        !verified.candidate,
        "an ordinary generation is no candidate"
    );
}

/// A candidate is signed with development keys only, so a release key refuses it, whether the key's
/// directory is named on the command line or in the environment.
#[test]
fn a_candidate_refuses_a_release_key() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let out = temporary.path().join("candidate");
    let keys = temporary.path().join("keys");
    for (arguments, environment) in [
        (
            vec![
                "--candidate",
                "--signing-dir",
                keys.to_str().expect("a path"),
            ],
            vec![],
        ),
        (
            vec!["--candidate"],
            vec![("KALAREACH_SIGNING_DIR", keys.to_str().expect("a path"))],
        ),
    ] {
        let mut all = arguments;
        all.extend(["--out", out.to_str().expect("a path")]);
        let output = build(&all, &environment);
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("a candidate generation is signed with development keys only"),
            "{}",
            stderr(&output)
        );
        assert!(!out.exists(), "nothing was written");
    }
}

/// A candidate is made on a person's machine: a release or hosted job refuses it.
#[test]
fn a_candidate_refuses_a_hosted_or_release_job() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let out = temporary.path().join("candidate");
    for variable in ["GITHUB_ACTIONS", "CI"] {
        let output = build(
            &[
                "--development",
                "--candidate",
                "--out",
                out.to_str().expect("a path"),
            ],
            &[(variable, "true")],
        );
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("a candidate generation is made on a person's machine"),
            "{}",
            stderr(&output)
        );
        assert!(!out.exists(), "nothing was written");
    }
}

/// A candidate is written outside the checkout, so it is never over the folder a generation is
/// published from.
#[test]
fn a_candidate_refuses_a_folder_inside_the_checkout() {
    let inside = root().join("snapshots").join("candidate-refused");
    // A folder that is not there, then `..` back into the checkout: the path cannot be resolved, so
    // it is refused. The folder it names is a sibling of the checkout and is never made.
    let missing = root()
        .parent()
        .expect("the checkout has a parent")
        .join("a-folder-that-is-not-there-qr1");
    assert!(!missing.exists(), "the case needs the folder to be absent");
    let through = missing
        .join("..")
        .join(root().file_name().expect("the checkout has a name"))
        .join("snapshots/candidate-refused-through");
    for out in [
        inside.clone(),
        root().join("snapshots/../snapshots/development"),
        through,
    ] {
        let output = build(
            &[
                "--development",
                "--candidate",
                "--out",
                out.to_str().expect("a path"),
            ],
            &[],
        );
        assert!(!output.status.success());
        assert!(
            stderr(&output).contains("a candidate generation is written outside the repository"),
            "{}",
            stderr(&output)
        );
    }
    assert!(!inside.exists(), "nothing was written");
    assert!(
        !missing.exists(),
        "the folder that was not there is not made"
    );
    assert!(!root().join("snapshots/candidate-refused-through").exists());
}

/// `verify` says of a candidate generation that it is one, and of an ordinary one nothing of the kind.
#[test]
fn verify_says_that_a_candidate_generation_is_one() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let verify = |generation: &Path| {
        let output = Command::new(env!("CARGO_BIN_EXE_kalareach-catalogue"))
            .env_remove("KALAREACH_SIGNING_DIR")
            .arg("--repository")
            .arg(root())
            .arg("verify")
            .arg(generation)
            .output()
            .expect("the pipeline runs");
        assert!(output.status.success(), "{}", stderr(&output));
        String::from_utf8_lossy(&output.stdout).into_owned()
    };
    let candidate = temporary.path().join("candidate");
    let output = build(
        &[
            "--development",
            "--candidate",
            "--out",
            candidate.to_str().expect("a path"),
        ],
        &[],
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(verify(&candidate).contains("a candidate generation"));
    assert!(!verify(&root().join("snapshots/development")).contains("candidate"));
}
