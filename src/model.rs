use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub destination: Option<String>,
    pub agent: Option<Agent>,
    pub skills: Vec<SourceSpec>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Codex,
    Claude,
    Cursor,
}

#[derive(Debug, Deserialize)]
pub struct SourceSpec {
    pub source: String,
    pub branch: Option<String>,
    pub skills: SkillsField,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SkillsField {
    Wildcard(String),
    List(Vec<SkillTarget>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum SkillTarget {
    Name(String),
    Obj { name: String, path: Option<String> },
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct SkillEntry {
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
pub struct State {
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
pub struct Summary {
    pub installed: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
    pub broken: usize,
    pub failed: usize,
}

#[derive(Debug, Serialize)]
pub struct Action {
    pub source: Option<String>,
    pub skill: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub run_id: String,
    pub config: String,
    pub destination: String,
    pub dry_run: bool,
    pub summary: Summary,
    pub actions: Vec<Action>,
}

#[derive(Debug, Serialize, Clone)]
pub struct InstalledSkill {
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
pub struct FailedInstall {
    pub skill: String,
    pub source: String,
    pub reason: String,
}
