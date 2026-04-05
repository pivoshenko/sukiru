use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub destination: Option<String>,
    #[serde(default)]
    pub agent: Option<AgentField>,
    #[serde(default)]
    pub skills: Vec<SourceSpec>,
    #[serde(default)]
    pub mcps: Vec<McpSourceSpec>,
}

impl Config {
    pub(crate) fn agents(&self) -> Vec<Agent> {
        match &self.agent {
            Some(AgentField::One(a)) => vec![*a],
            Some(AgentField::Many(v)) => v.clone(),
            None => vec![],
        }
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum AgentField {
    One(Agent),
    Many(Vec<Agent>),
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Agent {
    #[serde(rename = "amp")]
    Amp,
    #[serde(rename = "kimi-cli")]
    KimiCli,
    #[serde(rename = "replit")]
    Replit,
    #[serde(rename = "universal")]
    Universal,
    #[serde(rename = "antigravity")]
    Antigravity,
    #[serde(rename = "augment")]
    Augment,
    #[serde(rename = "claude-code")]
    ClaudeCode,
    #[serde(rename = "openclaw")]
    OpenClaw,
    #[serde(rename = "cline")]
    Cline,
    #[serde(rename = "warp")]
    Warp,
    #[serde(rename = "codebuddy")]
    CodeBuddy,
    #[serde(rename = "codex")]
    Codex,
    #[serde(rename = "command-code")]
    CommandCode,
    #[serde(rename = "continue")]
    Continue,
    #[serde(rename = "cortex")]
    Cortex,
    #[serde(rename = "crush")]
    Crush,
    #[serde(rename = "cursor")]
    Cursor,
    #[serde(rename = "deepagents")]
    DeepAgents,
    #[serde(rename = "droid")]
    Droid,
    #[serde(rename = "gemini-cli")]
    GeminiCli,
    #[serde(rename = "github-copilot")]
    GithubCopilot,
    #[serde(rename = "goose")]
    Goose,
    #[serde(rename = "junie")]
    Junie,
    #[serde(rename = "iflow-cli")]
    IflowCli,
    #[serde(rename = "kilo")]
    Kilo,
    #[serde(rename = "kiro-cli")]
    KiroCli,
    #[serde(rename = "kode")]
    Kode,
    #[serde(rename = "mcpjam")]
    McpJam,
    #[serde(rename = "mistral-vibe")]
    MistralVibe,
    #[serde(rename = "mux")]
    Mux,
    #[serde(rename = "opencode")]
    OpenCode,
    #[serde(rename = "openhands")]
    OpenHands,
    #[serde(rename = "pi")]
    Pi,
    #[serde(rename = "qoder")]
    Qoder,
    #[serde(rename = "qwen-code")]
    QwenCode,
    #[serde(rename = "roo")]
    Roo,
    #[serde(rename = "trae")]
    Trae,
    #[serde(rename = "trae-cn")]
    TraeCn,
    #[serde(rename = "windsurf")]
    Windsurf,
    #[serde(rename = "zencoder")]
    Zencoder,
    #[serde(rename = "neovate")]
    Neovate,
    #[serde(rename = "pochi")]
    Pochi,
    #[serde(rename = "adal")]
    Adal,
}

/// How Kasetto merges pack `mcpServers` into an agent config file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum McpSettingsFormat {
    /// `{ "mcpServers": { ... } }` (Claude, Cursor, Gemini CLI, Roo, Cline, etc.).
    McpServers,
    /// VS Code / GitHub Copilot user `mcp.json`: `{ "servers": { ... } }`.
    VsCodeServers,
    /// OpenCode global `opencode.json`: `{ "mcp": { "name": { "type": "local"|"remote", ... } } }`.
    OpenCode,
    /// OpenAI Codex `~/.codex/config.toml` (`[mcp_servers.name]` tables).
    CodexToml,
}

/// Destination file and merge format for MCP sync / clean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct McpSettingsTarget {
    pub path: PathBuf,
    pub format: McpSettingsFormat,
}

/// Every preset value (for clean / enumerating native MCP paths).
pub(crate) const AGENT_PRESETS: &[Agent] = &[
    Agent::Amp,
    Agent::KimiCli,
    Agent::Replit,
    Agent::Universal,
    Agent::Antigravity,
    Agent::Augment,
    Agent::ClaudeCode,
    Agent::OpenClaw,
    Agent::Cline,
    Agent::Warp,
    Agent::CodeBuddy,
    Agent::Codex,
    Agent::CommandCode,
    Agent::Continue,
    Agent::Cortex,
    Agent::Crush,
    Agent::Cursor,
    Agent::DeepAgents,
    Agent::Droid,
    Agent::GeminiCli,
    Agent::GithubCopilot,
    Agent::Goose,
    Agent::Junie,
    Agent::IflowCli,
    Agent::Kilo,
    Agent::KiroCli,
    Agent::Kode,
    Agent::McpJam,
    Agent::MistralVibe,
    Agent::Mux,
    Agent::OpenCode,
    Agent::OpenHands,
    Agent::Pi,
    Agent::Qoder,
    Agent::QwenCode,
    Agent::Roo,
    Agent::Trae,
    Agent::TraeCn,
    Agent::Windsurf,
    Agent::Zencoder,
    Agent::Neovate,
    Agent::Pochi,
    Agent::Adal,
];

/// Deduped native MCP config files for every known agent (for `clean` manifest wipe).
pub(crate) fn all_mcp_settings_targets(home: &Path, kasetto_config: &Path) -> Vec<McpSettingsTarget> {
    let mut seen = std::collections::HashSet::<PathBuf>::new();
    let mut out = Vec::new();
    for &a in AGENT_PRESETS {
        let t = a.mcp_settings_target(home, kasetto_config);
        if seen.insert(t.path.clone()) {
            out.push(t);
        }
    }
    out.sort_by(|x, y| x.path.cmp(&y.path));
    out
}

/// VS Code / Copilot user-profile `mcp.json` (not Insiders).
pub(crate) fn vscode_user_mcp_json(home: &Path) -> PathBuf {
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Code/User/mcp.json")
    } else if cfg!(target_os = "windows") {
        let base = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(base).join("Code/User/mcp.json")
    } else {
        home.join(".config/Code/User/mcp.json")
    }
}

