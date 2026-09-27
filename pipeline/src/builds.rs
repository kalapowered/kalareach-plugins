//! The executable builds each release is qualified against, as the signed index names them.
//!
//! An index entry's `builds` tell a host which version an executable is: each names an
//! application's build by the SHA-256 of its executable, with the version, the distribution it came
//! from and the platform it runs on. A host takes the version from there and nowhere else, and
//! holds it to the ranges a release declares for its recipe and its connector table. Section 12
//! makes each version gate an acceptance requirement: a build is qualified when every case it
//! lists has passed against it. So an entry names a build only when the evidence says so.
//!
//! Two committed inputs decide it, and neither is part of a package, so a generation can add or
//! withdraw a build without a new package:
//!
//! - the build list, `fixtures/agents/builds.json`, which pins each build: the package it is for,
//!   the application, the version, where it came from, how a launch reaches its pinned file, and
//!   that file's SHA-256;
//! - the qualification records beside it ([`crate::records`]), one per release, build and platform,
//!   which state the outcome of every part of section 12's cases.
//!
//! A pinned build is named in its release's entry when all of these hold:
//!
//! - its pinned file is what a process runs (a `native` or `child` launch). A script's process is
//!   its interpreter and a wheel is an archive: a host names no executable by their digests;
//! - its record is in good form, and is for this release (its version, manifest and attachment
//!   paths) and for this build (its application, version and SHA-256);
//! - the record's run is whole: no failure outside its identifiers, no problem, no failed step;
//! - no part failed, and every part of section 12's eight cases ([`REQUIRED_PARTS`]) passed. A part
//!   not run is not a pass, whatever the reason it could not run.
//!
//! A build that misses any of them is left out, and [`Builds::omitted`] says why. Input that
//! cannot be read for what it shows is refused instead: a build list or a record that breaks its
//! form, a platform the list does not know, a build pinned for a package this repository does not
//! publish, or one executable named as two versions.

use std::collections::BTreeMap;
use std::path::Path;

use kr_plugin_sdk::catalogue::QualifiedBuild;
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::text::Label;
use kr_plugin_sdk::version::PackageVersion;
use serde::Deserialize;
use serde_json::Value;

use crate::packages::LoadedPackage;
use crate::records::{self, Platform};
use crate::{Error, Result, read_json};

/// The build list, relative to the repository root.
pub const LIST: &str = "fixtures/agents/builds.json";

/// The build list format version this pipeline reads.
pub const LIST_VERSION: u32 = 1;

/// The parts a record must show passed for its build to be named: every part of section 12's
/// eight qualification cases.
pub const REQUIRED_PARTS: [&str; 13] = records::CASE_PARTS;

/// How a launch reaches a build's pinned file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Launch {
    /// The process the command starts runs the pinned file.
    Native,
    /// A runtime the command starts runs a process of the pinned file.
    Child,
    /// A runtime runs the pinned file as its script.
    Script,
    /// A runtime loads an installation of the pinned wheel.
    Wheel,
}

impl Launch {
    /// Whether the pinned file is what a process runs, which is what a host hashes and names an
    /// executable by.
    #[must_use]
    pub const fn pins_an_executable(self) -> bool {
        matches!(self, Self::Native | Self::Child)
    }
}

/// One build the list pins.
#[derive(Clone, Debug)]
pub struct Pinned {
    /// The package whose connector table is qualified against it.
    pub package: String,
    /// The application, as the records name it.
    pub application: Label,
    /// The upstream version.
    pub version: PackageVersion,
    /// Where it came from: a registry and its package, or the vendor's archive.
    pub distribution: Label,
    /// How a launch reaches the pinned file.
    pub launch: Launch,
    /// The SHA-256 of the pinned file.
    pub sha256: PayloadDigest,
}

/// The build list: the builds pinned for one platform.
#[derive(Clone, Debug)]
pub struct List {
    /// The platform every pinned build runs on.
    pub platform: Platform,
    /// The builds, in the list's order.
    pub builds: Vec<Pinned>,
}

/// The build list as it is written. It carries what the qualification harness needs as well; only
/// what decides a build's identity is read here.
#[derive(Deserialize)]
struct ListFile {
    list_version: u32,
    platform: String,
    builds: Vec<PinnedFile>,
}

#[derive(Deserialize)]
struct PinnedFile {
    package: String,
    application: String,
    version: String,
    distribution: String,
    launch: Launch,
    sha256: String,
    #[serde(default)]
    newer: Option<NewerFile>,
}

/// The newer build an upgrade part moves to. It is pinned for that part alone: no record is kept
/// for it, so it is never named, but it is still one executable at one version.
#[derive(Deserialize)]
struct NewerFile {
    version: String,
    sha256: String,
}

