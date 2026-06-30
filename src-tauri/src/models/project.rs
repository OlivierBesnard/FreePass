//! Project DTOs (DESIGN §4, PLAN Phase 10). A project is a clear-metadata
//! grouping layer above environments — never a principal (mono-user, DESIGN §6).
//! Serialized snake_case, like `EnvironmentInfo` / `EntrySummary`.

use serde::Serialize;

/// One project, identified for scoping CRUD calls and grouping environments.
#[derive(Debug, Serialize)]
pub struct ProjectInfo {
    pub id: String,         // UUID v4
    pub name: String,       // clear metadata (free FR text)
    pub created_at: String, // RFC3339
    pub updated_at: String,
}
