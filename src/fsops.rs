use reqwest::blocking::Client;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{err, Result};
use crate::model::{Config, SkillTarget, SkillsField};
use crate::source::{auth_env_inline_help, auth_for_request_url, http_fetch_auth_hint, rewrite_gitlab_raw_url};

pub(crate) fn load_config_any(config_path: &str) -> Result<(Config, PathBuf, String)> {
    if config_path.starts_with("http://") || config_path.starts_with("https://") {
        let fetch_url =
            rewrite_gitlab_raw_url(config_path).unwrap_or_else(|| config_path.to_string());
        let auth = auth_for_request_url(config_path);
        let request = auth.apply(http_client()?.get(&fetch_url));
        let response = request
            .send()
            .map_err(|e| err(format!("failed to fetch remote config: {config_path}: {e}")))?;
        let status = response.status().as_u16();
        let text = response
            .text()
            .map_err(|e| err(format!("failed to read remote config body for {config_path}: {e}")))?;
        if !(200..300).contains(&status) {
            return Err(err(format!(
                "remote config returned HTTP {status} for {config_path}{}",
                http_fetch_auth_hint(config_path, status)
            )));
        }
        if text.trim_start().starts_with("<!DOCTYPE") || text.trim_start().starts_with("<html") {
            return Err(err(format!(
                "remote config at {config_path} returned a login/HTML page instead of YAML — {}",
                auth_env_inline_help(config_path)
            )));
        }
        let cfg: Config = serde_yaml::from_str(&text)?;
        let cfg_dir = std::env::current_dir()
            .map_err(|e| err(format!("failed to get current directory: {e}")))?;
        return Ok((cfg, cfg_dir, config_path.to_string()));
    }

    let cfg_abs = fs::canonicalize(config_path)
        .map_err(|_| err(format!("config not found: {config_path}")))?;
    let cfg_text = fs::read_to_string(&cfg_abs)?;
    let cfg: Config = serde_yaml::from_str(&cfg_text)?;
    let cfg_dir = cfg_abs
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| err("invalid config path"))?;
    Ok((cfg, cfg_dir, cfg_abs.to_string_lossy().to_string()))
}

pub(crate) type TargetSelection = (Vec<(String, PathBuf)>, Vec<BrokenSkill>);