/// Reads the build list, or nothing when the repository has none.
///
/// # Errors
///
/// Returns an error when the list cannot be read, is another format version, names a platform
/// this pipeline does not know, carries a value the SDK does not accept, pins one build twice, or
/// names one executable as two versions.
pub fn read_list(root: &Path) -> Result<Option<List>> {
    let path = root.join(LIST);
    if !path.exists() {
        return Ok(None);
    }
    let file: ListFile = read_json(&path)?;
    let refuse = |detail: String| Error::Layout {
        detail: format!("{LIST}: {detail}"),
    };
    if file.list_version != LIST_VERSION {
        return Err(refuse(format!(
            "list version {} is not version {LIST_VERSION}",
            file.list_version
        )));
    }
    let platform = Platform::parse(&file.platform).map_err(refuse)?;
    let mut builds = Vec::new();
    let mut versions: BTreeMap<PayloadDigest, PackageVersion> = BTreeMap::new();
    let mut named_as = |digest: PayloadDigest, version: &PackageVersion| match versions.get(&digest)
    {
        Some(first) if first != version => Err(refuse(format!(
            "the executable {digest} is named as version {first} and as version {version}"
        ))),
        Some(_) => Ok(()),
        None => {
            versions.insert(digest, version.clone());
            Ok(())
        }
    };
    for build in file.builds {
        let what = format!("{} {}", build.package, build.version);
        let version =
            parse_version(&build.version).map_err(|detail| refuse(format!("{what}: {detail}")))?;
        let sha256 =
            parse_digest(&build.sha256).map_err(|detail| refuse(format!("{what}: {detail}")))?;
        named_as(sha256, &version)?;
        if let Some(newer) = &build.newer {
            let newer_version = parse_version(&newer.version)
                .map_err(|detail| refuse(format!("{what}, its newer build: {detail}")))?;
            let newer_sha256 = parse_digest(&newer.sha256)
                .map_err(|detail| refuse(format!("{what}, its newer build: {detail}")))?;
            named_as(newer_sha256, &newer_version)?;
        }
        let pinned = Pinned {
            application: Label::new(build.application)
                .map_err(|error| refuse(format!("{what}: the application: {error}")))?,
            distribution: Label::new(build.distribution)
                .map_err(|error| refuse(format!("{what}: the distribution: {error}")))?,
            package: build.package,
            version,
            launch: build.launch,
            sha256,
        };
        if builds
            .iter()
            .any(|seen: &Pinned| seen.package == pinned.package && seen.version == pinned.version)
        {
            return Err(refuse(format!("it pins {what} twice")));
        }
        builds.push(pinned);
    }
    Ok(Some(List { platform, builds }))
}

fn parse_version(text: &str) -> std::result::Result<PackageVersion, String> {
    PackageVersion::parse(text).map_err(|error| format!("{text:?} is not a version: {error}"))
}

fn parse_digest(text: &str) -> std::result::Result<PayloadDigest, String> {
    PayloadDigest::parse(text).map_err(|error| format!("{text:?} is not a SHA-256: {error}"))
}

/// One pinned build the index leaves out, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Omitted {
    /// The package it is pinned for.
    pub plugin_id: String,
    /// The release of that package this repository publishes.
    pub release: String,
    /// The application.
    pub application: String,
    /// The build's version.
    pub version: String,
    /// The platform it runs on.
    pub platform: String,
    /// Everything that keeps it out.
    pub reasons: Vec<String>,
}

impl std::fmt::Display for Omitted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} {}: {} {} on {}: {}",
            self.plugin_id,
            self.release,
            self.application,
            self.version,
            self.platform,
            self.reasons.join("; ")
        )
    }
}

/// The builds each release names, and the pinned builds left out.
#[derive(Clone, Debug, Default)]
pub struct Builds {
    /// By package and release.
    named: BTreeMap<(String, String), Vec<QualifiedBuild>>,
    omitted: Vec<Omitted>,
}

impl Builds {
    /// The builds one release names, in their canonical order.
    #[must_use]
    pub fn for_release(&self, plugin_id: &str, release: &str) -> Vec<QualifiedBuild> {
        self.named
            .get(&(plugin_id.to_owned(), release.to_owned()))
            .cloned()
            .unwrap_or_default()
    }

    /// Every named build, with the package and release it is named for.
    pub fn named(&self) -> impl Iterator<Item = (&str, &str, &QualifiedBuild)> {
        self.named
            .iter()
            .flat_map(|((plugin_id, release), builds)| {
                builds
                    .iter()
                    .map(move |build| (plugin_id.as_str(), release.as_str(), build))
            })
    }

    /// The pinned builds left out, in the list's order.
    #[must_use]
    pub fn omitted(&self) -> &[Omitted] {
        &self.omitted
    }
}

