//! Research-study services (see `paper/instrument/`). Active only under
//! `STUDY_MODE`; gated at the route layer.
//!
//! `paper/` — manuscript, replication package, and the build/operator docs this
//! module and its siblings cite — is tracked on the `paper/tosm-journal-writing`
//! branch and deliberately kept off this one, so the product line carries no
//! research material. Those paths resolve on that branch, which carries this
//! code too. Check it out there rather than going looking here.

pub mod access_code;
pub mod agent;
pub mod export;
pub mod github_fixtures;
pub mod provision;
pub mod seed;
pub mod survey;
pub mod telemetry;

pub use provision::{ProvisionResult, provision_participant};
