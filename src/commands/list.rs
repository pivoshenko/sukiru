use std::io::IsTerminal;

use crate::banner::print_banner;
use crate::colors::{ACCENT, RESET, SECONDARY, WARNING_EMPHASIS};
use crate::db::load_state;
use crate::error::Result;
use crate::list::{browse as browse_list, mcp_asset_entries, BrowseInput};
use crate::mcps::list_installed_mcps;
use crate::model::InstalledSkill;
use crate::profile::{format_updated_ago, list_color_enabled, read_skill_profile};

pub(crate) fn run(as_json: bool) -> Result<()> {
    let state = load_state()?;
    let managed_mcps = list_installed_mcps()?;

    let mut skills = Vec::new();
    for (id, entry) in &state.skills {
        let (name, fallback_description) = read_skill_profile(&entry.destination, &entry.skill);
        let description = if entry.description.trim().is_empty() {
            fallback_description
        } else {
            entry.description.clone()
        };
        let updated_ago = format_updated_ago(&entry.updated_at);
        skills.push(InstalledSkill {
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
    skills.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    if as_json {
        let output = serde_json::json!({
            "skills": skills,
            "mcps": managed_mcps,
        });
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    let has_anything = !skills.is_empty() || !managed_mcps.is_empty();

    if !has_anything {
        print_banner();
        println!("Nothing installed.");
        return Ok(());
    }

    if std::io::stdout().is_terminal() && std::env::var_os("NO_TUI").is_none() {
        let mcps = mcp_asset_entries(&managed_mcps);
        browse_list(&BrowseInput { skills, mcps })?;
        return Ok(());
    }

    print_banner();
    let color = list_color_enabled();

    if !skills.is_empty() {
        print_section_header("Skills", skills.len(), color);
        println!();
        for item in &skills {
            if color {
                println!(
                    "  {}{}{}  {}updated {} ({}){}",
                    WARNING_EMPHASIS, item.name, RESET, SECONDARY, item.updated_ago, item.updated_at, RESET
                );
            } else {
                println!(
                    "  {}  updated {} ({})",
                    item.name, item.updated_ago, item.updated_at
                );
            }
            println!("    {}", item.description);
            println!("    source: {}", item.source);
            println!();
        }
    }

    if !managed_mcps.is_empty() {
        print_section_header("MCP Servers", managed_mcps.len(), color);
        for name in &managed_mcps {
            println!("  {}", name);
        }
        println!();
    }

    Ok(())
}

fn print_section_header(title: &str, count: usize, color: bool) {
    if color {
        println!("{}{}: {}{}", ACCENT, title, count, RESET);
    } else {
        println!("{}: {}", title, count);
    }
}