#[inline]
fn mcp_servers_target(home: &Path, relative_to_home: &str) -> McpSettingsTarget {
    McpSettingsTarget {
        path: home.join(relative_to_home),
        format: McpSettingsFormat::McpServers,
    }
}

impl Agent {
    pub(crate) fn global_path(self, home: &Path) -> PathBuf {
        match self {
            Agent::Amp | Agent::KimiCli | Agent::Replit | Agent::Universal => {
                home.join(".config/agents/skills")
            }
            Agent::Antigravity => home.join(".gemini/antigravity/skills"),
            Agent::Augment => home.join(".augment/skills"),
            Agent::ClaudeCode => home.join(".claude/skills"),
            Agent::OpenClaw => home.join(".openclaw/skills"),
            Agent::Cline | Agent::Warp => home.join(".agents/skills"),
            Agent::CodeBuddy => home.join(".codebuddy/skills"),
            Agent::Codex => home.join(".codex/skills"),
            Agent::CommandCode => home.join(".commandcode/skills"),
            Agent::Continue => home.join(".continue/skills"),
            Agent::Cortex => home.join(".snowflake/cortex/skills"),
            Agent::Crush => home.join(".config/crush/skills"),
            Agent::Cursor => home.join(".cursor/skills"),
            Agent::DeepAgents => home.join(".deepagents/agent/skills"),
            Agent::Droid => home.join(".factory/skills"),
            Agent::GeminiCli => home.join(".gemini/skills"),
            Agent::GithubCopilot => home.join(".copilot/skills"),
            Agent::Goose => home.join(".config/goose/skills"),
            Agent::Junie => home.join(".junie/skills"),
            Agent::IflowCli => home.join(".iflow/skills"),
            Agent::Kilo => home.join(".kilocode/skills"),
            Agent::KiroCli => home.join(".kiro/skills"),
            Agent::Kode => home.join(".kode/skills"),
            Agent::McpJam => home.join(".mcpjam/skills"),
            Agent::MistralVibe => home.join(".vibe/skills"),
            Agent::Mux => home.join(".mux/skills"),
            Agent::OpenCode => home.join(".config/opencode/skills"),
            Agent::OpenHands => home.join(".openhands/skills"),
            Agent::Pi => home.join(".pi/agent/skills"),
            Agent::Qoder => home.join(".qoder/skills"),
            Agent::QwenCode => home.join(".qwen/skills"),
            Agent::Roo => home.join(".roo/skills"),
            Agent::Trae => home.join(".trae/skills"),
            Agent::TraeCn => home.join(".trae-cn/skills"),
            Agent::Windsurf => home.join(".codeium/windsurf/skills"),
            Agent::Zencoder => home.join(".zencoder/skills"),
            Agent::Neovate => home.join(".neovate/skills"),
            Agent::Pochi => home.join(".pochi/skills"),
            Agent::Adal => home.join(".adal/skills"),
        }
    }

