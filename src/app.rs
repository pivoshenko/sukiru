use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::io::IsTerminal;
use std::path::Path;

use crate::banner::print_banner;
use crate::cli::{Cli, Commands};
use crate::error::Result;
use crate::fsops::{
    copy_dir, hash_dir, load_config_any, load_latest_failed_installs, load_state,
    manifest_db_path, materialize_source, now_iso, now_unix, resolve_destination, save_report,
    save_state, select_targets,
};
use crate::list_tui::browse as browse_list;
use crate::model::{Action, FailedInstall, InstalledSkill, Report, SkillEntry, Summary};
use crate::ui::{animations_enabled, status_chip, with_spinner};

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Commands::Sync {
        config: "skills.config.yaml".into(),
        dry_run: false,
        quiet: false,
        json: false,
        plain: false,
        verbose: false,
    }) {
        Commands::Sync {
            config,
            dry_run,
            quiet,
            json,
            plain,
            verbose,
        } => run_sync(&config, dry_run, quiet, json, plain, verbose),
        Commands::List { json } => run_list(json),
        Commands::Doctor { json } => run_doctor(json),
    }
}

fn run_sync(
    config_path: &str,
    dry_run: bool,
    quiet: bool,
    as_json: bool,
    plain: bool,
    verbose: bool,
) -> Result<()> {
    let animate = animations_enabled(quiet, as_json, plain);
    if !quiet && !as_json {
        if plain {
            println!("kasetto | カセット");
        } else {
            print_banner();
        }
    }

    let (cfg, cfg_dir, cfg_label) = load_config_any(config_path)?;
    let destination = resolve_destination(&cfg_dir, &cfg)?;
    if !dry_run {
        fs::create_dir_all(&destination)?;
    }

    let mut state = load_state()?;
    let mut desired_keys = HashSet::new();
    let mut summary = Summary::default();
    let mut actions = Vec::new();

    for (i, src) in cfg.skills.iter().enumerate() {
        let stage = std::env::temp_dir().join(format!("kasetto-{}-{}", now_unix(), i));
        match materialize_source(src, &cfg_dir, &stage) {
            Ok((root, rev, available)) => {
                let (targets, broken_skills) = select_targets(&src.skills, &available)?;
                for broken in broken_skills {
                    let broken_name = broken.name.clone();
                    let broken_reason = broken.reason.clone();
                    summary.broken += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(broken_name.clone()),
                        status: "broken".into(),
                        error: Some(broken_reason.clone()),
                    });
                    if !as_json && !quiet {
                        if plain {
                            eprintln!("x Failed {} {}", broken_name, src.source);
                        } else {
                            eprintln!(
                                "\x1b[31mx\x1b[0m Failed \x1b[1;35m{}\x1b[0m \x1b[90m{}\x1b[0m",
                                broken_name, src.source
                            );
                        }
                    }
                }
                for (skill_name, skill_path) in targets {
                    let (_, profile_description) =
                        read_skill_profile_from_dir(&skill_path, &skill_name);
                    let sync_step = if plain {
                        format!("Syncing {} {}", skill_name, src.source)
                    } else {
                        format!(
                            "Syncing \x1b[1;35m{}\x1b[0m \x1b[90m{}\x1b[0m",
                            skill_name, src.source
                        )
                    };
                    with_spinner(animate, plain, &sync_step, || {
                        let key = format!("{}::{}", src.source, skill_name);
                        desired_keys.insert(key.clone());
                        let hash = hash_dir(&skill_path)?;
                        let dest = destination.join(&skill_name);
                        let is_unchanged = state
                            .skills
                            .get(&key)
                            .map(|prev| prev.hash == hash && dest.exists())
                            .unwrap_or(false);
                        if is_unchanged {
                            if !dry_run {
                                if let Some(entry) = state.skills.get_mut(&key) {
                                    entry.description = profile_description.clone();
                                }
                            }
                            summary.unchanged += 1;
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(skill_name.clone()),
                                status: "unchanged".into(),
                                error: None,
                            });
                            return Ok(());
                        }

                        if dry_run {
                            let status = if state.skills.contains_key(&key) {
                                "would_update"
                            } else {
                                "would_install"
                            };
                            if status == "would_install" {
                                summary.installed += 1
                            } else {
                                summary.updated += 1
                            }
                            actions.push(Action {
                                source: Some(src.source.clone()),
                                skill: Some(skill_name.clone()),
                                status: status.into(),
                                error: None,
                            });
                            return Ok(());
                        }

                        copy_dir(&skill_path, &dest)?;
                        let status = if state.skills.contains_key(&key) {
                            summary.updated += 1;
                            "updated"
                        } else {
                            summary.installed += 1;
                            "installed"
                        };
                        state.skills.insert(
                            key,
                            SkillEntry {
                                destination: dest.to_string_lossy().to_string(),
                                hash,
                                skill: skill_name.clone(),
                                description: profile_description.clone(),
                                source: src.source.clone(),
                                source_revision: rev.clone(),
                                updated_at: now_iso(),
                            },
                        );
                        actions.push(Action {
                            source: Some(src.source.clone()),
                            skill: Some(skill_name.clone()),
                            status: status.into(),
                            error: None,
                        });
                        Ok(())
                    })?;
                }
                let _ = fs::remove_dir_all(root);
            }
            Err(e) => {
                summary.failed += 1;
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: None,
                    status: "source_error".into(),
                    error: Some(e.to_string()),
                });
            }
        }
    }

    let existing_keys: Vec<String> = state.skills.keys().cloned().collect();
    for k in existing_keys {
        if desired_keys.contains(&k) {
            continue;
        }
        if let Some(entry) = state.skills.get(&k).cloned() {
            if dry_run {
                summary.removed += 1;
                actions.push(Action {
                    source: Some(entry.source),
                    skill: Some(entry.skill),
                    status: "would_remove".into(),
                    error: None,
                });
            } else {
                let _ = fs::remove_dir_all(&entry.destination);
                state.skills.remove(&k);
                summary.removed += 1;
                actions.push(Action {
                    source: Some(entry.source),
                    skill: Some(entry.skill),
                    status: "removed".into(),
                    error: None,
                });
            }
        }
    }

    if !dry_run {
        state.last_run = Some(now_iso());
        save_state(&state)?;
    }

    let report = Report {
        run_id: format!("{}", now_unix()),
        config: cfg_label,
        destination: destination.to_string_lossy().to_string(),
        dry_run,
        summary,
        actions,
    };
    let _manifest_path = save_report(&report)?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else if !quiet {
        if plain {
            println!();
            println!("  Installed: {}", report.summary.installed);
            println!("  Updated:   {}", report.summary.updated);
            println!("  Removed:   {}", report.summary.removed);
            println!("  Unchanged: {}", report.summary.unchanged);
            println!("  Broken:    {}", report.summary.broken);
            println!("  Failed:    {}", report.summary.failed);
        } else {
            println!();
            println!(
                "  \x1b[32mInstalled\x1b[0m: {}   \x1b[36mUpdated\x1b[0m: {}   \x1b[33mRemoved\x1b[0m: {}",
                report.summary.installed, report.summary.updated, report.summary.removed
            );
            println!(
                "  \x1b[90mUnchanged\x1b[0m: {}   \x1b[35mBroken\x1b[0m: {}   \x1b[31mFailed\x1b[0m: {}",
                report.summary.unchanged, report.summary.broken, report.summary.failed
            );
        }

        if verbose {
            println!("\nActions:");
            for a in &report.actions {
                let status = status_chip(&a.status, plain);
                let src = a.source.as_deref().unwrap_or("-");
                let skill = a.skill.as_deref().unwrap_or("-");
                if let Some(err) = &a.error {
                    println!("  {} {} :: {} -> {}", status, src, skill, err);
                } else {
                    println!("  {} {} :: {}", status, src, skill);
                }
            }
        }
    }

    if report.summary.failed > 0 {
        std::process::exit(1);
    }
    Ok(())
}

