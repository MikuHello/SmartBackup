use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

fn yes() -> bool {
    true
}
fn seven() -> String {
    "7z".into()
}
fn full() -> String {
    "full".into()
}
fn metadata() -> String {
    "metadata".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VolumeIdentity {
    pub kind: String,
    pub primary_id: String,
    pub fs_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub alias: String,
    pub path: PathBuf,
    pub volume: VolumeIdentity,
    #[serde(default = "yes")]
    pub required: bool,
    #[serde(default)]
    pub follow_symlinks: bool,
    #[serde(default)]
    pub cross_filesystems: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    pub sources: Vec<Source>,
    pub output: PathBuf,
    pub output_volume: VolumeIdentity,
    #[serde(default = "seven")]
    pub format: String,
    #[serde(default = "full")]
    pub verification: String,
    #[serde(default = "metadata")]
    pub change_detection: String,
    #[serde(default)]
    pub filters: Filters,
    // Retain unknown fields for future version round-trips; validation rejects activation.
    #[serde(flatten)]
    pub extra: BTreeMap<String, toml::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filters {
    #[serde(default = "yes")]
    pub builtin: bool,
    #[serde(default)]
    pub follow_gitignore: bool,
    #[serde(default)]
    pub rules: Vec<String>,
    #[serde(default)]
    pub rule_files: Vec<PathBuf>,
}
impl Default for Filters {
    fn default() -> Self {
        Self {
            builtin: true,
            follow_gitignore: false,
            rules: vec![],
            rule_files: vec![],
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub path: PathBuf,
    pub sha256: String,
    pub version: String,
    pub provenance: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub schema_version: u32,
    pub engine: EngineConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub schema_version: u32,
    pub id: String,
    pub job_id: String,
    pub job_name: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub reason_code: String,
    pub artifact: Option<PathBuf>,
    pub sha256: Option<String>,
    pub verification_state: String,
    pub fingerprint: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    pub job_id: String,
    pub run_id: String,
    pub archive_id: String,
    pub job_name: String,
    pub application_version: String,
    pub engine_version: String,
    pub created_at: String,
    pub format: String,
    pub source_aliases: Vec<String>,
    pub files: u64,
    pub directories: u64,
    pub bytes: u64,
    pub verification: String,
    pub rules_hash: String,
}