    /// Native MCP config location and merge format for this agent (see `AGENT_PRESETS`).
    ///
    /// Paths follow each tool’s usual config layout; packs use `mcpServers` in JSON and are
    /// merged with format-specific rules in `crate::mcps`.
    pub(crate) fn mcp_settings_target(self, home: &Path, _kasetto_config: &Path) -> McpSettingsTarget {
        match self {
            Agent::ClaudeCode => mcp_servers_target(home, ".claude.json"),
            Agent::Cursor => mcp_servers_target(home, ".cursor/mcp.json"),
            Agent::GithubCopilot => McpSettingsTarget {
                path: vscode_user_mcp_json(home),
                format: McpSettingsFormat::VsCodeServers,
            },
            Agent::GeminiCli => mcp_servers_target(home, ".gemini/settings.json"),
            Agent::Roo => mcp_servers_target(home, ".roo/mcp_settings.json"),
            Agent::Windsurf => mcp_servers_target(home, ".codeium/windsurf/mcp_config.json"),
            Agent::Cline => mcp_servers_target(home, ".cline/data/settings/cline_mcp_settings.json"),
            Agent::Continue => mcp_servers_target(home, ".continue/mcpServers/kasetto.json"),
            Agent::OpenCode => McpSettingsTarget {
                path: home.join(".config/opencode/opencode.json"),
                format: McpSettingsFormat::OpenCode,
            },
            Agent::Amp | Agent::KimiCli | Agent::Replit | Agent::Universal => {
                mcp_servers_target(home, ".config/agents/mcp.json")
            }
            Agent::Antigravity => mcp_servers_target(home, ".gemini/antigravity/mcp.json"),
            Agent::Augment => mcp_servers_target(home, ".augment/mcp.json"),
            Agent::OpenClaw => mcp_servers_target(home, ".openclaw/mcp.json"),
            Agent::Warp => mcp_servers_target(home, ".warp/mcp.json"),
            Agent::CodeBuddy => mcp_servers_target(home, ".codebuddy/mcp.json"),
            Agent::Codex => McpSettingsTarget {
                path: home.join(".codex/config.toml"),
                format: McpSettingsFormat::CodexToml,
            },
            Agent::CommandCode => mcp_servers_target(home, ".commandcode/mcp.json"),
            Agent::Cortex => mcp_servers_target(home, ".snowflake/cortex/mcp.json"),
            Agent::Crush => mcp_servers_target(home, ".config/crush/mcp.json"),
            Agent::DeepAgents => mcp_servers_target(home, ".deepagents/agent/mcp.json"),
            Agent::Droid => mcp_servers_target(home, ".factory/mcp.json"),
            Agent::Goose => mcp_servers_target(home, ".config/goose/mcp.json"),
            Agent::Junie => mcp_servers_target(home, ".junie/mcp.json"),
            Agent::IflowCli => mcp_servers_target(home, ".iflow/mcp.json"),
            Agent::Kilo => mcp_servers_target(home, ".kilocode/mcp.json"),
            Agent::KiroCli => mcp_servers_target(home, ".kiro/mcp.json"),
            Agent::Kode => mcp_servers_target(home, ".kode/mcp.json"),
            Agent::McpJam => mcp_servers_target(home, ".mcpjam/mcp.json"),
            Agent::MistralVibe => mcp_servers_target(home, ".vibe/mcp.json"),
            Agent::Mux => mcp_servers_target(home, ".mux/mcp.json"),
            Agent::OpenHands => mcp_servers_target(home, ".openhands/mcp.json"),
            Agent::Pi => mcp_servers_target(home, ".pi/agent/mcp.json"),
            Agent::Qoder => mcp_servers_target(home, ".qoder/mcp.json"),
            Agent::QwenCode => mcp_servers_target(home, ".qwen/mcp.json"),
            Agent::Trae => mcp_servers_target(home, ".trae/mcp.json"),
            Agent::TraeCn => mcp_servers_target(home, ".trae-cn/mcp.json"),
            Agent::Zencoder => mcp_servers_target(home, ".zencoder/mcp.json"),
            Agent::Neovate => mcp_servers_target(home, ".neovate/mcp.json"),
            Agent::Pochi => mcp_servers_target(home, ".pochi/mcp.json"),
            Agent::Adal => mcp_servers_target(home, ".adal/mcp.json"),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct SourceSpec {
    pub source: String,
    pub branch: Option<String>,
    pub skills: SkillsField,
}

#[derive(Debug, Deserialize)]
pub(crate) struct McpSourceSpec {
    pub source: String,
    pub branch: Option<String>,
}

impl McpSourceSpec {
    pub(crate) fn as_source_spec(&self) -> SourceSpec {
        SourceSpec {
            source: self.source.clone(),
            branch: self.branch.clone(),
            skills: SkillsField::Wildcard("*".to_string()),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum SkillsField {
    Wildcard(String),
    List(Vec<SkillTarget>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum SkillTarget {
    Name(String),
    Obj { name: String, path: Option<String> },
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub(crate) struct SkillEntry {
    pub destination: String,
    pub hash: String,
    pub skill: String,
    #[serde(default)]
    pub description: String,
    pub source: String,
    pub source_revision: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct State {
    pub version: u8,
    pub last_run: Option<String>,
    pub skills: BTreeMap<String, SkillEntry>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            last_run: None,
            skills: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Serialize, Default)]
pub(crate) struct Summary {
    pub installed: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub broken: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct Action {
    pub source: Option<String>,
    pub skill: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Report {
    pub run_id: String,
    pub config: String,
    pub destination: String,
    pub dry_run: bool,
    pub summary: Summary,
    pub actions: Vec<Action>,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct InstalledSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: String,
    pub skill: String,
    pub destination: String,
    pub hash: String,
    pub source_revision: String,
    pub updated_at: String,
    pub updated_ago: String,
}

#[derive(Debug, Serialize, Clone)]
pub(crate) struct SyncFailure {
    pub name: String,
    pub source: String,
    pub reason: String,
}
