use std::fs;

use crate::banner::print_banner;
use crate::colors::{ACCENT, ERROR, RESET, SUCCESS, WARNING};
use crate::db;
use crate::error::Result;
use crate::fsops::{dirs_home, dirs_kasetto_config};
use crate::mcps::remove_mcp_server;
use crate::model::all_mcp_settings_targets;

#[derive(serde::Serialize)]
struct CleanOutput {
    skills_removed: usize,
    mcps_removed: usize,
    dry_run: bool,
}

pub(crate) fn run(dry_run: bool, as_json: bool, quiet: bool) -> Result<()> {
    if !as_json && !quiet {
        print_banner();
    }

    let home = dirs_home()?;
    let kasetto_config = dirs_kasetto_config()?;
    let state = db::load_state()?;
    let mcp_assets = db::list_tracked_asset_ids("mcp")?;

    let skills_count = state.skills.len();
    let mcps_count = mcp_assets.len();

    if !dry_run {
        // Remove skill directories
        for entry in state.skills.values() {
            let _ = fs::remove_dir_all(&entry.destination);
        }

        // Remove MCP servers from every native agent config path Kasetto might have written to.
        let mcp_targets = all_mcp_settings_targets(&home, &kasetto_config);
        for (_id, servers_csv) in &mcp_assets {
            for server_name in servers_csv.split(',').filter(|s| !s.is_empty()) {
                for target in &mcp_targets {
                    if target.path.exists() {
                        let _ = remove_mcp_server(server_name, target);
                    }
                }
            }
        }

        db::clear_all()?;
    }

    let output = CleanOutput {
        skills_removed: skills_count,
        mcps_removed: mcps_count,
        dry_run,
    };

    if as_json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if !quiet {
        let (label_color, prefix) = if dry_run {
            (WARNING, "Would remove")
        } else {
            (ERROR, "Removed")
        };
        println!();
        println!(
            "  {label_color}{prefix}{RESET}: {}   {label_color}MCP configs{RESET}: {}",
            skills_count, mcps_count
        );

        if !dry_run {
            println!();
            println!("{SUCCESS}✓{RESET} Manifest reset.");
        } else {
            println!();
            println!("Run without {ACCENT}--dry-run{RESET} to apply.");
        }
    }

    Ok(())
}
