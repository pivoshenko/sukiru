//! Merge pack `mcpServers` into JSON-based agent settings.

use std::path::Path;

use crate::error::{err, Result};
use crate::fsops::SettingsFile;

use super::pack::read_source_mcp_servers;

pub(super) fn merge_mcp_servers_object(source_path: &Path, target_path: &Path) -> Result<()> {
    let src_map = read_source_mcp_servers(source_path)?;
    let mut sf = SettingsFile::load(target_path)?;
    let target_obj = sf
        .data
        .as_object_mut()
        .ok_or_else(|| err("settings file is not a JSON object"))?;
    let target_servers = target_obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    if let Some(dst_map) = target_servers.as_object_mut() {
        for (key, value) in src_map {
            if !dst_map.contains_key(&key) {
                dst_map.insert(key, value);
            }
        }
    }
    sf.save()?;
    Ok(())
}

fn normalize_vscode_server(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        if !obj.contains_key("type") {
            if obj.contains_key("command") {
                obj.insert("type".into(), serde_json::json!("stdio"));
            } else if obj.contains_key("url") {
                obj.insert("type".into(), serde_json::json!("http"));
            }
        }
    }
    value
}

pub(super) fn merge_vscode_servers_object(source_path: &Path, target_path: &Path) -> Result<()> {
    let src_map = read_source_mcp_servers(source_path)?;
    let mut sf = SettingsFile::load(target_path)?;
    let target_obj = sf
        .data
        .as_object_mut()
        .ok_or_else(|| err("settings file is not a JSON object"))?;
    let servers = target_obj
        .entry("servers")
        .or_insert_with(|| serde_json::json!({}));

    if let Some(dst_map) = servers.as_object_mut() {
        for (key, value) in src_map {
            if !dst_map.contains_key(&key) {
                dst_map.insert(key, normalize_vscode_server(value));
            }
        }
    }
    sf.save()?;
    Ok(())
}

fn mcp_entry_to_opencode(name: &str, v: &serde_json::Value) -> Result<serde_json::Value> {
    let Some(obj) = v.as_object() else {
        return Err(err(format!(
            "MCP server {name} must be a JSON object for OpenCode merge"
        )));
    };

    if let Some(url) = obj
        .get("url")
        .and_then(|u| u.as_str())
        .or_else(|| obj.get("serverUrl").and_then(|u| u.as_str()))
    {
        let mut out = serde_json::Map::new();
        out.insert("type".into(), serde_json::json!("remote"));
        out.insert("url".into(), serde_json::json!(url));
        out.insert("enabled".into(), serde_json::json!(true));
        if let Some(h) = obj.get("headers").and_then(|x| x.as_object()) {
            out.insert("headers".into(), serde_json::Value::Object(h.clone()));
        }
        return Ok(serde_json::Value::Object(out));
    }

    let cmd = obj
        .get("command")
        .and_then(|c| c.as_str())
        .ok_or_else(|| err(format!("MCP server {name} needs `command` or `url` for OpenCode")))?;

    let mut cmd_arr = vec![serde_json::json!(cmd)];
    if let Some(args) = obj.get("args").and_then(|a| a.as_array()) {
        cmd_arr.extend(args.iter().cloned());
    }

    let mut out = serde_json::Map::new();
    out.insert("type".into(), serde_json::json!("local"));
    out.insert("command".into(), serde_json::Value::Array(cmd_arr));
    out.insert("enabled".into(), serde_json::json!(true));
    if let Some(env) = obj.get("env").and_then(|e| e.as_object()) {
        out.insert(
            "environment".into(),
            serde_json::Value::Object(env.clone()),
        );
    }
    Ok(serde_json::Value::Object(out))
}

pub(super) fn merge_opencode_mcp_object(source_path: &Path, target_path: &Path) -> Result<()> {
    let src_map = read_source_mcp_servers(source_path)?;
    let mut sf = SettingsFile::load(target_path)?;
    let target_obj = sf
        .data
        .as_object_mut()
        .ok_or_else(|| err("OpenCode config is not a JSON object"))?;
    let mcp = target_obj.entry("mcp").or_insert_with(|| serde_json::json!({}));

    if let Some(dst_map) = mcp.as_object_mut() {
        for (key, value) in src_map {
            if dst_map.contains_key(&key) {
                continue;
            }
            let converted = mcp_entry_to_opencode(&key, &value)?;
            dst_map.insert(key, converted);
        }
    }
    sf.save()?;
    Ok(())
}
