use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "global_role", rename_all = "lowercase")
)]
pub enum GlobalRole {
    Admin,
    Member,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "sqlx", derive(sqlx::Type))]
#[cfg_attr(
    feature = "sqlx",
    sqlx(type_name = "team_role", rename_all = "lowercase")
)]
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
