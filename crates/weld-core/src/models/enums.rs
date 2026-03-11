use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GlobalRole {
    Admin,
    Member,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TeamRole {
    Owner,
    Admin,
    Member,
    Viewer,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Priority {
    Critical,
    High,
    Medium,
    Low,
    None,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Environment {
    Development,
    Staging,
    Production,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimesheetStatus {
    Draft,
    Submitted,
    Approved,
    Rejected,
}
