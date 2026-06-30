//! Guard test for the mono-user invariant (DESIGN §6, CLAUDE.md).
//!
//! FreePass is mono-user, no server auth, no multi-tenancy: a vault is one
//! person on one machine. We must never accidentally introduce columns that
//! would imply otherwise (`user_id`, `owner_id`, a future `assignee`, etc.) —
//! they rot as dead columns and erode the invariant.
//!
//! This test scans every `migrations/*.sql` file and fails if a forbidden
//! identifier appears as a standalone token.

use std::fs;
use std::path::Path;

const FORBIDDEN_IDENTIFIERS: &[&str] = &[
    "user_id",
    "owner_id",
    "tenant_id",
    "created_by",
    "author_id",
    "assignee",
    // Extra mono-user smells, kept in sync with the spirit of DESIGN §6.
    "assignee_id",
    "updated_by",
    "organization_id",
    "workspace_id",
];

#[test]
fn migrations_do_not_reference_multi_user_columns() {
    let migrations_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let entries = fs::read_dir(&migrations_dir)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", migrations_dir.display()));

    let mut violations: Vec<String> = Vec::new();
    let mut scanned = 0usize;

    for entry in entries {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("sql") {
            continue;
        }
        scanned += 1;

        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        // Scan SCHEMA, not prose. A migration's explanatory comment may legitimately
        // *name* a forbidden identifier (e.g. to document the mono-user invariant);
        // strip SQL comments first so only real DDL is checked.
        let lowered = strip_sql_comments(&content).to_lowercase();

        for forbidden in FORBIDDEN_IDENTIFIERS {
            if contains_identifier(&lowered, forbidden) {
                violations.push(format!(
                    "{}: forbidden identifier `{forbidden}` (mono-user invariant)",
                    path.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }

    assert!(
        scanned > 0,
        "no migrations found at {}",
        migrations_dir.display()
    );
    assert!(
        violations.is_empty(),
        "mono-user invariant violated. See DESIGN.md §6 / CLAUDE.md.\n{}",
        violations.join("\n")
    );
}

/// Phase 10 guard: prove the scan actually reaches the new `004_projects.sql`
/// migration (so a future principal column there could not slip through), and
/// that `project_id` — an OBJECT identifier, not a principal — is not flagged.
#[test]
fn migration_004_projects_is_scanned_and_clean() {
    let migrations_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let path = migrations_dir.join("004_projects.sql");
    assert!(
        path.exists(),
        "expected the Phase 10 migration at {}",
        path.display()
    );

    let content = fs::read_to_string(&path).expect("read 004_projects.sql");
    let lowered = strip_sql_comments(&content).to_lowercase();

    // It really does add `project_id` (object id) — and the guard does NOT flag
    // it (the forbidden list is principal identifiers only).
    assert!(
        contains_identifier(&lowered, "project_id"),
        "004 should add the project_id column"
    );
    for forbidden in FORBIDDEN_IDENTIFIERS {
        assert!(
            !contains_identifier(&lowered, forbidden),
            "004_projects.sql leaked a forbidden principal identifier `{forbidden}`"
        );
    }
}

/// True iff `needle` appears in `haystack` as a standalone identifier
/// (not surrounded by `[a-z0-9_]`). `haystack` must already be lowercased.
fn contains_identifier(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let nbytes = needle.as_bytes();
    if nbytes.is_empty() || bytes.len() < nbytes.len() {
        return false;
    }
    for i in 0..=bytes.len() - nbytes.len() {
        if &bytes[i..i + nbytes.len()] != nbytes {
            continue;
        }
        let before_ok = i == 0 || !is_ident_byte(bytes[i - 1]);
        let after = i + nbytes.len();
        let after_ok = after == bytes.len() || !is_ident_byte(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Remove SQL comments (`/* ... */` blocks and `-- ... EOL` lines) so the scan
/// only sees DDL, not documentation prose. UTF-8 safe. Conservative: does not
/// honor string literals (our migrations never embed `--`/`/*` inside values).
fn strip_sql_comments(sql: &str) -> String {
    // Pass 1: drop /* ... */ block comments (may span lines).
    let chars: Vec<char> = sql.chars().collect();
    let mut without_blocks = String::with_capacity(sql.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len()); // skip the closing */
            without_blocks.push(' ');
        } else {
            without_blocks.push(chars[i]);
            i += 1;
        }
    }
    // Pass 2: drop -- line comments (slice at the ASCII "--", a valid char boundary).
    without_blocks
        .lines()
        .map(|line| match line.find("--") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::{contains_identifier, strip_sql_comments};

    #[test]
    fn strips_comments_so_documented_forbidden_words_are_ignored() {
        let sql = "-- this lists user_id as forbidden\n\
                   CREATE TABLE vault (id INTEGER); /* owner_id, tenant_id note */";
        let stripped = strip_sql_comments(sql);
        assert!(!stripped.contains("user_id"), "line comment not stripped");
        assert!(!stripped.contains("owner_id"), "block comment not stripped");
        assert!(!stripped.contains("tenant_id"), "block comment not stripped");
        assert!(stripped.contains("CREATE TABLE vault"), "DDL must survive");
    }

    #[test]
    fn matches_standalone_identifier() {
        assert!(contains_identifier("user_id text", "user_id"));
        assert!(contains_identifier("(user_id),", "user_id"));
        assert!(contains_identifier("user_id\n", "user_id"));
    }

    #[test]
    fn ignores_substring_inside_other_identifier() {
        assert!(!contains_identifier("superuser_id_thing", "user_id"));
        assert!(!contains_identifier("xuser_idy", "user_id"));
        assert!(!contains_identifier("created_byte", "created_by"));
    }
}