pub(crate) fn select_targets(
    sf: &SkillsField,
    available: &HashMap<String, PathBuf>,
) -> Result<TargetSelection> {
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
pub(crate) struct BrokenSkill {
    pub name: String,
    pub reason: String,
}

pub(crate) fn hash_dir(path: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect_files(path, &mut files)?;
    files.sort();

    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    for f in files {
        let rel = f.strip_prefix(path)?.to_string_lossy();
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        let file = fs::File::open(&f)?;
        let mut reader = BufReader::new(file);
        sha256_update_reader(&mut reader, &mut hasher, &mut buf)?;
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Hash a single file (for MCPs tracking).
pub(crate) fn hash_file(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let file = fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buf = [0u8; 8192];
    sha256_update_reader(&mut reader, &mut hasher, &mut buf)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn sha256_update_reader<R: Read>(
    reader: &mut R,
    hasher: &mut Sha256,
    buf: &mut [u8; 8192],
) -> Result<()> {
    loop {
        let n = reader.read(buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(())
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    copy_dir_contents(src, dst)
}

pub(crate) fn resolve_path(base: &Path, raw: &str) -> PathBuf {
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

/// Returns one skills path per configured agent.
/// Falls back to explicit `destination` if set.
pub(crate) fn resolve_destinations(base: &Path, cfg: &Config) -> Result<Vec<PathBuf>> {
    if let Some(destination) = cfg.destination.as_deref() {
        return Ok(vec![resolve_path(base, destination)]);
    }
    let agents = cfg.agents();
    if agents.is_empty() {
        return Err(err(
            "config must define either destination or a supported agent preset",
        ));
    }
    let home = dirs_home()?;
    Ok(agents.iter().map(|a| a.global_path(&home)).collect())
}

/// Returns one MCP settings path per configured agent.
pub(crate) fn resolve_mcp_settings_targets(
    cfg: &Config,
) -> Result<Vec<crate::model::McpSettingsTarget>> {
    let agents = cfg.agents();
    if agents.is_empty() {
        return Ok(vec![]);
    }
    let home = dirs_home()?;
    let kasetto_config = dirs_kasetto_config()?;
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut out = Vec::new();
    for a in agents {
        let t = a.mcp_settings_target(&home, &kasetto_config);
        if seen.insert(t.path.clone()) {
            out.push(t);
        }
    }
    Ok(out)
}

pub(crate) fn dirs_home() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| err("HOME is not set"))
}

/// [XDG Base Directory](https://specifications.freedesktop.org/basedir-spec/latest/) config home:
/// `XDG_CONFIG_HOME`, or `$HOME/.config` when unset or empty.
pub(crate) fn dirs_xdg_config_home() -> Result<PathBuf> {
    match std::env::var("XDG_CONFIG_HOME") {
        Ok(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => Ok(dirs_home()?.join(".config")),
    }
}

/// Per-user Kasetto configuration directory: `$XDG_CONFIG_HOME/kasetto`.
pub(crate) fn dirs_kasetto_config() -> Result<PathBuf> {
    Ok(dirs_xdg_config_home()?.join("kasetto"))
}

/// [XDG Base Directory](https://specifications.freedesktop.org/basedir-spec/latest/) data home:
/// `XDG_DATA_HOME`, or `$HOME/.local/share` when unset or empty.
pub(crate) fn dirs_xdg_data_home() -> Result<PathBuf> {
    match std::env::var("XDG_DATA_HOME") {
        Ok(p) if !p.is_empty() => Ok(PathBuf::from(p)),
        _ => Ok(dirs_home()?.join(".local/share")),
    }
}

/// Per-user Kasetto data directory (manifest DB, etc.): `$XDG_DATA_HOME/kasetto`.
pub(crate) fn dirs_kasetto_data() -> Result<PathBuf> {
    Ok(dirs_xdg_data_home()?.join("kasetto"))
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

pub(crate) fn now_iso() -> String {
    format!("{}", now_unix())
}

/// Wrapper for loading, mutating, and saving agent settings JSON files.
pub(crate) struct SettingsFile {
    path: PathBuf,
    pub data: serde_json::Value,
}

impl SettingsFile {
    /// Load an existing JSON file or start with an empty `{}`.
    pub(crate) fn load(path: &Path) -> Result<Self> {
        let data = if path.exists() {
            let text = fs::read_to_string(path)?;
            serde_json::from_str(&text)
                .map_err(|e| err(format!("invalid settings JSON {}: {e}", path.display())))?
        } else {
            serde_json::json!({})
        };
        Ok(Self {
            path: path.to_path_buf(),
            data,
        })
    }

    /// Write pretty-printed JSON back to disk, creating parent dirs if needed.
    pub(crate) fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(&self.data)?)?;
        Ok(())
    }
}

static HTTP_CLIENT: OnceLock<std::result::Result<Client, String>> = OnceLock::new();

/// Shared client: avoids TLS/session setup on every asset or config fetch.
pub(crate) fn http_client() -> Result<Client> {
    let built = HTTP_CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent(format!("kasetto/{}", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| e.to_string())
    });
    match built {
        Ok(c) => Ok(c.clone()),
        Err(e) => Err(err(format!("failed to build HTTP client: {e}"))),
    }
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
            let reader = BufReader::new(fs::File::open(&src_path)?);
            let mut writer = BufWriter::new(fs::File::create(&target)?);
            let mut buf_reader = reader;
            std::io::copy(&mut buf_reader, &mut writer)?;
            writer.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Agent, AgentField, Config, SkillTarget, SkillsField};
    use std::path::Path;

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn select_targets_reports_missing_skill() {
        let mut available = HashMap::new();
        available.insert("present".to_string(), PathBuf::from("/tmp/present"));
        let sf = SkillsField::List(vec![
            SkillTarget::Name("present".to_string()),
            SkillTarget::Name("missing".to_string()),
        ]);

        let (targets, broken) = select_targets(&sf, &available).expect("select");
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, "present");
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].name, "missing");
        assert!(broken[0].reason.contains("skill not found"));
    }

    #[test]
    fn select_targets_prefers_explicit_path_override() {
        let root = temp_dir("kasetto-targets");
        let nested = root.join("skills-repo");
        let skill_dir = nested.join("custom-skill");
        fs::create_dir_all(&skill_dir).expect("create dirs");
        fs::write(skill_dir.join("SKILL.md"), "# Custom\n\nDesc\n").expect("write skill");

        let mut available = HashMap::new();
        available.insert(
            "custom-skill".to_string(),
            PathBuf::from("/tmp/wrong-location"),
        );
        let sf = SkillsField::List(vec![SkillTarget::Obj {
            name: "custom-skill".to_string(),
            path: Some(nested.to_string_lossy().to_string()),
        }]);

        let (targets, broken) = select_targets(&sf, &available).expect("select");
        assert!(broken.is_empty());
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].0, "custom-skill");
        assert_eq!(targets[0].1, skill_dir);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn agent_paths_cover_supported_presets() {
        let home = Path::new("/tmp/kasetto-home");

        assert_eq!(Agent::Codex.global_path(home), home.join(".codex/skills"));
        assert_eq!(
            Agent::Amp.global_path(home),
            home.join(".config/agents/skills")
        );
        assert_eq!(
            Agent::Antigravity.global_path(home),
            home.join(".gemini/antigravity/skills")
        );
        assert_eq!(
            Agent::OpenClaw.global_path(home),
            home.join(".openclaw/skills")
        );
        assert_eq!(
            Agent::Windsurf.global_path(home),
            home.join(".codeium/windsurf/skills")
        );
        assert_eq!(
            Agent::TraeCn.global_path(home),
            home.join(".trae-cn/skills")
        );
    }

    #[test]
    fn config_agent_parses_hyphenated_names() {
        let hyphenated: Config =
            serde_yaml::from_str("agent: command-code\nskills: []\n").expect("parse config");
        assert_eq!(hyphenated.agent, Some(AgentField::One(Agent::CommandCode)));

        let claude_code: Config =
            serde_yaml::from_str("agent: claude-code\nskills: []\n").expect("parse config");
        assert_eq!(claude_code.agent, Some(AgentField::One(Agent::ClaudeCode)));
    }

    #[test]
    fn config_agent_parses_multi_agent_list() {
        let multi: Config =
            serde_yaml::from_str("agent:\n  - claude-code\n  - cursor\nskills: []\n")
                .expect("parse config");
        assert_eq!(
            multi.agent,
            Some(AgentField::Many(vec![Agent::ClaudeCode, Agent::Cursor]))
        );
        assert_eq!(multi.agents(), vec![Agent::ClaudeCode, Agent::Cursor]);
    }

    #[test]
    fn settings_file_load_creates_empty_for_missing_file() {
        let dir = temp_dir("kasetto-sf-missing");
        let path = dir.join("nonexistent.json");
        let sf = SettingsFile::load(&path).expect("load");
        assert_eq!(sf.data, serde_json::json!({}));
    }

    #[test]
    fn settings_file_load_parses_existing_json() {
        let dir = temp_dir("kasetto-sf-parse");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"mcpServers":{}}"#).unwrap();

        let sf = SettingsFile::load(&path).expect("load");
        assert!(sf.data["mcpServers"].is_object());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_file_save_creates_parent_dirs() {
        let dir = temp_dir("kasetto-sf-save");
        let nested = dir.join("deep").join("path").join("settings.json");

        let mut sf = SettingsFile::load(&nested).expect("load");
        sf.data["key"] = serde_json::json!("value");
        sf.save().expect("save");

        let text = fs::read_to_string(&nested).unwrap();
        let val: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(val["key"], "value");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_file_load_rejects_invalid_json() {
        let dir = temp_dir("kasetto-sf-invalid");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        fs::write(&path, "not valid json {{{").unwrap();

        let result = SettingsFile::load(&path);
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&dir);
    }
}
