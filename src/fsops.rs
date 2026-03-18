use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{err, Result};
use crate::model::{
    Agent, Config, FailedInstall, Report, SkillEntry, SkillTarget, SkillsField, SourceSpec,
    State,
};

pub fn load_config_any(config_path: &str) -> Result<(Config, PathBuf, String)> {
    if config_path.starts_with("http://") || config_path.starts_with("https://") {
        let response = reqwest::blocking::get(config_path)
            .map_err(|e| err(format!("failed to fetch remote config: {config_path}: {e}")))?;
        let text = response
            .error_for_status()
            .map_err(|e| {
                err(format!(
                    "remote config returned non-success status: {config_path}: {e}"
                ))
            })?
            .text()?;
        let cfg: Config = serde_yaml::from_str(&text)?;
        let cfg_dir = std::env::current_dir()
            .map_err(|e| err(format!("failed to get current directory: {e}")))?;
        return Ok((cfg, cfg_dir, config_path.to_string()));
    }

    let cfg_abs = fs::canonicalize(config_path)
        .map_err(|e| err(format!("config not found: {config_path}: {e}")))?;
    let cfg_text = fs::read_to_string(&cfg_abs)?;
    let cfg: Config = serde_yaml::from_str(&cfg_text)?;
    let cfg_dir = cfg_abs
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| err("invalid config path"))?;
    Ok((cfg, cfg_dir, cfg_abs.to_string_lossy().to_string()))
}

pub fn materialize_source(
    src: &SourceSpec,
    cfg_dir: &Path,
    stage: &Path,
) -> Result<(PathBuf, String, HashMap<String, PathBuf>)> {
    if src.source.contains("://") {
        let (owner, repo) = parse_github(&src.source)?;
        let branch = src.branch.clone().unwrap_or_else(|| "main".into());
        let url = format!("https://codeload.github.com/{owner}/{repo}/tar.gz/refs/heads/{branch}");
        download_extract(&url, stage).or_else(|_| {
            if src.branch.is_none() {
                let url2 =
                    format!("https://codeload.github.com/{owner}/{repo}/tar.gz/refs/heads/master");
                download_extract(&url2, stage)
            } else {
                Err(err("failed to download source"))
            }
        })?;
        let available = discover(stage)?;
        Ok((stage.to_path_buf(), format!("branch:{branch}"), available))
    } else {
        let root = resolve_path(cfg_dir, &src.source);
        let available = discover(&root)?;
        Ok((root, "local".into(), available))
    }
}

pub fn select_targets(
    sf: &SkillsField,
    available: &HashMap<String, PathBuf>,
) -> Result<(Vec<(String, PathBuf)>, Vec<BrokenSkill>)> {
    let mut out = Vec::new();
    let mut broken = Vec::new();
    match sf {
        SkillsField::Wildcard(s) if s == "*" => {
            for (k, v) in available {
                out.push((k.clone(), v.clone()));
            }
        }
        SkillsField::List(items) => {
            for it in items {
                match it {
                    SkillTarget::Name(name) => {
                        if let Some(p) = available.get(name) {
                            out.push((name.clone(), p.clone()));
                        } else {
                            broken.push(BrokenSkill {
                                name: name.clone(),
                                reason: format!("skill not found: {name}"),
                            });
                        }
                    }
                    SkillTarget::Obj { name, path } => {
                        if let Some(path) = path {
                            let d = PathBuf::from(path).join(name);
                            if d.join("SKILL.md").exists() {
                                out.push((name.clone(), d));
                                continue;
                            }
                        }
                        if let Some(p) = available.get(name) {
                            out.push((name.clone(), p.clone()));
                        } else {
                            broken.push(BrokenSkill {
                                name: name.clone(),
                                reason: format!("skill not found: {name}"),
                            });
                        }
                    }
                }
            }
        }
        _ => return Err(err("invalid skills field")),
    }
    Ok((out, broken))
}

#[derive(Debug)]
pub struct BrokenSkill {
    pub name: String,
    pub reason: String,
}

