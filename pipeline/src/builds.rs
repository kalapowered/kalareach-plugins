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
//!   paths) and for this build: the build its parts ran against, the first of the record's
//!   `run.applications`, installed, is this application at this version with this SHA-256. A newer
//!   build an upgrade part moved to is listed after it, and qualifies nothing;
//! - the record's run is whole: no failure outside its identifiers, no problem, no failed step;
//! - no part failed, and every part of section 12's eight cases ([`REQUIRED_PARTS`]) passed. A part
//!   not run is not a pass, whatever the reason it could not run, except a gateway part of a
//!   package on [`TERMINAL_EVIDENCE_PACKAGES`] while its release declares no command integration
//!   and no native bridge;
//! - its release declares a capability, since a result is written for each one: a release that
//!   declares none names no build in an ordinary generation, whatever its record shows.
//!
//! A build that misses any of them is left out, and [`Builds::omitted`] says why. Input that
//! cannot be read for what it shows is refused instead: a build list or a record that breaks its
//! form, a platform the list does not know, a build pinned for a package this repository does not
//! publish or publishes without a connector table, or one executable named as two versions.
//!
//! The host release record in the core repository lists a build only with results that state what
//! the build's qualification record shows, so a build the index names comes with them: for each
//! capability its release declares, one
//! [`QualificationResult`] of the version that was qualified, from the signed record, carrying the
//! SHA-256 of the record file's bytes and a statement of what the record shows. The statement names
//! the platform of the build. It says that every case passed, or, for a package on
//! [`TERMINAL_EVIDENCE_PACKAGES`], which gateway parts did not run and that every other case part
//! passed; it speaks of the parts of section 12's cases only. Nothing else writes a result, and a
//! candidate generation, whose builds are named without requiring a record that qualifies them,
//! carries none ([`Builds::results_for`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kr_plugin_sdk::capability::{
    CapabilityRequest, CapabilityState, EvidenceSource, PluginCapability,
};
use kr_plugin_sdk::catalogue::{QualificationResult, QualifiedBuild};
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::ids::CapabilityId;
use kr_plugin_sdk::text::{Label, Summary};
use kr_plugin_sdk::version::PackageVersion;
use serde::Deserialize;
use serde_json::Value;

use crate::packages::LoadedPackage;
use crate::records::{self, Platform};
use crate::{Error, Result, read, read_json};

/// The build list, relative to the repository root.
pub const LIST: &str = "fixtures/agents/builds.json";

/// The build list format version this pipeline reads.
pub const LIST_VERSION: u32 = 1;

/// The parts a record must show passed for its build to be named: every part of section 12's
/// eight qualification cases.
pub const REQUIRED_PARTS: [&str; 13] = records::CASE_PARTS;

/// The parts of section 12's cases that run against a gateway.
pub const GATEWAY_PARTS: [&str; 3] = ["5b", "6b", "8b"];

/// The packages whose build is named on terminal evidence alone. A gateway part of one of them that
/// did not run is carried as not applicable while its release declares no command integration and
/// no native bridge. The packages are named here, and nothing in a record adds one.
pub const TERMINAL_EVIDENCE_PACKAGES: [&str; 1] = ["kalareach/opencode"];

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

/// One build a release names, with the results that state what its record shows of it.
#[derive(Clone, Debug)]
struct Named {
    build: QualifiedBuild,
    /// One for each capability the release declares; none for a candidate's build.
    results: Vec<QualificationResult>,
}

/// The builds each release names, and the pinned builds left out.
#[derive(Clone, Debug, Default)]
pub struct Builds {
    /// By package and release.
    named: BTreeMap<(String, String), Vec<Named>>,
    omitted: Vec<Omitted>,
    /// Whether the builds were named without a record ([`load_candidate`]).
    candidate: bool,
}

impl Builds {
    /// Whether these builds were named without a qualification record, for a candidate generation.
    #[must_use]
    pub const fn is_candidate(&self) -> bool {
        self.candidate
    }

