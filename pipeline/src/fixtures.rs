//! Package conformance fixtures.
//!
//! A declarative package's behaviour is its predicates: which controls a person sees, and which of
//! those they can use, given what the host currently knows. A fixture states that behaviour as
//! cases, and the pipeline evaluates the package's own predicates against each case on every run.
//!
//! This is what stops a predicate edit from quietly changing what a person sees. The package
//! declares the answer, and the build checks it.

use std::collections::BTreeSet;
use std::path::Path;

use kr_plugin_sdk::capability::{CapabilityState, PluginCapability};
use kr_plugin_sdk::effect::ActionRight;
use kr_plugin_sdk::ids::NodeId;
use kr_plugin_sdk::package::Package;
use kr_plugin_sdk::predicate::{BindingState, PredicateContext, PresentationFlag};
use kr_plugin_sdk::presentation::Control;
use serde::{Deserialize, Serialize};

use crate::{Error, Result, read_json};

/// The fixture file a package may carry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureFile {
    /// The fixture format version.
    pub fixture_version: u32,
    /// The visibility cases.
    pub visibility: Vec<VisibilityCase>,
}

impl FixtureFile {
    /// The fixture format version this pipeline reads.
    pub const CURRENT_VERSION: u32 = 1;
}

/// One case: a context, and the controls it should show and enable.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VisibilityCase {
    /// What the case is called.
    pub name: String,
    /// What the host knows.
    pub context: FixtureContext,
    /// The control identifiers a person should see, in any order.
    pub visible_controls: BTreeSet<String>,
    /// The control identifiers a person should be able to use, in any order.
    pub enabled_controls: BTreeSet<String>,
}

/// What the host knows in one fixture case.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureContext {
    /// The rights the actor holds.
    pub rights: Vec<ActionRight>,
    /// The binding's state.
    pub binding_state: BindingState,
    /// The presentation facts that hold.
    pub flags: Vec<PresentationFlag>,
    /// The capability states the host has evidence for.
    pub capabilities: Vec<(PluginCapability, CapabilityState)>,
    /// The nodes present in the document.
    pub present_nodes: Vec<NodeId>,
}

/// Runs a package's fixtures against its own predicates.
///
/// # Errors
///
/// Returns [`Error::FixtureMismatch`] when a case's declared answer is not the answer the
/// package's predicates produce, and a read error when the fixture file cannot be read.
pub fn check(name: &str, directory: &Path, package: &Package) -> Result<usize> {
    let fixtures: Vec<_> = package
        .manifest
        .payloads
        .iter()
        .filter(|payload| payload.role == kr_plugin_sdk::plugin::PayloadRole::Fixture)
        .collect();
    if fixtures.is_empty() {
        return Ok(0);
    }

    let controls: Vec<&Control> = package
        .presentation
        .nodes
        .iter()
        .flat_map(|node| node.body.controls())
        .collect();

    // A package may carry several fixture files, and every one of them runs. Running only the
    // first would make adding a second file a way to stop the others being checked.
    let mut cases = 0usize;
    for payload in fixtures {
        cases += check_file(name, &directory.join(payload.path.as_str()), &controls)?;
    }
    Ok(cases)
}

/// Runs one fixture file against a package's controls.
fn check_file(name: &str, path: &Path, controls: &[&Control]) -> Result<usize> {
    let file: FixtureFile = read_json(path)?;
    if file.fixture_version != FixtureFile::CURRENT_VERSION {
        return Err(Error::FixtureMismatch {
            package: name.to_owned(),
            case: path.display().to_string(),
            detail: format!(
                "fixture version {} is not version {}",
                file.fixture_version,
                FixtureFile::CURRENT_VERSION
            ),
        });
    }

    for case in &file.visibility {
        let context = PredicateContext {
            capability_states: &case.context.capabilities,
            rights: &case.context.rights,
            binding_state: case.context.binding_state,
            present_nodes: &case.context.present_nodes,
            flags: &case.context.flags,
            // A package's own document is written before any request exists, so a fixture names
            // no upstream request and a term that tests one particular request is false here.
            pending_approvals: &[],
        };
        let visible: BTreeSet<String> = controls
            .iter()
            .filter(|control| control.visible_when.evaluate(&context))
            .map(|control| control.id.to_string())
            .collect();
        if visible != case.visible_controls {
            return Err(Error::FixtureMismatch {
                package: name.to_owned(),
                case: case.name.clone(),
                detail: format!(
                    "visible controls are {visible:?}, the fixture declares {:?}",
                    case.visible_controls
                ),
            });
        }
        let enabled: BTreeSet<String> = controls
            .iter()
            .filter(|control| {
                control.visible_when.evaluate(&context) && control.enabled_when.evaluate(&context)
            })
            .map(|control| control.id.to_string())
            .collect();
        if enabled != case.enabled_controls {
            return Err(Error::FixtureMismatch {
                package: name.to_owned(),
                case: case.name.clone(),
                detail: format!(
                    "enabled controls are {enabled:?}, the fixture declares {:?}",
                    case.enabled_controls
                ),
            });
        }
    }
    Ok(file.visibility.len())
}
