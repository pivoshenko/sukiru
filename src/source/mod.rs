//! Skill pack sources: local paths, remote archives, discovery.

mod auth;
mod hosts;
mod parse;
mod remote;

pub(crate) use auth::{auth_env_inline_help, auth_for_request_url, http_fetch_auth_hint};
pub(crate) use remote::rewrite_gitlab_raw_url;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::resolve_path;
use crate::model::SourceSpec;

pub(crate) fn materialize_source(
    src: &SourceSpec,
    cfg_dir: &Path,
    stage: &Path,
) -> Result<MaterializedSource> {
    if src.source.contains("://") {
        let parsed = parse::parse_repo_url(&src.source)?;
        let branch_label = src.branch.clone().unwrap_or_else(|| "main".into());
        let (url, auth) = remote::remote_repo_archive(&parsed, branch_label.as_str());

        remote::download_extract(&url, &auth, stage, &src.source).or_else(|e| {
            if src.branch.is_some() {
                Err(e)
            } else {
                let (url, auth) = remote::remote_repo_archive(&parsed, "master");
                remote::download_extract(&url, &auth, stage, &src.source).map_err(|e2| {
                    err(format!(
                        "{e2} (also tried branch `master` after `{}`)",
                        branch_label
                    ))
                })
            }
        })?;

        let available = discover(stage)?;
        Ok(MaterializedSource {
            source_revision: format!("branch:{branch_label}"),
            available,
            cleanup_dir: Some(stage.to_path_buf()),
        })
    } else {
        let root = resolve_path(cfg_dir, &src.source);
        let available = discover(&root)?;
        Ok(MaterializedSource {
            source_revision: "local".into(),
            available,
            cleanup_dir: None,
        })
    }
}

pub(crate) struct MaterializedSource {
    pub source_revision: String,
    pub available: HashMap<String, PathBuf>,
    pub cleanup_dir: Option<PathBuf>,
}

pub(crate) fn discover(root: &Path) -> Result<HashMap<String, PathBuf>> {
    let mut out = HashMap::new();
    discover_skills_in_subdir(root, &mut out)?;
    discover_skills_in_subdir(&root.join("skills"), &mut out)?;
    Ok(out)
}

fn discover_skills_in_subdir(base: &Path, out: &mut HashMap<String, PathBuf>) -> Result<()> {
    if !base.exists() {
        return Ok(());
    }
    for e in fs::read_dir(base)? {
        let e = e?;
        if !e.file_type()?.is_dir() {
            continue;
        }
        let d = e.path();
        if d.join("SKILL.md").is_file() {
            out.insert(e.file_name().to_string_lossy().to_string(), d);
        }
    }
    Ok(())
}

pub(crate) fn discover_mcps(root: &Path) -> Result<Vec<PathBuf>> {
    let mcp_dir = root.join("mcp");
    if !mcp_dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for e in fs::read_dir(mcp_dir)? {
        let e = e?;
        let path = e.path();
        if e.file_type()?.is_file() && path.extension().map(|ext| ext == "json").unwrap_or(false) {
            out.push(path);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{SkillsField, SourceSpec};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn local_materialize_does_not_set_cleanup_dir() {
        let root = temp_dir("kasetto-local-src");
        let skill_dir = root.join("demo-skill");
        fs::create_dir_all(&skill_dir).expect("create dirs");
        fs::write(skill_dir.join("SKILL.md"), "# Demo\n\nDesc\n").expect("write skill");

        let src = SourceSpec {
            source: root.to_string_lossy().to_string(),
            branch: None,
            skills: SkillsField::Wildcard("*".to_string()),
        };
        let stage = temp_dir("kasetto-stage");
        let materialized = materialize_source(&src, Path::new("/"), &stage).expect("materialize local");

        assert!(materialized.cleanup_dir.is_none());
        assert!(materialized.available.contains_key("demo-skill"));
        assert!(root.exists());

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&stage);
    }
}