pub fn hash_dir(path: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files(path, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    for f in files {
        let rel = f.strip_prefix(path)?.to_string_lossy();
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        let mut file = fs::File::open(&f)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        hasher.update(&buf);
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    copy_dir_contents(src, dst)
}

pub fn resolve_path(base: &Path, raw: &str) -> PathBuf {
    let p = PathBuf::from(
        raw.replace(
            '~',
            &dirs_home()
                .unwrap_or_else(|_| PathBuf::from("~"))
                .to_string_lossy(),
        ),
    );
    if p.is_absolute() {
        p
    } else {
        base.join(p)
    }
}

pub fn resolve_destination(base: &Path, cfg: &Config) -> Result<PathBuf> {
    if let Some(destination) = cfg.destination.as_deref() {
        return Ok(resolve_path(base, destination));
    }

    if let Some(agent) = cfg.agent {
        let home = dirs_home()?;
        let preset = match agent {
            Agent::Codex => home.join(".codex/skills"),
            Agent::Claude => home.join(".claude/skills"),
            Agent::Cursor => home.join(".cursor/skills"),
        };
        return Ok(preset);
    }

    Err(err(
        "config must define either destination or agent (codex|claude|cursor)",
    ))
}

pub fn dirs_home() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| err("HOME is not set"))
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub fn now_iso() -> String {
    chrono_like_now()
}

pub fn load_state() -> Result<State> {
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

pub fn save_state(state: &State) -> Result<()> {
    let (mut conn, _) = open_db()?;
    persist_state(&mut conn, state)?;
    Ok(())
}

pub fn save_report(report: &Report) -> Result<PathBuf> {
    let (conn, db_path) = open_db()?;
    conn.execute(
        "INSERT INTO reports (run_id, created_at, report_json) VALUES (?1, ?2, ?3)
         ON CONFLICT(run_id) DO UPDATE SET created_at=excluded.created_at, report_json=excluded.report_json",
        params![&report.run_id, now_unix() as i64, serde_json::to_string(report)?],
    )?;
    Ok(db_path)
}

pub fn manifest_db_path() -> Result<PathBuf> {
    db_path()
}

pub fn load_latest_failed_installs() -> Result<Vec<FailedInstall>> {
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
            if status != "broken" {
                continue;
            }
            failed.push(FailedInstall {
                skill: action
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

fn parse_github(url: &str) -> Result<(String, String)> {
    let p = url.trim_end_matches('/').trim_end_matches(".git");
    let parts: Vec<_> = p.split('/').collect();
    if parts.len() < 2 {
        return Err(err("unsupported github url"));
    }
    Ok((
        parts[parts.len() - 2].to_string(),
        parts[parts.len() - 1].to_string(),
    ))
}

fn discover(root: &Path) -> Result<HashMap<String, PathBuf>> {
    let mut out = HashMap::new();
    for base in [root.to_path_buf(), root.join("skills")] {
        if !base.exists() {
            continue;
        }
        for e in fs::read_dir(base)? {
            let e = e?;
            if !e.file_type()?.is_dir() {
                continue;
            }
            let d = e.path();
            if d.join("SKILL.md").exists() {
                out.insert(e.file_name().to_string_lossy().to_string(), d);
            }
        }
    }
    Ok(out)
}

fn download_extract(url: &str, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    let body = reqwest::blocking::get(url)?.bytes()?;
    let gz = flate2::read::GzDecoder::new(body.as_ref());
    let mut archive = tar::Archive::new(gz);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let p = entry.path()?;
        let parts: Vec<_> = p.components().collect();
        if parts.len() < 2 {
            continue;
        }
        let rel = parts
            .iter()
            .skip(1)
            .map(|c| c.as_os_str())
            .collect::<PathBuf>();
        if rel.to_string_lossy().contains("..") {
            return Err(err("unsafe archive path"));
        }
        let target = dst.join(rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        entry.unpack(target)?;
    }
    Ok(())
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let path = entry.path();
        if file_type.is_dir() {
            collect_files(&path, out)?;
        } else if file_type.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&target)?;
            copy_dir_contents(&src_path, &target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(src_path, target)?;
        }
    }
    Ok(())
}

fn db_path() -> Result<PathBuf> {
    Ok(dirs_home()?.join(".kst/manifest.db"))
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
        CREATE TABLE IF NOT EXISTS reports (
            run_id TEXT PRIMARY KEY,
            created_at INTEGER NOT NULL,
            report_json TEXT NOT NULL
        );
        "#,
    )?;
    Ok(())
}

fn persist_state(conn: &mut Connection, state: &State) -> Result<()> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM skills", [])?;

    for (id, entry) in &state.skills {
        tx.execute(
            "INSERT INTO skills (id, destination, hash, skill, description, source, source_revision, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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

fn chrono_like_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}", now)
}
