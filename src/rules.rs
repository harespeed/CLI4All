use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const DEV_COMMANDS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/commands.yaml");
const DEV_RISKS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/data/risks.yaml");
const UNIX_SHARE_COMMANDS_PATHS: &[&str] = &[
    "/usr/share/cli4all/data/commands.yaml",
    "/usr/local/share/cli4all/data/commands.yaml",
    "/opt/homebrew/share/cli4all/data/commands.yaml",
];
const UNIX_SHARE_RISKS_PATHS: &[&str] = &[
    "/usr/share/cli4all/data/risks.yaml",
    "/usr/local/share/cli4all/data/risks.yaml",
    "/opt/homebrew/share/cli4all/data/risks.yaml",
];

#[derive(Debug, Clone, Deserialize)]
pub struct CommandCatalog {
    pub commands: Vec<CommandIntent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommandIntent {
    pub intent: String,
    pub description: String,
    pub category: String,
    pub risk_level: String,
    pub commands: PlatformCommands,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PlatformCommands {
    #[serde(default)]
    pub windows_cmd: Vec<String>,
    #[serde(default)]
    pub powershell: Vec<String>,
    #[serde(default)]
    pub macos: Vec<String>,
    #[serde(default)]
    pub ubuntu: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskCatalog {
    pub risks: Vec<RiskRule>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RiskRule {
    pub name: String,
    pub level: String,
    pub reason: String,
    #[serde(default)]
    pub patterns: Vec<String>,
}

pub fn load_command_catalog() -> Result<CommandCatalog> {
    let catalog: CommandCatalog = load_yaml_from_candidates(command_catalog_paths())?;
    validate_command_catalog(&catalog)?;
    Ok(catalog)
}

pub fn load_risk_catalog() -> Result<RiskCatalog> {
    let catalog: RiskCatalog = load_yaml_from_candidates(risk_catalog_paths())?;
    validate_risk_catalog(&catalog)?;
    Ok(catalog)
}

fn load_yaml<T>(path: impl AsRef<Path>) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let path = path.as_ref();
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read YAML file at {}", path.display()))?;
    serde_yaml::from_str(&contents)
        .with_context(|| format!("failed to parse YAML file at {}", path.display()))
}

fn load_yaml_from_candidates<T>(paths: Vec<PathBuf>) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    for path in &paths {
        if path.exists() {
            return load_yaml(path);
        }
    }

    let searched = paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    Err(anyhow::anyhow!(
        "failed to locate YAML file in any known path: {searched}"
    ))
}

fn command_catalog_paths() -> Vec<PathBuf> {
    candidate_paths(
        "commands.yaml",
        DEV_COMMANDS_PATH,
        UNIX_SHARE_COMMANDS_PATHS,
    )
}

fn risk_catalog_paths() -> Vec<PathBuf> {
    candidate_paths("risks.yaml", DEV_RISKS_PATH, UNIX_SHARE_RISKS_PATHS)
}

fn candidate_paths(file_name: &str, dev_path: &str, unix_paths: &[&str]) -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from(dev_path)];

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            paths.push(exe_dir.join("data").join(file_name));
            paths.push(exe_dir.join("..").join("data").join(file_name));
            paths.push(
                exe_dir
                    .join("..")
                    .join("share")
                    .join("cli4all")
                    .join("data")
                    .join(file_name),
            );
        }
    }

    paths.extend(unix_paths.iter().map(PathBuf::from));
    paths
}

fn validate_command_catalog(catalog: &CommandCatalog) -> Result<()> {
    for intent in &catalog.commands {
        for (platform, commands) in intent.commands.iter() {
            for command in commands {
                build_command_regex(command).with_context(|| {
                    format!(
                        "invalid command example for intent '{}' platform '{}': {}",
                        intent.intent, platform, command
                    )
                })?;
            }
        }
    }
    Ok(())
}

fn validate_risk_catalog(catalog: &RiskCatalog) -> Result<()> {
    for rule in &catalog.risks {
        validate_patterns(&rule.name, &rule.patterns)?;
    }
    Ok(())
}

fn validate_patterns(rule_name: &str, patterns: &[String]) -> Result<()> {
    for pattern in patterns {
        Regex::new(pattern)
            .with_context(|| format!("invalid regex pattern for rule '{rule_name}': {pattern}"))?;
    }
    Ok(())
}

pub fn build_command_regex(command: &str) -> Result<Regex> {
    let mut parts = Vec::new();

    for token in command.split_whitespace() {
        if let Some(placeholder) = placeholder_name(token) {
            parts.push(format!(r"(?P<{placeholder}>\S+)"));
        } else {
            parts.push(regex::escape(token));
        }
    }

    let pattern = format!(r"(?i)^\s*{}\s*$", parts.join(r"\s+"));
    Regex::new(&pattern).with_context(|| format!("invalid generated regex: {pattern}"))
}

pub fn placeholder_name(token: &str) -> Option<&str> {
    token.strip_prefix('<')?.strip_suffix('>')
}

impl CommandIntent {
    pub fn ubuntu_commands(&self) -> &[String] {
        &self.commands.ubuntu
    }
}

impl PlatformCommands {
    pub fn iter(&self) -> [(&'static str, &[String]); 4] {
        [
            ("windows_cmd", &self.windows_cmd),
            ("powershell", &self.powershell),
            ("macos", &self.macos),
            ("ubuntu", &self.ubuntu),
        ]
    }
}