/// Decides which pinned builds each release in `packages` names.
///
/// # Errors
///
/// Returns an error when the build list is refused ([`read_list`]), pins a build for a package this
/// repository does not publish, or a record cannot be read or breaks its form.
pub fn load(root: &Path, packages: &[LoadedPackage]) -> Result<Builds> {
    let Some(list) = read_list(root)? else {
        return Ok(Builds::default());
    };
    let mut builds = Builds::default();
    for pinned in &list.builds {
        let Some(package) = packages
            .iter()
            .find(|package| package.package.manifest.plugin_id().as_str() == pinned.package)
        else {
            return Err(Error::Layout {
                detail: format!(
                    "{LIST} pins a build for {}, which this repository does not publish",
                    pinned.package
                ),
            });
        };
        let manifest = &package.package.manifest;
        let release = manifest.version.to_string();
        let relative = records::path(
            manifest.publisher_id.as_str(),
            manifest.plugin_name.as_str(),
            &pinned.version.to_string(),
            &list.platform,
        );
        let reasons = reasons_to_leave_out(root, &relative, &list.platform, pinned, package)?;
        if reasons.is_empty() {
            builds
                .named
                .entry((pinned.package.clone(), release))
                .or_default()
                .push(QualifiedBuild {
                    application: pinned.application.clone(),
                    distribution: pinned.distribution.clone(),
                    version: pinned.version.clone(),
                    os: list.platform.os(),
                    architecture: list.platform.architecture(),
                    executable_digest: pinned.sha256,
                });
        } else {
            builds.omitted.push(Omitted {
                plugin_id: pinned.package.clone(),
                release,
                application: pinned.application.to_string(),
                version: pinned.version.to_string(),
                platform: list.platform.name().to_owned(),
                reasons,
            });
        }
    }
    for named in builds.named.values_mut() {
        named.sort_by(|left, right| canonical(left).cmp(&canonical(right)));
    }
    Ok(builds)
}

/// The order an entry's builds are written in, so a rebuild writes the same bytes.
fn canonical(
    build: &QualifiedBuild,
) -> (
    kr_plugin_sdk::matching::OperatingSystem,
    kr_plugin_sdk::matching::Architecture,
    &Label,
    &PackageVersion,
    PayloadDigest,
) {
    (
        build.os,
        build.architecture,
        &build.application,
        &build.version,
        build.executable_digest,
    )
}

/// Everything that keeps one pinned build out of its release's entry; nothing when it is named.
fn reasons_to_leave_out(
    root: &Path,
    relative: &Path,
    platform: &Platform,
    pinned: &Pinned,
    package: &LoadedPackage,
) -> Result<Vec<String>> {
    let path = root.join(relative);
    if !path.is_file() {
        return Ok(vec![format!(
            "no qualification record at {}",
            relative.display()
        )]);
    }
    let record: Value = read_json(&path)?;
    let form = records::form_problems(&record, platform);
    if !form.is_empty() {
        return Err(Error::Layout {
            detail: format!(
                "{} is not a qualification record in good form: {}",
                relative.display(),
                form.join("; ")
            ),
        });
    }
    let mut reasons = Vec::new();
    match pinned.launch {
        Launch::Script => reasons.push(
            "the pinned file is a script, whose process is its interpreter, so it names no \
             executable a host runs"
                .to_owned(),
        ),
        Launch::Wheel => reasons.push(
            "the pinned file is a wheel, an archive, so it names no executable a host runs"
                .to_owned(),
        ),
        Launch::Native | Launch::Child => {}
    }
    reasons.extend(records::release_problems(&record, package));
    reasons.extend(records::build_problems(
        &record,
        pinned.application.as_str(),
        &pinned.version.to_string(),
        &pinned.sha256.to_string(),
    ));
    reasons.extend(qualification_gaps(&record));
    Ok(reasons)
}

/// What a record in good form lacks for its build to be named: a whole run, no failed part, and
/// every part of section 12's cases passed.
fn qualification_gaps(record: &Value) -> Vec<String> {
    let mut gaps = Vec::new();
    if record["failures_outside_identifiers"]
        .as_array()
        .is_some_and(|failures| !failures.is_empty())
    {
        gaps.push("the run has failures outside its identifiers".to_owned());
    }
    if record["problems"]
        .as_array()
        .is_some_and(|problems| !problems.is_empty())
    {
        gaps.push("the run names problems that leave its result incomplete".to_owned());
    }
    let failed_steps: Vec<String> = record["steps"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|step| step["exit"] != 0)
        .map(|step| step["number"].to_string())
        .collect();
    if !failed_steps.is_empty() {
        gaps.push(format!(
            "steps {} did not exit cleanly",
            failed_steps.join(", ")
        ));
    }
    let failed: Vec<&str> = records::all_tests(record)
        .into_iter()
        .filter(|test| test["outcome"] == "failed")
        .filter_map(|test| test["part"].as_str())
        .collect();
    if !failed.is_empty() {
        gaps.push(format!("parts {} failed", failed.join(", ")));
    }
    let cases = records::parts(record, records::CASES);
    let not_passed: Vec<&str> = REQUIRED_PARTS
        .iter()
        .copied()
        .filter(|part| {
            cases
                .get(part)
                .is_none_or(|test| test["outcome"] != "passed")
        })
        .collect();
    if !not_passed.is_empty() {
        gaps.push(format!(
            "section 12 parts {} did not pass",
            not_passed.join(", ")
        ));
    }
    gaps
}
