//! Remote archive and tarball download (GitHub, GitLab, Bitbucket, Gitea).

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::error::{err, Result};
use crate::fsops::http_client;

use super::auth::{auth_env_inline_help, http_fetch_auth_hint, UrlRequestAuth};
use super::parse::RepoUrl;

pub(super) fn remote_repo_archive(parsed: &RepoUrl, branch: &str) -> (String, UrlRequestAuth) {
    match parsed {
        RepoUrl::GitHub { host, owner, repo } => (
            github_archive_tarball_url(host, owner, repo, branch),
            UrlRequestAuth::for_github_archive(),
        ),
        RepoUrl::GitLab { host, project_path } => (
            gitlab_project_archive_url(host, project_path, branch),
            UrlRequestAuth::for_gitlab_archive(),
        ),
        RepoUrl::Bitbucket {
            workspace,
            repo_slug,
        } => (
            bitbucket_archive_tarball_url(workspace, repo_slug, branch),
            UrlRequestAuth::for_bitbucket_archive(),
        ),
        RepoUrl::Gitea {
            host,
            owner,
            repo,
        } => (
            gitea_archive_tarball_url(host, owner, repo, branch),
            UrlRequestAuth::for_gitea_archive(),
        ),
    }
}

fn github_archive_tarball_url(host: &str, owner: &str, repo: &str, branch: &str) -> String {
    format!("https://{host}/{owner}/{repo}/archive/refs/heads/{branch}.tar.gz")
}

/// GitLab API path encoding: `/` → `%2F`.
fn encode_gitlab_path(path: &str) -> String {
    path.replace('/', "%2F")
}

fn gitlab_project_archive_url(host: &str, project_path: &str, branch: &str) -> String {
    let encoded = encode_gitlab_path(project_path);
    format!("https://{host}/api/v4/projects/{encoded}/repository/archive.tar.gz?sha={branch}")
}

fn gitlab_repository_file_raw_url(
    host: &str,
    project: &str,
    file_path: &str,
    git_ref: &str,
) -> String {
    format!(
        "https://{host}/api/v4/projects/{}/repository/files/{}/raw?ref={git_ref}",
        encode_gitlab_path(project),
        encode_gitlab_path(file_path),
    )
}

/// Bitbucket Cloud source archive (see Atlassian KB: `.../get/{branch}.tar.gz`).
fn bitbucket_archive_tarball_url(workspace: &str, repo_slug: &str, branch: &str) -> String {
    format!("https://bitbucket.org/{workspace}/{repo_slug}/get/{branch}.tar.gz")
}

fn gitea_archive_tarball_url(host: &str, owner: &str, repo: &str, branch: &str) -> String {
    format!("https://{host}/{owner}/{repo}/archive/{branch}.tar.gz")
}

pub(crate) fn rewrite_gitlab_raw_url(url: &str) -> Option<String> {
    let cleaned = url.split('?').next().unwrap_or(url);
    let without_scheme = cleaned
        .strip_prefix("https://")
        .or_else(|| cleaned.strip_prefix("http://"))?;

    let (host, rest) = without_scheme.split_once('/')?;
    if host == "github.com" {
        return None;
    }

    for marker in ["/-/raw/", "/-/blob/"] {
        if let Some(idx) = rest.find(marker) {
            let project = &rest[..idx];
            let after = &rest[idx + marker.len()..];
            let (ref_name, file_path) = after.split_once('/')?;
            return Some(gitlab_repository_file_raw_url(
                host, project, file_path, ref_name,
            ));
        }
    }

    let parts: Vec<&str> = rest.split('/').collect();
    if parts.len() < 3 {
        return None;
    }
    let file_start = parts.iter().position(|p| p.contains('.'))?;
    if file_start < 2 {
        return None;
    }
    let project = parts[..file_start].join("/");
    let file_path = parts[file_start..].join("/");
    Some(gitlab_repository_file_raw_url(host, &project, &file_path, "main"))
}

pub(super) fn download_extract(
    fetch_url: &str,
    auth: &UrlRequestAuth,
    dst: &Path,
    user_source: &str,
) -> Result<()> {
    if dst.exists() {
        fs::remove_dir_all(dst)?;
    }
    fs::create_dir_all(dst)?;
    let request = http_client()?.get(fetch_url);
    let request = auth.apply(request);
    let response = request
        .send()
        .map_err(|e| err(format!("failed to reach {user_source}: {e}")))?;
    let status = response.status();
    let status_u16 = status.as_u16();
    let body = response
        .bytes()
        .map_err(|e| err(format!("failed to read archive for {user_source}: {e}")))?;
    if !status.is_success() {
        return Err(err(format!(
            "failed to download {user_source} (HTTP {status_u16}){}",
            http_fetch_auth_hint(user_source, status_u16)
        )));
    }
    if body.starts_with(b"<") || body.starts_with(b"<!") {
        return Err(err(format!(
            "failed to download {user_source}: server returned HTML instead of a .tar.gz — {}",
            auth_env_inline_help(user_source)
        )));
    }
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
        if rel.components().any(|c| c == Component::ParentDir) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_archive_urls_use_host_from_link() {
        let u = github_archive_tarball_url("github.com", "o", "r", "main");
        assert_eq!(
            u,
            "https://github.com/o/r/archive/refs/heads/main.tar.gz"
        );
        let u = github_archive_tarball_url("ghe.internal", "o", "r", "dev");
        assert_eq!(
            u,
            "https://ghe.internal/o/r/archive/refs/heads/dev.tar.gz"
        );
    }

    #[test]
    fn bitbucket_archive_urls_use_bitbucket_org() {
        let u = bitbucket_archive_tarball_url("ws", "myrepo", "main");
        assert_eq!(u, "https://bitbucket.org/ws/myrepo/get/main.tar.gz");
    }

    #[test]
    fn gitea_archive_urls_match_web_download() {
        let u = gitea_archive_tarball_url("codeberg.org", "a", "b", "main");
        assert_eq!(u, "https://codeberg.org/a/b/archive/main.tar.gz");
    }

    #[test]
    fn rewrite_github_returns_none() {
        assert!(rewrite_gitlab_raw_url("https://github.com/owner/repo/file.yaml").is_none());
    }
}
