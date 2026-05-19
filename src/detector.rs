use anyhow::Result;
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::rules::{build_command_regex, placeholder_name};
use crate::store::normalize::normalize_command;
use crate::store::{CommandRecord, CommandStore};

#[derive(Debug, Clone)]
pub struct Detection {
    pub input: String,
    pub command: String,
    pub source_platform: String,
    pub matched_example: String,
    pub captures: BTreeMap<String, String>,
    pub intent: CommandRecord,
}

pub fn detect_command(store_input: &str, store: &impl CommandStore) -> Result<Option<Detection>> {
    let record = match store.find_by_command(store_input)? {
        Some(record) => record,
        None => return Ok(None),
    };

    let token = extract_command_token(store_input);
    let normalized_input = normalize_command(store_input);
    let mut full_matches = Vec::new();
    let mut token_matches = Vec::new();

    for (platform, commands) in record.iter_commands() {
        for example in commands {
            if let Some(captures) = match_example(store_input, example) {
                full_matches.push(Detection {
                    input: store_input.to_string(),
                    command: token
                        .clone()
                        .unwrap_or_else(|| first_token(example).to_string()),
                    source_platform: platform.to_string(),
                    matched_example: example.to_string(),
                    captures,
                    intent: record.clone(),
                });
                continue;
            }

            if normalize_command(example) == normalized_input {
                full_matches.push(Detection {
                    input: store_input.to_string(),
                    command: token
                        .clone()
                        .unwrap_or_else(|| first_token(example).to_string()),
                    source_platform: platform.to_string(),
                    matched_example: example.to_string(),
                    captures: BTreeMap::new(),
                    intent: record.clone(),
                });
                continue;
            }

            if let Some(token) = token.as_deref() {
                if first_token(example).eq_ignore_ascii_case(token) {
                    token_matches.push(Detection {
                        input: store_input.to_string(),
                        command: token.to_string(),
                        source_platform: platform.to_string(),
                        matched_example: example.to_string(),
                        captures: BTreeMap::new(),
                        intent: record.clone(),
                    });
                }
            }
        }
    }

    Ok(pick_best_match(full_matches).or_else(|| pick_best_match(token_matches)))
}

pub fn extract_command_token(input: &str) -> Option<String> {
    if let Some(captures) = command_not_found_regex().captures(input) {
        return captures.get(1).map(|value| value.as_str().to_string());
    }

    leading_token_regex()
        .captures(input)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}

fn command_not_found_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)command not found:\s*([A-Za-z0-9._-]+)").expect("valid regex")
    })
}

fn leading_token_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"^\s*([A-Za-z0-9._-]+)").expect("valid regex"))
}

fn match_example(input: &str, example: &str) -> Option<BTreeMap<String, String>> {
    let regex = build_command_regex(example).expect("validated command regex");
    let captures = regex.captures(input)?;
    let mut values = BTreeMap::new();

    for token in example.split_whitespace() {
        if let Some(name) = placeholder_name(token) {
            if let Some(value) = captures.name(name) {
                values.insert(name.to_string(), value.as_str().to_string());
            }
        }
    }

    Some(values)
}

fn first_token(command: &str) -> &str {
    command.split_whitespace().next().unwrap_or(command)
}

fn pick_best_match(matches: Vec<Detection>) -> Option<Detection> {
    matches
        .into_iter()
        .min_by_key(|detection| platform_rank(&detection.source_platform))
}

fn platform_rank(platform: &str) -> u8 {
    match platform {
        "ubuntu" => 0,
        "windows_cmd" => 1,
        "powershell" => 2,
        "macos" => 3,
        _ => 4,
    }
}
