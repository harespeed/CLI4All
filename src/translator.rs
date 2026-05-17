use anyhow::{bail, Result};

use crate::detector::detect_command;
use crate::rules::placeholder_name;
use crate::rules::CommandCatalog;

#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub command: String,
    pub source: String,
    pub target: String,
    pub description: String,
    pub category: String,
    pub risk_level: String,
    pub suggestions: Vec<String>,
}

pub fn translate_command(
    input: &str,
    target: &str,
    catalog: &CommandCatalog,
) -> Result<Option<TranslationResult>> {
    if !target.eq_ignore_ascii_case("ubuntu") {
        bail!("unsupported target '{target}'. v0.1 only supports ubuntu");
    }

    Ok(
        detect_command(input, catalog).map(|detection| TranslationResult {
            command: detection.command,
            source: display_platform_name(&detection.source_platform).to_string(),
            target: "ubuntu".to_string(),
            description: detection.intent.description.clone(),
            category: detection.intent.category.clone(),
            risk_level: detection.intent.risk_level.clone(),
            suggestions: detection
                .intent
                .ubuntu_commands()
                .iter()
                .map(|command| apply_captures(command, &detection.captures))
                .collect(),
        }),
    )
}

fn apply_captures(template: &str, captures: &std::collections::BTreeMap<String, String>) -> String {
    template
        .split_whitespace()
        .map(|token| {
            if let Some(name) = placeholder_name(token) {
                captures
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| token.to_string())
            } else {
                token.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn display_platform_name(platform: &str) -> &str {
    match platform {
        "windows_cmd" => "Windows CMD",
        "powershell" => "PowerShell",
        "macos" => "macOS",
        "ubuntu" => "Ubuntu",
        _ => platform,
    }
}
