//! Research-study services (see `paper/instrument/`). Active only under
//! `STUDY_MODE`; gated at the route layer.
//!
//! `paper/` — manuscript, replication package, and the build/operator docs this
//! module and its siblings cite — is tracked on the `paper/tosm-journal-writing`
//! branch, so those paths resolve there rather than on this one until it merges.

pub mod access_code;
pub mod agent;
pub mod export;
pub mod github_fixtures;
pub mod provision;
pub mod seed;
pub mod survey;
pub mod telemetry;

pub use provision::{ProvisionResult, provision_participant};
