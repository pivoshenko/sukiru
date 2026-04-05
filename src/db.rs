use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::PathBuf;

use crate::error::{err, Result};
use crate::fsops::{dirs_kasetto_data, now_iso, now_unix};
use crate::model::{Report, SkillEntry, State, SyncFailure};

fn db_path() -> Result<PathBuf> {
    Ok(dirs_kasetto_data()?.join("manifest.db"))
}

fn open_db() -> Result<(Connection, PathBuf)> {
    let path = db_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(&path)?;
    init_db(&conn)?;
    Ok((conn, path))
}

fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        PRAGMA synchronous=NORMAL;
        PRAGMA cache_size=-8000;
        PRAGMA temp_store=MEMORY;
        CREATE TABLE IF NOT EXISTS meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS skills (
            id TEXT PRIMARY KEY,
            destination TEXT NOT NULL,
            hash TEXT NOT NULL,
            skill TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            source TEXT NOT NULL,
            source_revision TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_skills_source ON skills(source);
        CREATE INDEX IF NOT EXISTS idx_skills_destination ON skills(destination);
        CREATE TABLE IF NOT EXISTS assets (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            name TEXT NOT NULL,
            hash TEXT NOT NULL,
            source TEXT NOT NULL,
            destination TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_assets_kind ON assets(kind);
        CREATE TABLE IF NOT EXISTS reports (
            run_id TEXT PRIMARY KEY,
            created_at INTEGER NOT NULL,
            report_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_reports_created_at ON reports(created_at);
        "#,
    )?;
    Ok(())
}

fn persist_state(conn: &mut Connection, state: &State) -> Result<()> {
    let tx = conn.transaction()?;
    let mut existing_ids = HashSet::new();
    {
        let mut stmt = tx.prepare("SELECT id FROM skills")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        for row in rows {
            existing_ids.insert(row?);
        }
    }
    let mut current_ids = HashSet::new();

    for (id, entry) in &state.skills {
        current_ids.insert(id.clone());
        tx.execute(
            "INSERT INTO skills (id, destination, hash, skill, description, source, source_revision, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
               destination=excluded.destination,
               hash=excluded.hash,
               skill=excluded.skill,
               description=excluded.description,
               source=excluded.source,
               source_revision=excluded.source_revision,
               updated_at=excluded.updated_at",
            params![
                id,
                &entry.destination,
                &entry.hash,
                &entry.skill,
                &entry.description,
                &entry.source,
                &entry.source_revision,
                &entry.updated_at
            ],
        )?;
    }

    for stale_id in existing_ids.difference(&current_ids) {
        tx.execute("DELETE FROM skills WHERE id = ?1", params![stale_id])?;
    }

    match &state.last_run {
        Some(last_run) => {
            tx.execute(
                "INSERT INTO meta (key, value) VALUES ('last_run', ?1)
                 ON CONFLICT(key) DO UPDATE SET value=excluded.value",
                params![last_run],
            )?;
        }
        None => {
            tx.execute("DELETE FROM meta WHERE key = 'last_run'", [])?;
        }
    }

    tx.commit()?;
    Ok(())
}

pub(crate) fn load_state() -> Result<State> {
    let (conn, _) = open_db()?;
    let last_run = conn
        .query_row("SELECT value FROM meta WHERE key = 'last_run'", [], |row| {
            row.get::<_, String>(0)
        })
        .optional()?;

    let mut skills = BTreeMap::new();
    let mut stmt = conn.prepare(
        "SELECT id, destination, hash, skill, description, source, source_revision, updated_at FROM skills",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            SkillEntry {
                destination: row.get(1)?,
                hash: row.get(2)?,
                skill: row.get(3)?,
                description: row.get(4)?,
                source: row.get(5)?,
                source_revision: row.get(6)?,
                updated_at: row.get(7)?,
            },
        ))
    })?;

    for row in rows {
        let (key, entry) = row?;
        skills.insert(key, entry);
    }

    Ok(State {
        version: 1,
        last_run,
        skills,
    })
}

pub(crate) fn save_state(state: &State) -> Result<()> {
    let (mut conn, _) = open_db()?;
    persist_state(&mut conn, state)?;
    Ok(())
}

pub(crate) fn save_report(report: &Report) -> Result<PathBuf> {
    let (conn, db_path) = open_db()?;
    conn.execute(
        "INSERT INTO reports (run_id, created_at, report_json) VALUES (?1, ?2, ?3)
         ON CONFLICT(run_id) DO UPDATE SET created_at=excluded.created_at, report_json=excluded.report_json",
        params![&report.run_id, now_unix() as i64, serde_json::to_string(report)?],
    )?;
    Ok(db_path)
}

pub(crate) fn manifest_db_path() -> Result<PathBuf> {
    db_path()
}

