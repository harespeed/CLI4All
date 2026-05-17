use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::rules::{build_command_regex, placeholder_name, CommandCatalog, CommandIntent};

#[derive(Debug, Clone)]
pub struct Detection {
    pub input: String,
    pub command: String,
    pub source_platform: String,
    pub matched_example: String,
    pub captures: BTreeMap<String, String>,
    pub intent: CommandIntent,
}

pub fn detect_command(input: &str, catalog: &CommandCatalog) -> Option<Detection> {
    let token = extract_command_token(input);
    let mut full_matches = Vec::new();
    let mut token_matches = Vec::new();

    for intent in &catalog.commands {
        for (platform, commands) in intent.commands.iter() {
            for example in commands {
                if let Some(captures) = match_example(input, example) {
                    full_matches.push(Detection {
                        input: input.to_string(),
                        command: token
                            .clone()
                            .unwrap_or_else(|| first_token(example).to_string()),
                        source_platform: platform.to_string(),
                        matched_example: example.clone(),
                        captures,
                        intent: intent.clone(),
                    });
                    continue;
                }

                if let Some(token) = token.as_deref() {
                    if first_token(example).eq_ignore_ascii_case(token) {
                        token_matches.push(Detection {
                            input: input.to_string(),
                            command: token.to_string(),
                            source_platform: platform.to_string(),
                            matched_example: example.clone(),
                            captures: BTreeMap::new(),
                            intent: intent.clone(),
                        });
                    }
                }
            }
        }
    }

    pick_best_match(full_matches).or_else(|| pick_best_match(token_matches))
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
