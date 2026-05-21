use anyhow::{anyhow, Result};

use crate::detector::{detect_command, Detection};
use crate::platform::normalize_target_platform;
use crate::rules::apply_template_captures;
use crate::store::CommandStore;

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
    store: &impl CommandStore,
) -> Result<Option<TranslationResult>> {
    let detection = match detect_command(input, store)? {
        Some(detection) => detection,
        None => return Ok(None),
    };

    translate_detection(&detection, target)
}

pub fn translate_detection(
    detection: &Detection,
    target: &str,
) -> Result<Option<TranslationResult>> {
    let target_platform = normalize_target_platform(target)
        .ok_or_else(|| anyhow!("unsupported target '{target}'"))?;
    let suggestions = detection
        .intent
        .target_commands(target_platform.key())
        .iter()
        .map(|command| apply_template_captures(command, &detection.captures))
        .filter(|command| !contains_placeholder(command))
        .collect::<Vec<_>>();

    if suggestions.is_empty() {
        return Ok(None);
    }

    Ok(Some(TranslationResult {
        command: detection.command.clone(),
        source: display_platform_name(&detection.source_platform).to_string(),
        target: target_platform.key().to_string(),
        description: detection.intent.description.clone(),
        category: detection.intent.category.clone(),
        risk_level: detection.intent.risk_level.clone(),
        suggestions,
    }))
}
pub fn display_platform_name(platform: &str) -> &str {
    match platform {
        "windows_cmd" => "Windows CMD",
        "powershell" => "PowerShell",
        "windows" => "Windows",
        "macos" => "macOS",
        "linux" | "ubuntu" => "Ubuntu",
        _ => platform,
    }
}

fn contains_placeholder(command: &str) -> bool {
    command
        .split_whitespace()
        .any(|token| token.starts_with('<') && token.ends_with('>'))
}