    /// The builds one release names, in their canonical order.
    #[must_use]
    pub fn for_release(&self, plugin_id: &str, release: &str) -> Vec<QualifiedBuild> {
        self.named
            .get(&(plugin_id.to_owned(), release.to_owned()))
            .map(|named| named.iter().map(|named| named.build.clone()).collect())
            .unwrap_or_default()
    }

    /// The results one release carries: for each of its builds, in their canonical order, one for
    /// each capability the release declares, in the order of their identifiers. None for a
    /// release that names no build, and none for a candidate's.
    #[must_use]
    pub fn results_for(&self, plugin_id: &str, release: &str) -> Vec<QualificationResult> {
        self.named
            .get(&(plugin_id.to_owned(), release.to_owned()))
            .map(|named| {
                named
                    .iter()
                    .flat_map(|named| named.results.iter().cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Every named build, with the package and release it is named for.
    pub fn named(&self) -> impl Iterator<Item = (&str, &str, &QualifiedBuild)> {
        self.named
            .iter()
            .flat_map(|((plugin_id, release), builds)| {
                builds
                    .iter()
                    .map(move |named| (plugin_id.as_str(), release.as_str(), &named.build))
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
/// repository does not publish or publishes without a connector table, or a record cannot be read or
/// breaks its form.
pub fn load(root: &Path, packages: &[LoadedPackage]) -> Result<Builds> {
    load_with(root, packages, false)
}

/// Decides which pinned builds a candidate generation names: the same as [`load`], except that no
/// qualification record is read and none is required, so every pinned build a host could run is
/// named.
///
/// A candidate generation exists so that a real run can install the package and have the host
/// serve its route to a build whose record is not required to show it passing. It says nothing of
/// whether the build qualifies, and the build that names it carries the mark that says so.
///
/// # Errors
///
/// Returns an error when the build list is refused ([`read_list`]) or pins a build for a package
/// this repository does not publish or publishes without a connector table.
pub fn load_candidate(root: &Path, packages: &[LoadedPackage]) -> Result<Builds> {
    load_with(root, packages, true)
}

fn load_with(root: &Path, packages: &[LoadedPackage], candidate: bool) -> Result<Builds> {
    let Some(list) = read_list(root)? else {
        return Ok(Builds::default());
    };
    let mut builds = Builds {
        candidate,
        ..Builds::default()
    };
    for pinned in &list.builds {
        let package =
            connector_package(packages, &pinned.package).map_err(|what| Error::Layout {
                detail: format!("{LIST} pins a build for {what}"),
            })?;
        let manifest = &package.package.manifest;
        let release = manifest.version.to_string();
        let relative = records::path(
            manifest.publisher_id.as_str(),
            manifest.plugin_name.as_str(),
            &pinned.version.to_string(),
            &list.platform,
        );
        let assessment = if candidate {
            Assessment {
                reasons: launch_reasons(pinned),
                basis: None,
            }
        } else {
            assess(root, &relative, &list.platform, pinned, package)?
        };
        if assessment.reasons.is_empty() {
            let build = QualifiedBuild {
                application: pinned.application.clone(),
                distribution: pinned.distribution.clone(),
                version: pinned.version.clone(),
                os: list.platform.os(),
                architecture: list.platform.architecture(),
                executable_digest: pinned.sha256,
            };
            let results = match &assessment.basis {
                Some(basis) => {
                    results_of(&manifest.capabilities, &manifest.version, &build, basis)?
                }
                None => Vec::new(),
            };
            builds
                .named
                .entry((pinned.package.clone(), release))
                .or_default()
                .push(Named { build, results });
        } else {
            builds.omitted.push(Omitted {
                plugin_id: pinned.package.clone(),
                release,
                application: pinned.application.to_string(),
                version: pinned.version.to_string(),
                platform: list.platform.name().to_owned(),
                reasons: assessment.reasons,
            });
        }
    }
    for named in builds.named.values_mut() {
        named.sort_by(|left, right| canonical(&left.build).cmp(&canonical(&right.build)));
    }
    Ok(builds)
}

/// The package this repository publishes under `plugin_id` with a connector table, or what it is
/// instead. A build is qualified against a connector's table, so a pin or a record for any other
/// package is one the pipeline cannot read for what it shows.
fn connector_package<'a>(
    packages: &'a [LoadedPackage],
    plugin_id: &str,
) -> std::result::Result<&'a LoadedPackage, String> {
    let Some(package) = packages
        .iter()
        .find(|package| package.package.manifest.plugin_id().as_str() == plugin_id)
    else {
        return Err(format!(
            "{plugin_id}, which this repository does not publish"
        ));
    };
    if package.package.connector.is_none() {
        return Err(format!(
            "{plugin_id}, whose release has no connector table for a build to be qualified against"
        ));
    }
    Ok(package)
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

/// What keeps a pinned build from naming an executable a host runs, whatever a record shows.
fn launch_reasons(pinned: &Pinned) -> Vec<String> {
    match pinned.launch {
        Launch::Script => vec![
            "the pinned file is a script, whose process is its interpreter, so it names no \
             executable a host runs"
                .to_owned(),
        ],
        Launch::Wheel => vec![
            "the pinned file is a wheel, an archive, so it names no executable a host runs"
                .to_owned(),
        ],
        Launch::Native | Launch::Child => Vec::new(),
    }
}

/// What one record shows of the build it qualifies, for the results written about it.
struct Basis {
    /// The SHA-256 of the record file's bytes.
    digest: PayloadDigest,
    /// The gateway parts that did not run and were carried as not applicable, in the order of
    /// [`GATEWAY_PARTS`].
    gateway_not_run: Vec<&'static str>,
}

/// Whether one pinned build is named: everything that keeps it out, and, when nothing does, what
/// its record shows.
struct Assessment {
    reasons: Vec<String>,
    /// What the record shows; none where no record was read.
    basis: Option<Basis>,
}

/// Decides whether one pinned build is named for its release.
fn assess(
    root: &Path,
    relative: &Path,
    platform: &Platform,
    pinned: &Pinned,
    package: &LoadedPackage,
) -> Result<Assessment> {
    let path = root.join(relative);
    if !path.is_file() {
        return Ok(Assessment {
            reasons: vec![format!("no qualification record at {}", relative.display())],
            basis: None,
        });
    }
    let bytes = read(&path)?;
    let record: Value = serde_json::from_slice(&bytes).map_err(|source| Error::Json {
        path: path.clone(),
        source,
    })?;
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
    let mut reasons = launch_reasons(pinned);
    reasons.extend(records::release_problems(&record, package));
    reasons.extend(records::build_problems(
        &record,
        pinned.application.as_str(),
        &pinned.version.to_string(),
        &pinned.sha256.to_string(),
    ));
    let gaps = qualification_gaps(&record, package);
    reasons.extend(gaps.reasons);
    if package.package.manifest.capabilities.is_empty() {
        reasons.push(
            "the release declares no capability, so no qualification result can state what \
             its record shows of the build"
                .to_owned(),
        );
    }
    Ok(Assessment {
        reasons,
        basis: Some(Basis {
            digest: PayloadDigest::of(&bytes),
            gateway_not_run: gaps.gateway_not_run,
        }),
    })
}

/// What a record in good form lacks for its build to be named, and which gateway parts it was not
/// asked to show.
struct Gaps {
    /// What is lacking; nothing when the build is named.
    reasons: Vec<String>,
    /// The gateway parts that did not run and were accepted as not applicable, in the order of
    /// [`GATEWAY_PARTS`]. Each is accepted on its own, so this is some of the three, all of them or
    /// none.
    gateway_not_run: Vec<&'static str>,
}

/// What a record in good form lacks for its build to be named: a whole run, no failed part, and
/// every part of section 12's cases passed, or, for a package on [`TERMINAL_EVIDENCE_PACKAGES`]
/// whose release declares no command integration and no native bridge, is a gateway part that did
/// not run.
fn qualification_gaps(record: &Value, package: &LoadedPackage) -> Gaps {
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
    let manifest = &package.package.manifest;
    // A gateway part that did not run is not a gap for a package named on terminal evidence; a
    // failed part is still one.
    let gateway_not_applicable = TERMINAL_EVIDENCE_PACKAGES
        .contains(&manifest.plugin_id().as_str())
        && manifest.command_integration.is_none()
        && manifest.native_bridge.0.is_none();
    let not_passed: Vec<&str> = REQUIRED_PARTS
        .iter()
        .copied()
        .filter(|part| {
            cases.get(part).is_none_or(|test| {
                test["outcome"] != "passed"
                    && !(gateway_not_applicable
                        && GATEWAY_PARTS.contains(part)
                        && test["outcome"] == "not_run")
            })
        })
        .collect();
    if !not_passed.is_empty() {
        gaps.push(format!(
            "section 12 parts {} did not pass",
            not_passed.join(", ")
        ));
    }
    let gateway_not_run = if gateway_not_applicable {
        GATEWAY_PARTS
            .iter()
            .copied()
            .filter(|part| {
                cases
                    .get(part)
                    .is_some_and(|test| test["outcome"] == "not_run")
            })
            .collect()
    } else {
        Vec::new()
    };
    Gaps {
        reasons: gaps,
        gateway_not_run,
    }
}

/// The identifier a result gives a declared capability: the plugin namespace, the capability's name
/// and version 1.
#[must_use]
pub fn capability_id(capability: PluginCapability) -> String {
    format!("plugin.{}/1", capability.as_str())
}

/// What a result names a build by: its application and version.
#[must_use]
pub fn subject(build: &QualifiedBuild) -> String {
    format!("{} {}", build.application.as_str(), build.version)
}

/// What a result states about a build on `platform` whose record shows every part of section 12's
/// cases passed except `gateway_not_run`, which did not run and are not applicable to the package.
fn statement(gateway_not_run: &[&str], platform: &str) -> String {
    if gateway_not_run.is_empty() {
        format!("every case of the version qualification passed on {platform}")
    } else {
        format!(
            "{} not run (no gateway); every other case part passed on {platform}",
            gateway_not_run.join(", ")
        )
    }
}

/// Every statement the pipeline writes about a build on `platform`: that every case passed, and
/// each combination of the gateway parts that did not run.
#[must_use]
pub fn statements(platform: &str) -> Vec<String> {
    (0..1_usize << GATEWAY_PARTS.len())
        .map(|combination| {
            let not_run: Vec<&str> = GATEWAY_PARTS
                .iter()
                .enumerate()
                .filter(|(position, _)| combination & (1 << position) != 0)
                .map(|(_, part)| *part)
                .collect();
            statement(&not_run, platform)
        })
        .collect()
}

/// The results for one named build: one for each capability its release declares, in the order of
/// their identifiers. Each states that the version was qualified, from the signed record whose bytes
/// `basis` digests.
fn results_of(
    capabilities: &[CapabilityRequest],
    release: &PackageVersion,
    build: &QualifiedBuild,
    basis: &Basis,
) -> Result<Vec<QualificationResult>> {
    let layout = |what: String| Error::Layout { detail: what };
    let subject = Label::new(subject(build)).map_err(|source| {
        layout(format!(
            "{LIST}: the build of {} {} cannot be named as a result's subject: {source}",
            build.application, build.version
        ))
    })?;
    let platform = records::platform_name(build.os, build.architecture);
    let statement =
        Summary::new(statement(&basis.gateway_not_run, &platform)).map_err(|source| {
            layout(format!(
                "{LIST}: the statement for {subject} cannot be written: {source}",
                subject = subject.as_str()
            ))
        })?;
    let mut ids: Vec<String> = capabilities
        .iter()
        .map(|request| capability_id(request.capability))
        .collect();
    ids.sort();
    ids.into_iter()
        .map(|id| {
            Ok(QualificationResult {
                capability_id: CapabilityId::new(id.as_str()).map_err(|source| {
                    layout(format!("{id} is not a capability identifier: {source}"))
                })?,
                capability_version: release.clone(),
                subject: subject.clone(),
                state: CapabilityState::VersionQualified,
                source: EvidenceSource::SignedRecord,
                profile_digest: basis.digest,
                statement: statement.clone(),
            })
        })
        .collect()
}

/// Everything that keeps the repository's records from matching its packages and its build list.
///
/// Every connector package has a record for the build its table is pinned to. Every record, pinned
/// or not, is one the repository can read for what it shows: for a connector package it publishes,
/// on the list's platform, in good form, for the release it publishes, and run against the version its
/// path names, which is the pinned application and SHA-256 where the list pins that build. None of
/// this needs a pin, so a build withdrawn by removing its pin keeps its record, and the record
/// still has to hold.
///
/// # Errors
///
/// Returns an error when the build list is refused or a record cannot be read.
pub fn check_records(root: &Path, packages: &[LoadedPackage]) -> Result<Vec<String>> {
    let Some(list) = read_list(root)? else {
        return Ok(vec![format!("{LIST} is missing")]);
    };
    let mut problems = Vec::new();
    for package in packages {
        let Some(connector) = &package.package.connector else {
            continue;
        };
        let manifest = &package.package.manifest;
        let tested = connector.protocol.tested_version.to_string();
        let relative = records::path(
            manifest.publisher_id.as_str(),
            manifest.plugin_name.as_str(),
            &tested,
            &list.platform,
        );
        if !root.join(&relative).is_file() {
            problems.push(format!(
                "{}: {} has no record for {tested}, the build its table is pinned to",
                relative.display(),
                manifest.plugin_id()
            ));
        }
    }
    for relative in record_files(root) {
        problems.extend(
            check_record(root, &relative, packages, &list)?
                .into_iter()
                .map(|problem| format!("{}: {problem}", relative.display())),
        );
    }
    Ok(problems)
}

/// What keeps one record file from being a record this repository can read for what it shows.
fn check_record(
    root: &Path,
    relative: &Path,
    packages: &[LoadedPackage],
    list: &List,
) -> Result<Vec<String>> {
    let named: Vec<String> = relative
        .strip_prefix(records::RECORDS)
        .unwrap_or(relative)
        .iter()
        .map(|part| part.to_string_lossy().into_owned())
        .collect();
    let [publisher, plugin, version, file] = named.as_slice() else {
        return Ok(vec![
            "not a record: a record is <publisher>/<plugin>/<version>/<os>-<arch>.json".to_owned(),
        ]);
    };
    let package = match connector_package(packages, &format!("{publisher}/{plugin}")) {
        Ok(package) => package,
        Err(what) => return Ok(vec![format!("a record for {what}")]),
    };
    if *file != format!("{}.json", list.platform.name()) {
        return Ok(vec![format!(
            "a record for another platform than {}, the one the build list is for",
            list.platform
        )]);
    }
    let record: Value = read_json(&root.join(relative))?;
    let mut found = records::form_problems(&record, &list.platform);
    if !found.is_empty() {
        return Ok(found);
    }
    found.extend(records::release_problems(&record, package));
    let plugin_id = package.package.manifest.plugin_id().to_string();
    let pinned = list
        .builds
        .iter()
        .find(|pinned| pinned.package == plugin_id && pinned.version.to_string() == *version);
    match pinned {
        Some(pinned) => found.extend(records::build_problems(
            &record,
            pinned.application.as_str(),
            version,
            &pinned.sha256.to_string(),
        )),
        None => {
            let tested = records::tested_build(&record);
            if tested["version"] != version.as_str() || tested["status"] != "installed" {
                found.push(format!(
                    "the record's parts ran against version {} ({}), not {version}, the version \
                     its path names",
                    tested["version"], tested["status"]
                ));
            }
        }
    }
    Ok(found)
}

/// Every record file under the records directory, relative to the repository root: the files four
/// directories down, which is where a record for one publisher, plugin, version and platform is.
fn record_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut directories = vec![(root.join(records::RECORDS), 0_usize)];
    while let Some((directory, depth)) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                directories.push((path, depth + 1));
            } else if depth == 3
                && let Ok(relative) = path.strip_prefix(root)
            {
                found.push(relative.to_path_buf());
            }
        }
    }
    found.sort();
    found
}
