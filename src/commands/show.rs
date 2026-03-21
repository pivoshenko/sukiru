use crate::error::Result;
use crate::fsops::load_state;
use crate::model::InstalledSkill;
use crate::profile::{format_updated_ago, list_color_enabled, read_skill_profile};

pub fn run(skill: &str, as_json: bool) -> Result<()> {
    let state = load_state()?;

    let mut items: Vec<InstalledSkill> = state
        .skills
        .iter()
        .filter(|(_, entry)| entry.skill == skill)
        .map(|(id, entry)| {
            let (name, fallback_description) =
                read_skill_profile(&entry.destination, &entry.skill);
            let description = if entry.description.trim().is_empty() {
                fallback_description
            } else {
                entry.description.clone()
            };
            let updated_ago = format_updated_ago(&entry.updated_at);
            InstalledSkill {
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
            }
        })
        .collect();

    if items.is_empty() {
        if as_json {
            println!("null");
        } else {
            let color = list_color_enabled();
            if color {
                eprintln!("\x1b[31merror:\x1b[0m skill not found: {}", skill);
            } else {
                eprintln!("error: skill not found: {}", skill);
            }
        }
        std::process::exit(1);
    }

    items.sort_by(|a, b| a.source.cmp(&b.source));

    if as_json {
        if items.len() == 1 {
            println!("{}", serde_json::to_string_pretty(&items[0])?);
        } else {
            println!("{}", serde_json::to_string_pretty(&items)?);
        }
        return Ok(());
    }

    let color = list_color_enabled();
    for item in &items {
        if color {
            println!(
                "\x1b[1;33m{}\x1b[0m  \x1b[90m({})\x1b[0m",
                item.name, item.skill
            );
            println!("  {}", item.description);
            println!(
                "  \x1b[1;35mSource:\x1b[0m          {}",
                item.source
            );
            println!(
                "  \x1b[1;35mSource revision:\x1b[0m {}",
                item.source_revision
            );
            println!(
                "  \x1b[1;35mDestination:\x1b[0m     {}",
                item.destination
            );
            println!(
                "  \x1b[1;35mHash:\x1b[0m            {}",
                item.hash
            );
            println!(
                "  \x1b[1;35mUpdated:\x1b[0m         {} \x1b[90m({})\x1b[0m",
                item.updated_ago, item.updated_at
            );
        } else {
            println!("{}  ({})", item.name, item.skill);
            println!("  {}", item.description);
            println!("  Source:          {}", item.source);
            println!("  Source revision: {}", item.source_revision);
            println!("  Destination:     {}", item.destination);
            println!("  Hash:            {}", item.hash);
            println!("  Updated:         {} ({})", item.updated_ago, item.updated_at);
        }
        if items.len() > 1 {
            println!();
        }
    }
    Ok(())
}
