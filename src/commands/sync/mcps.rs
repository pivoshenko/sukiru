use std::collections::HashSet;
use std::fs;

use crate::db::{
    get_tracked_asset, list_tracked_asset_ids, remove_tracked_asset, save_tracked_asset,
};
use crate::error::Result;
use crate::fsops::{hash_file, now_unix, resolve_mcp_settings_targets};
use crate::mcps::{merge_mcp_config, remove_mcp_server, servers_present_in_settings};
use crate::model::{Action, Summary};
use crate::source::{discover_mcps, materialize_source};
use crate::ui::with_spinner;

use super::{file_name_str, sync_label, SyncContext};
pub(super) fn sync_mcps(ctx: &SyncContext, summary: &mut Summary, actions: &mut Vec<Action>) -> Result<()> {
    let mut desired_mcp_ids = HashSet::new();
    let mcp_settings_list = resolve_mcp_settings_targets(ctx.cfg)?;
    if mcp_settings_list.is_empty() {
        return Ok(());
    }

    for (i, src) in ctx.cfg.mcps.iter().enumerate() {
        let stage = std::env::temp_dir().join(format!("kasetto-mcp-{}-{}", now_unix(), i));
        let materialized = materialize_source(&src.as_source_spec(), ctx.cfg_dir, &stage)?;
        let root = materialized
            .cleanup_dir
            .as_deref()
            .unwrap_or_else(|| std::path::Path::new(&src.source));
        let mcps = discover_mcps(root)?;
        for mcp_path in &mcps {
            let file_name = file_name_str(mcp_path);
            let hash = hash_file(mcp_path)?;

            // Parse server names from this MCP file
            let mcp_text = fs::read_to_string(mcp_path)?;
            let mcp_val: serde_json::Value = serde_json::from_str(&mcp_text)?;
            let server_names: Vec<String> = mcp_val
                .get("mcpServers")
                .and_then(|v| v.as_object())
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();

            let asset_id = format!("mcp::{}::{}", src.source, file_name);
            desired_mcp_ids.insert(asset_id.clone());

            let existing = get_tracked_asset("mcp", &asset_id)?;
            let is_unchanged = existing
                .as_ref()
                .map(|(h, _)| {
                    h == &hash
                        && mcp_settings_list
                            .iter()
                            .all(|target| servers_present_in_settings(&server_names, target))
                })
                .unwrap_or(false);

            let label = sync_label("MCP", &file_name, &src.source, ctx.plain);
            with_spinner(ctx.animate, ctx.plain, &label, || {
                if is_unchanged {
                    summary.unchanged += 1;
                    actions.push(Action {
                        source: Some(src.source.clone()),
                        skill: Some(format!("mcp:{file_name}")),
                        status: "unchanged".into(),
                        error: None,
                    });
                    return Ok(());
                }

                let status = if existing.is_some() {
                    if ctx.dry_run {
                        "would_update"
                    } else {
                        "updated"
                    }
                } else if ctx.dry_run {
                    "would_install"
                } else {
                    "installed"
                };

                if !ctx.dry_run {
                    for target in &mcp_settings_list {
                        merge_mcp_config(mcp_path, target)?;
                    }
                    let servers_csv = server_names.join(",");
                    save_tracked_asset(
                        "mcp",
                        &asset_id,
                        &file_name,
                        &hash,
                        &src.source,
                        &servers_csv,
                    )?;
                }

                if status.contains("install") {
                    summary.installed += 1;
                } else {
                    summary.updated += 1;
                }
                actions.push(Action {
                    source: Some(src.source.clone()),
                    skill: Some(format!("mcp:{file_name}")),
                    status: status.into(),
                    error: None,
                });
                Ok(())
            })?;
        }
        if let Some(d) = materialized.cleanup_dir {
            let _ = fs::remove_dir_all(d);
        }
    }

    // Remove MCP servers no longer in config
    for (old_id, old_servers_csv) in list_tracked_asset_ids("mcp")? {
        if desired_mcp_ids.contains(&old_id) {
            continue;
        }
        let mcp_name = old_id.rsplit("::").next().unwrap_or(&old_id).to_string();
        if ctx.dry_run {
            summary.removed += 1;
            actions.push(Action {
                source: None,
                skill: Some(format!("mcp:{mcp_name}")),
                status: "would_remove".into(),
                error: None,
            });
        } else {
            for target in &mcp_settings_list {
                for server_name in old_servers_csv.split(',').filter(|s| !s.is_empty()) {
                    let _ = remove_mcp_server(server_name, target);
                }
            }
            remove_tracked_asset(&old_id)?;
            summary.removed += 1;
            actions.push(Action {
                source: None,
                skill: Some(format!("mcp:{mcp_name}")),
                status: "removed".into(),
                error: None,
            });
        }
    }

    Ok(())
}
