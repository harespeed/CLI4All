use regex::Regex;

use crate::rules::RiskCatalog;
use crate::safety::{assess_risk, RiskAssessment};

#[derive(Debug, Clone)]
pub struct ExplanationItem {
    pub token: String,
    pub explanation: String,
}

#[derive(Debug, Clone)]
pub struct ExplanationResult {
    pub command: String,
    pub items: Vec<ExplanationItem>,
    pub risk: RiskAssessment,
}

pub fn explain_command(input: &str, risk_catalog: &RiskCatalog) -> ExplanationResult {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let command = tokens.first().copied().unwrap_or("").to_string();
    let items = tokens
        .iter()
        .enumerate()
        .map(|(index, token)| ExplanationItem {
            token: (*token).to_string(),
            explanation: explain_token(&command, token, index),
        })
        .collect();

    ExplanationResult {
        command: command.clone(),
        items,
        risk: assess_risk(input, risk_catalog),
    }
}

fn explain_token(command: &str, token: &str, index: usize) -> String {
    if index == 0 {
        return explain_command_name(command);
    }

    match (command, token) {
        ("chmod", "-R") => {
            "Applies the permission change recursively to all nested files and directories."
                .to_string()
        }
        ("chmod", ".") => "Targets the current directory.".to_string(),
        ("rm", "-rf") | ("rm", "-fr") => "Forces recursive deletion without prompting.".to_string(),
        ("rm", "/") => "Targets the filesystem root directory.".to_string(),
        _ if octal_permission_regex().is_match(token) => format_permission_explanation(token),
        _ => "No built-in explanation is available for this token.".to_string(),
    }
}

fn explain_command_name(command: &str) -> String {
    match command {
        "chmod" => "Changes file or directory permission bits.".to_string(),
        "rm" => "Removes files or directories.".to_string(),
        _ => "Command or executable name.".to_string(),
    }
}

fn format_permission_explanation(token: &str) -> String {
    if token == "777" {
        return "Grants read, write, and execute permissions to owner, group, and others."
            .to_string();
    }

    format!("Sets Unix permission bits to octal mode {token}.")
}

fn octal_permission_regex() -> &'static Regex {
    static REGEX: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^[0-7]{3,4}$").expect("valid regex"))
}
