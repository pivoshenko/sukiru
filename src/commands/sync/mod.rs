mod mcps;
mod skills;

use std::fs;
use std::path::{Path, PathBuf};

use crate::banner::print_banner;
use crate::colors::{ACCENT, ATTENTION, ERROR, INFO, RESET, SECONDARY, SUCCESS, WARNING};
use crate::db::{load_state, save_report, save_state};
use crate::error::Result;
use crate::fsops::{load_config_any, now_iso, now_unix, resolve_destinations};
use crate::model::{Config, Report, Summary};
use crate::ui::{animations_enabled, status_chip};

pub(super) struct SyncContext<'a> {
    pub(super) cfg: &'a Config,
    pub(super) cfg_dir: &'a Path,
    pub(super) destinations: &'a [PathBuf],
    pub(super) dry_run: bool,
    pub(super) animate: bool,
    pub(super) plain: bool,
    pub(super) as_json: bool,
    pub(super) quiet: bool,
}

pub(crate) fn run(
    config_path: &str,
    dry_run: bool,
    quiet: bool,
    as_json: bool,
    plain: bool,
    verbose: bool,
) -> Result<()> {
    run_inner(config_path, dry_run, quiet, as_json, plain, verbose, true)
}

fn run_inner(
    config_path: &str,
    dry_run: bool,
    quiet: bool,
    as_json: bool,
    plain: bool,
    verbose: bool,
    show_banner: bool,
) -> Result<()> {
    let animate = animations_enabled(quiet, as_json, plain);
    if show_banner && !quiet && !as_json && std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        if plain {
            println!("kasetto | カセット");
        } else {
            print_banner();
        }
    }

    let (cfg, cfg_dir, cfg_label) = load_config_any(config_path)?;
    let destinations = resolve_destinations(&cfg_dir, &cfg)?;
    let destination = destinations[0].clone();
    if !dry_run {
        for d in &destinations {
            fs::create_dir_all(d)?;
        }
    }

    let ctx = SyncContext {
        cfg: &cfg,
        cfg_dir: &cfg_dir,
        destinations: &destinations,
        dry_run,
        animate,
        plain,
        as_json,
        quiet,
    };

    let mut state = load_state()?;
    let mut summary = Summary::default();
    let mut actions = Vec::new();

    skills::sync_skills(&ctx, &mut state, &mut summary, &mut actions)?;
    mcps::sync_mcps(&ctx, &mut summary, &mut actions)?;

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
                "  {}Installed{}: {}   {}Updated{}: {}   {}Removed{}: {}",
                SUCCESS,
                RESET,
                report.summary.installed,
                INFO,
                RESET,
                report.summary.updated,
                WARNING,
                RESET,
                report.summary.removed
            );
            println!(
                "  {}Unchanged{}: {}   {}Broken{}: {}   {}Failed{}: {}",
                SECONDARY,
                RESET,
                report.summary.unchanged,
                ATTENTION,
                RESET,
                report.summary.broken,
                ERROR,
                RESET,
                report.summary.failed
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

pub(super) fn sync_label(kind: &str, name: &str, source: &str, plain: bool) -> String {
    if plain {
        format!("Syncing {kind} {name}")
    } else {
        format!(
            "Syncing {kind} {}{}{} {}{}{}",
            ACCENT, name, RESET, SECONDARY, source, RESET
        )
    }
}

pub(super) fn file_name_str(path: &std::path::Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
