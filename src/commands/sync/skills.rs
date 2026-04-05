use std::collections::HashSet;
use std::fs;

use crate::colors::{ACCENT, ERROR, RESET, SECONDARY};
use crate::error::Result;
use crate::fsops::{copy_dir, hash_dir, now_iso, now_unix, select_targets};
use crate::model::{Action, SkillEntry, State, Summary};
use crate::profile::read_skill_profile_from_dir;
use crate::source::materialize_source;
use crate::ui::with_spinner;

use super::SyncContext;

pub(super) fn sync_skills(
    ctx: &SyncContext,
    state: &mut State,
    summary: &mut Summary,
    actions: &mut Vec<Action>,
) -> Result<()> {
    let mut desired_keys = HashSet::new();
    let destination = &ctx.destinations[0];

    for (i, src) in ctx.cfg.skills.iter().enumerate() {
        let stage = std::env::temp_dir().join(format!("kasetto-{}-{}", now_unix(), i));
        match materialize_source(src, ctx.cfg_dir, &stage) {
            Ok(materialized) => {
                let (targets, broken_skills) =
                    select_targets(&src.skills, &materialized.available)?;
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
                    if !ctx.as_json && !ctx.quiet {
                        if ctx.plain {
                            eprintln!("x Failed {} {}", broken_name, src.source);
                        } else {
                            eprintln!(
                                "{}x{} Failed {}{}{} {}{}{}",
                                ERROR,
                                RESET,
                                ACCENT,
                                broken_name,
                                RESET,
                                SECONDARY,
                                src.source,
                                RESET
                            );
                        }
                    }
                }
                for (skill_name, skill_path) in targets {
                    let (_, profile_description) =
                        read_skill_profile_from_dir(&skill_path, &skill_name);
                    let sync_step = if ctx.plain {
                        format!("Syncing {} {}", skill_name, src.source)
                    } else {
                        format!(
                            "Syncing {}{}{} {}{}{}",
                            ACCENT, skill_name, RESET, SECONDARY, src.source, RESET
                        )
                    };
                    with_spinner(ctx.animate, ctx.plain, &sync_step, || {
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
                            if !ctx.dry_run {
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

                        if ctx.dry_run {
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

                        for agent_dest in ctx.destinations {
                            copy_dir(&skill_path, &agent_dest.join(&skill_name))?;
                        }
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
                                source_revision: materialized.source_revision.clone(),
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
                if let Some(cleanup_dir) = materialized.cleanup_dir {
                    let _ = fs::remove_dir_all(cleanup_dir);
                }
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

    // Remove skills no longer in config
    let existing_keys: Vec<String> = state.skills.keys().cloned().collect();
    for k in existing_keys {
        if desired_keys.contains(&k) {
            continue;
        }
        if let Some(entry) = state.skills.get(&k).cloned() {
            if ctx.dry_run {
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

    Ok(())
}
