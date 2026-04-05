use std::path::Path;

use crate::banner::print_banner;
use crate::colors::{ACCENT, RESET, SECONDARY};
use crate::db::{load_latest_failures, load_state, manifest_db_path};
use crate::error::Result;
use crate::fsops::dirs_home;
use crate::mcps::list_installed_mcps;
use crate::model::SyncFailure;
use crate::profile::{format_updated_ago, list_color_enabled};

#[derive(serde::Serialize)]
struct DoctorOutput {
    version: String,
    manifest_db: String,
    skills: Vec<String>,
    installation_path: String,
    last_sync: Option<String>,
    failures: Vec<SyncFailure>,
    mcps: Vec<String>,
}

pub(crate) fn run(as_json: bool) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION").to_string();
    let manifest_path = manifest_db_path()?;
    let _home = dirs_home()?;
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

    let mut skills: Vec<String> = state.skills.values().map(|e| e.skill.clone()).collect();
    skills.sort();

    let failures = load_latest_failures()?;
    let last_sync = state.last_run.clone();

    let managed_mcps = list_installed_mcps()?;

    let output = DoctorOutput {
        version,
        manifest_db: manifest_path.to_string_lossy().to_string(),
        skills,
        installation_path,
        last_sync,
        failures,
        mcps: managed_mcps,
    };

    if as_json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    let color = list_color_enabled();
    if std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        if color {
            print_banner();
        } else {
            println!("kasetto | カセット");
        }
        println!();
    }
    let last_sync_text = match &output.last_sync {
        Some(ts) => format!("{} ({})", format_updated_ago(ts), ts),
        None => "none".to_string(),
    };

    print_field("Version", &output.version, color);
    print_field("Manifest DB", &output.manifest_db, color);
    print_field("Installation Path", &output.installation_path, color);
    print_field("Last Sync", &last_sync_text, color);

    print_label("Failures", color);
    if output.failures.is_empty() {
        println!("  none");
    } else {
        for f in &output.failures {
            if color {
                println!(
                    "  {}{}{} {} {}{}{}",
                    ACCENT, f.name, RESET, f.reason, SECONDARY, f.source, RESET
                );
            } else {
                println!("  {} {} {}", f.name, f.reason, f.source);
            }
        }
    }

    print_label("Skills", color);
    print_name_list(&output.skills);

    print_label("MCP Servers", color);
    print_name_list(&output.mcps);

    Ok(())
}

fn print_field(label: &str, value: &str, color: bool) {
    if color {
        println!("{}{}: {}{}", ACCENT, label, RESET, value);
    } else {
        println!("{}: {}", label, value);
    }
}

fn print_label(label: &str, color: bool) {
    if color {
        println!("{}{}:{}", ACCENT, label, RESET);
    } else {
        println!("{}:", label);
    }
}

fn print_name_list(items: &[String]) {
    if items.is_empty() {
        println!("  none");
    } else {
        for item in items {
            println!("  {}", item);
        }
    }
}