pub(crate) fn load_latest_failures() -> Result<Vec<SyncFailure>> {
    let (conn, _) = open_db()?;
    let latest_report_json = conn
        .query_row(
            "SELECT report_json FROM reports ORDER BY created_at DESC, rowid DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let Some(report_json) = latest_report_json else {
        return Ok(Vec::new());
    };

    let value: serde_json::Value = serde_json::from_str(&report_json)
        .map_err(|e| err(format!("failed to parse latest report JSON: {e}")))?;
    let mut failed = Vec::new();

    if let Some(actions) = value.get("actions").and_then(|v| v.as_array()) {
        for action in actions {
            let status = action.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if status != "broken" && status != "source_error" {
                continue;
            }
            failed.push(SyncFailure {
                name: action
                    .get("skill")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-")
                    .to_string(),
                source: action
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-")
                    .to_string(),
                reason: action
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown reason")
                    .to_string(),
            });
        }
    }

    Ok(failed)
}

pub(crate) fn get_tracked_asset(kind: &str, id: &str) -> Result<Option<(String, String)>> {
    let (conn, _) = open_db()?;
    let result = conn
        .query_row(
            "SELECT hash, destination FROM assets WHERE id = ?1 AND kind = ?2",
            params![id, kind],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    Ok(result)
}

pub(crate) fn save_tracked_asset(
    kind: &str,
    id: &str,
    name: &str,
    hash: &str,
    source: &str,
    destination: &str,
) -> Result<()> {
    let (conn, _) = open_db()?;
    conn.execute(
        "INSERT INTO assets (id, kind, name, hash, source, destination, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
           kind=excluded.kind, name=excluded.name, hash=excluded.hash,
           source=excluded.source, destination=excluded.destination,
           updated_at=excluded.updated_at",
        params![id, kind, name, hash, source, destination, now_iso()],
    )?;
    Ok(())
}

pub(crate) fn remove_tracked_asset(id: &str) -> Result<()> {
    let (conn, _) = open_db()?;
    conn.execute("DELETE FROM assets WHERE id = ?1", params![id])?;
    Ok(())
}

pub(crate) fn clear_all() -> Result<()> {
    let (conn, _) = open_db()?;
    conn.execute_batch("DELETE FROM skills; DELETE FROM assets; DELETE FROM reports; DELETE FROM meta WHERE key = 'last_run';")?;
    Ok(())
}

pub(crate) fn list_tracked_asset_ids(kind: &str) -> Result<Vec<(String, String)>> {
    let (conn, _) = open_db()?;
    let mut stmt = conn.prepare("SELECT id, destination FROM assets WHERE kind = ?1")?;
    let rows = stmt.query_map(params![kind], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        init_db(&conn).expect("init schema");
        conn
    }

    #[test]
    fn persist_and_load_skills() {
        let mut conn = mem_db();
        let mut state = State {
            last_run: Some("12345".to_string()),
            ..Default::default()
        };
        state.skills.insert(
            "src::skill-a".to_string(),
            SkillEntry {
                destination: "/tmp/skill-a".into(),
                hash: "abc".into(),
                skill: "skill-a".into(),
                description: "desc".into(),
                source: "src".into(),
                source_revision: "rev1".into(),
                updated_at: "100".into(),
            },
        );

        persist_state(&mut conn, &state).expect("persist");

        let row: (String, String) = conn
            .query_row(
                "SELECT skill, hash FROM skills WHERE id = ?1",
                ["src::skill-a"],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("query");
        assert_eq!(row.0, "skill-a");
        assert_eq!(row.1, "abc");

        let last_run: String = conn
            .query_row("SELECT value FROM meta WHERE key = 'last_run'", [], |r| {
                r.get(0)
            })
            .expect("last_run");
        assert_eq!(last_run, "12345");
    }

    #[test]
    fn persist_removes_stale_skills() {
        let mut conn = mem_db();

        let mut state = State::default();
        state.skills.insert(
            "src::old".to_string(),
            SkillEntry {
                skill: "old".into(),
                hash: "h1".into(),
                ..Default::default()
            },
        );
        persist_state(&mut conn, &state).expect("persist1");

        state.skills.clear();
        state.skills.insert(
            "src::new".to_string(),
            SkillEntry {
                skill: "new".into(),
                hash: "h2".into(),
                ..Default::default()
            },
        );
        persist_state(&mut conn, &state).expect("persist2");

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM skills", [], |r| r.get(0))
            .expect("count");
        assert_eq!(count, 1);

        let skill: String = conn
            .query_row("SELECT skill FROM skills", [], |r| r.get(0))
            .expect("skill");
        assert_eq!(skill, "new");
    }

    #[test]
    fn persist_clears_last_run_when_none() {
        let mut conn = mem_db();

        persist_state(
            &mut conn,
            &State {
                last_run: Some("999".to_string()),
                ..Default::default()
            },
        )
        .expect("persist with last_run");

        persist_state(
            &mut conn,
            &State {
                last_run: None,
                ..Default::default()
            },
        )
        .expect("persist without last_run");

        let result: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key = 'last_run'", [], |r| {
                r.get(0)
            })
            .optional()
            .expect("query");
        assert!(result.is_none());
    }
}
