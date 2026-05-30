//! Research-study services (see `research-paper/`). Active only under
//! `STUDY_MODE`; gated at the route layer.

pub mod agent;
pub mod github_fixtures;
pub mod provision;
pub mod seed;
pub mod survey;
pub mod telemetry;

pub use provision::{ProvisionResult, provision_participant};