fn run_list(as_json: bool) -> Result<()> {
    let state = load_state()?;
    if state.skills.is_empty() {
        if as_json {
            println!("[]");
            return Ok(());
        }
        println!("No installed skills.");
        return Ok(());
    }

    let mut items = Vec::new();
    for (id, entry) in &state.skills {
        let (name, fallback_description) = read_skill_profile(&entry.destination, &entry.skill);
        let description = if entry.description.trim().is_empty() {
            fallback_description
        } else {
            entry.description.clone()
        };
        let updated_ago = format_updated_ago(&entry.updated_at);
        items.push(InstalledSkill {
            id: id.clone(),
            name,
            description,
            source: entry.source.clone(),
            skill: entry.skill.clone(),
            destination: entry.destination.clone(),
            hash: entry.hash.clone(),
            source_revision: entry.source_revision.clone(),
            updated_at: entry.updated_at.clone(),
            updated_ago,
        });
    }

    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    if as_json {
        println!("{}", serde_json::to_string_pretty(&items)?);
        return Ok(());
    }

    if std::io::stdout().is_terminal() && std::env::var_os("NO_TUI").is_none() {
        browse_list(&items)?;
    } else {
        print_list_text(&items);
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct DoctorOutput {
    version: String,
    manifest_db: String,
    installation_path: String,
    last_sync: Option<String>,
    failed_skills: Vec<FailedInstall>,
}

fn run_doctor(as_json: bool) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let manifest_path = manifest_db_path()?;
    let state = load_state()?;
    let mut install_paths: Vec<String> = state
        .skills
        .values()
        .map(|entry| {
            let p = Path::new(&entry.destination);
            p.parent().unwrap_or(p).to_string_lossy().to_string()
        })
        .collect();
    install_paths.sort();
    install_paths.dedup();
    let installation_path = if install_paths.is_empty() {
        "none".to_string()
    } else if install_paths.len() == 1 {
        install_paths[0].clone()
    } else {
        install_paths.join(", ")
    };

    let failed_skills = load_latest_failed_installs()?;
    let last_sync = state.last_run.clone();
    let output = DoctorOutput {
        version,
        manifest_db: manifest_path.to_string_lossy().to_string(),
        installation_path,
        last_sync,
        failed_skills,
    };

    if as_json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    let color = list_color_enabled();
    let last_sync_text = match &output.last_sync {
        Some(ts) => format!("{} ({})", format_updated_ago(ts), ts),
        None => "none".to_string(),
    };

    if color {
        println!("\x1b[1;35mVersion:\x1b[0m {}", output.version);
        println!("\x1b[1;35mManifest DB:\x1b[0m {}", output.manifest_db);
        println!(
            "\x1b[1;35mInstallation Path:\x1b[0m {}",
            output.installation_path
        );
        println!("\x1b[1;35mLast Sync:\x1b[0m {}", last_sync_text);
        println!("\x1b[1;35mFailed Skills:\x1b[0m");
        if output.failed_skills.is_empty() {
            println!("none");
        } else {
            for failed in &output.failed_skills {
                println!(
                    "\x1b[1;33m{}\x1b[0m {} \x1b[90m{}\x1b[0m",
                    failed.skill, failed.reason, failed.source
                );
            }
        }
    } else {
        println!("Version: {}", output.version);
        println!("Manifest DB: {}", output.manifest_db);
        println!("Installation Path: {}", output.installation_path);
        println!("Last Sync: {}", last_sync_text);
        println!("Failed Skills:");
        if output.failed_skills.is_empty() {
            println!("none");
        } else {
            for failed in &output.failed_skills {
                println!("{} {} {}", failed.skill, failed.reason, failed.source);
            }
        }
    }

    Ok(())
}

fn print_list_text(items: &[InstalledSkill]) {
    let color = list_color_enabled();
    println!("Installed skills: {}", items.len());
    println!();
    for item in items {
        if color {
            println!(
                "\x1b[1;33m{}\x1b[0m  \x1b[90mupdated {} ({})\x1b[0m",
                item.name, item.updated_ago, item.updated_at
            );
        } else {
            println!(
                "{}  updated {} ({})",
                item.name, item.updated_ago, item.updated_at
            );
        }
        println!("  {}", item.description);
        println!("  source: {}", item.source);
        println!("  path: {}", item.destination);
        println!();
    }
}

fn read_skill_profile(destination: &str, fallback_name: &str) -> (String, String) {
    read_skill_profile_from_dir(Path::new(destination), fallback_name)
}

fn read_skill_profile_from_dir(skill_dir: &Path, fallback_name: &str) -> (String, String) {
    let skill_md = skill_dir.join("SKILL.md");
    let body = match fs::read_to_string(skill_md) {
        Ok(v) => v,
        Err(_) => return (fallback_name.to_string(), "No description.".to_string()),
    };

    let lines: Vec<&str> = body.lines().collect();
    let mut content_start = 0usize;
    let mut front_name: Option<String> = None;
    let mut title: Option<String> = None;
    let mut description: Option<String> = None;

    if lines.first().map(|line| line.trim()) == Some("---") {
        for (idx, line) in lines.iter().enumerate().skip(1) {
            let trimmed = line.trim();
            if trimmed == "---" {
                content_start = idx + 1;
                break;
            }
            if front_name.is_none() {
                if let Some(raw) = trimmed.strip_prefix("name:") {
                    let value = raw.trim();
                    if !value.is_empty() {
                        front_name = Some(value.to_string());
                    }
                }
            }
            if description.is_none() {
                if let Some(raw) = trimmed.strip_prefix("description:") {
                    let value = raw.trim();
                    if !value.is_empty() {
                        description = Some(value.to_string());
                    }
                }
            }
        }
    }

    let mut in_code = false;
    for line in lines.iter().skip(content_start) {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code || trimmed.is_empty() {
            continue;
        }
        if title.is_none() && trimmed.starts_with('#') {
            title = Some(trimmed.trim_start_matches('#').trim().to_string());
            continue;
        }
        if trimmed.starts_with('#') {
            continue;
        }
        if description.is_none() {
            let candidate = trimmed
                .trim_start_matches('-')
                .trim_start_matches('*')
                .trim();
            if !candidate.is_empty() {
                description = Some(candidate.to_string());
            }
        }
        if title.is_some() && description.is_some() {
            break;
        }
    }

    (
        title
            .or(front_name)
            .unwrap_or_else(|| fallback_name.to_string()),
        description.unwrap_or_else(|| "No description.".to_string()),
    )
}

fn format_updated_ago(updated_at: &str) -> String {
    let ts = match updated_at.parse::<u64>() {
        Ok(v) => v,
        Err(_) => return "unknown".to_string(),
    };
    let now = now_unix();
    if ts > now {
        let d = ts - now;
        return format!("in {}s", d);
    }
    let d = now - ts;
    if d < 60 {
        format!("{}s ago", d)
    } else if d < 3600 {
        format!("{}m ago", d / 60)
    } else if d < 86_400 {
        format!("{}h ago", d / 3600)
    } else {
        format!("{}d ago", d / 86_400)
    }
}

fn list_color_enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}
