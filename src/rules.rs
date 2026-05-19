use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;
use std::fs;
use std::path::Path;

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

pub fn load_command_catalog() -> Result<crate::store::C4DbCommandStore> {
    crate::store::load_command_store()
}

pub fn load_risk_catalog() -> Result<RiskCatalog> {
    let risk_path = crate::data_paths::find_data_file("risks.yaml")?;
    let catalog: RiskCatalog = load_yaml(&risk_path)?;
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
