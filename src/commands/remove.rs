use std::fs;

use crate::error::Result;
use crate::fsops::{load_state, save_state};
use crate::profile::list_color_enabled;

#[derive(serde::Serialize)]
struct RemovedEntry {
    skill: String,
    source: String,
    destination: String,
    status: String,
}

#[derive(serde::Serialize)]
struct RemoveOutput {
    removed: Vec<RemovedEntry>,
    not_found: Vec<String>,
}

pub fn run(skills: &[String], dry_run: bool, as_json: bool) -> Result<()> {
    let mut state = load_state()?;
    let mut removed: Vec<RemovedEntry> = Vec::new();
    let mut not_found: Vec<String> = Vec::new();

    for skill_name in skills {
        let matching_keys: Vec<String> = state
            .skills
            .iter()
            .filter(|(_, entry)| entry.skill == *skill_name)
            .map(|(k, _)| k.clone())
            .collect();

        if matching_keys.is_empty() {
            not_found.push(skill_name.clone());
            continue;
        }

        for key in &matching_keys {
            if let Some(entry) = state.skills.get(key) {
                removed.push(RemovedEntry {
                    skill: entry.skill.clone(),
                    source: entry.source.clone(),
                    destination: entry.destination.clone(),
                    status: if dry_run {
                        "would_remove".into()
                    } else {
                        "removed".into()
                    },
                });
                if !dry_run {
                    let _ = fs::remove_dir_all(&entry.destination);
                }
            }
            if !dry_run {
                state.skills.remove(key);
            }
        }
    }

    if !dry_run && !removed.is_empty() {
        save_state(&state)?;
    }

    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&RemoveOutput { removed, not_found })?
        );
        return Ok(());
    }

    let color = list_color_enabled();
    for entry in &removed {
        if dry_run {
            if color {
                println!(
                    "\x1b[33mwould remove\x1b[0m \x1b[1;35m{}\x1b[0m \x1b[90m{}\x1b[0m",
                    entry.skill, entry.source
                );
            } else {
                println!("would remove {} {}", entry.skill, entry.source);
            }
        } else if color {
            println!(
                "\x1b[32mremoved\x1b[0m \x1b[1;35m{}\x1b[0m \x1b[90m{}\x1b[0m",
                entry.skill, entry.source
            );
        } else {
            println!("removed {} {}", entry.skill, entry.source);
        }
    }
    for name in &not_found {
        if color {
            eprintln!("\x1b[31merror:\x1b[0m skill not found: {}", name);
        } else {
            eprintln!("error: skill not found: {}", name);
        }
    }

    if !not_found.is_empty() {
        std::process::exit(1);
    }
    Ok(())
}
