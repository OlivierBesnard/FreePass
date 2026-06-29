//! Entry DTOs (DESIGN §4). Clear metadata (`title`, `url`) travels in summaries;
//! decrypted secret values only appear in `EntryDetail`, returned by `get_entry`
//! for the entry the user explicitly opened — never in list responses (F5).

use serde::{Deserialize, Serialize};

/// One environment, identified for scoping CRUD calls.
#[derive(Debug, Serialize)]
pub struct EnvironmentInfo {
    pub id: String,
    pub name: String,
}

/// List-view row: clear metadata only, no secret material.
#[derive(Debug, Serialize)]
pub struct EntrySummary {
    pub id: String,
    pub env_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub url: Option<String>,
    pub updated_at: String,
}

/// Full entry with its decrypted fields, for the opened entry.
#[derive(Debug, Serialize)]
pub struct EntryDetail {
    pub id: String,
    pub env_id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
    /// Site favicon as a `data:` URL, fetched best-effort and stored encrypted
    /// like any other field. `None` if never fetched or not found.
    pub icon: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Create/update payload for a login entry. Secret fields are encrypted in the
/// service before they touch the DB.
#[derive(Debug, Deserialize)]
pub struct EntryInput {
    pub title: String,
    pub url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub notes: Option<String>,
}
