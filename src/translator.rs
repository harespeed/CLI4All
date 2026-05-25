use anyhow::{anyhow, Result};
use std::collections::BTreeSet;

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
    let suggestions =
        normalize_translated_suggestions(detection, target_platform.key(), suggestions);

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

fn normalize_translated_suggestions(
    detection: &Detection,
    target: &str,
    suggestions: Vec<String>,
) -> Vec<String> {
    let normalized = match detection.intent.intent.as_str() {
        "list_files" | "list_all_files" => normalize_list_command(detection, target),
        "ping_host" => normalize_ping_command(detection, target),
        "search_text" => normalize_search_text_command(detection, target),
        "count_lines_words_chars" => normalize_word_count_command(detection, target),
        "list_listening_ports" => normalize_listening_ports_command(detection, target),
        "process_by_port" => normalize_process_by_port_command(detection, target),
        _ => None,
    };

    prepend_suggestion(normalized, suggestions)
}

fn normalize_list_command(detection: &Detection, target: &str) -> Option<String> {
    let parsed = parse_list_like_input(detection);
    let path = detection.captures.get("path").cloned().or(parsed.path);
    let wants_all = detection.intent.intent == "list_all_files" || parsed.wants_all;

    let mut parts = match target {
        "windows" => vec!["Get-ChildItem".to_string()],
        "macos" | "ubuntu" => vec!["ls".to_string()],
        _ => return None,
    };

    if wants_all {
        match target {
            "windows" => parts.push("-Force".to_string()),
            "macos" | "ubuntu" => parts.push("-la".to_string()),
            _ => {}
        }
    }

    if let Some(path) = path {
        parts.push(path);
    }

    Some(parts.join(" "))
}

fn normalize_ping_command(detection: &Detection, target: &str) -> Option<String> {
    let parsed = parse_ping_input(detection);
    let host = detection.captures.get("host").cloned().or(parsed.host)?;

    match target {
        "windows" => {
            let mut parts = vec!["Test-Connection".to_string()];
            if let Some(count) = parsed.count {
                parts.push("-Count".to_string());
                parts.push(count);
            }
            parts.push(host);
            Some(parts.join(" "))
        }
        "macos" | "ubuntu" => Some(format!(
            "ping -c {} {}",
            parsed.count.unwrap_or_else(|| "4".to_string()),
            host
        )),
        _ => None,
    }
}

fn normalize_search_text_command(detection: &Detection, target: &str) -> Option<String> {
    let parsed = parse_search_input(detection);
    let pattern = detection
        .captures
        .get("pattern")
        .cloned()
        .or(parsed.pattern)?;
    let file = detection.captures.get("file").cloned().or(parsed.file)?;

    match target {
        "windows" => {
            let mut parts = vec!["Select-String".to_string()];
            if !parsed.case_insensitive {
                parts.push("-CaseSensitive".to_string());
            }
            parts.push(pattern);
            parts.push(file);
            Some(parts.join(" "))
        }
        "macos" | "ubuntu" => {
            let mut parts = vec!["grep".to_string()];
            if parsed.case_insensitive {
                parts.push("-i".to_string());
            }
            parts.push(pattern);
            parts.push(file);
            Some(parts.join(" "))
        }
        _ => None,
    }
}

fn normalize_word_count_command(detection: &Detection, target: &str) -> Option<String> {
    let parsed = parse_word_count_input(detection);
    let file = detection.captures.get("file").cloned().or(parsed.file)?;

    match target {
        "windows" => {
            if parsed.lines_only {
                Some(format!("Get-Content {file} | Measure-Object -Line"))
            } else {
                Some(format!(
                    "Get-Content {file} | Measure-Object -Line -Word -Character"
                ))
            }
        }
        "macos" | "ubuntu" => {
            if parsed.lines_only {
                Some(format!("wc -l {file}"))
            } else {
                Some(format!("wc {file}"))
            }
        }
        _ => None,
    }
}

fn normalize_listening_ports_command(detection: &Detection, target: &str) -> Option<String> {
    match target {
        "windows" => Some("Get-NetTCPConnection -State Listen".to_string()),
        "macos" => {
            if detection.source_platform == "windows_cmd"
                || detection
                    .input
                    .to_ascii_lowercase()
                    .contains("netstat -ano")
            {
                Some("netstat -an".to_string())
            } else {
                Some("lsof -i -P -n | grep LISTEN".to_string())
            }
        }
        "ubuntu" => Some("ss -tulnp".to_string()),
        _ => None,
    }
}

fn normalize_process_by_port_command(detection: &Detection, target: &str) -> Option<String> {
    let parsed = parse_port_query_input(detection);
    let raw_port = detection
        .captures
        .get("port")
        .cloned()
        .or_else(|| detection.captures.get("port_token").cloned())
        .or(parsed.port_token)?;
    let port = strip_port_prefix(&raw_port).to_string();

    match target {
        "windows" => Some(format!("Get-NetTCPConnection -LocalPort {port}")),
        "macos" | "ubuntu" => Some(format!("lsof -i :{port}")),
        _ => None,
    }
}

fn prepend_suggestion(normalized: Option<String>, suggestions: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();

    if let Some(command) = normalized {
        if seen.insert(command.clone()) {
            ordered.push(command);
        }
    }

    for suggestion in suggestions {
        if seen.insert(suggestion.clone()) {
            ordered.push(suggestion);
        }
    }

    ordered
}

#[derive(Default)]
struct ParsedListInput {
    wants_all: bool,
    path: Option<String>,
}

fn parse_list_like_input(detection: &Detection) -> ParsedListInput {
    let tokens = shellish_tokens(&detection.input);
    let mut parsed = ParsedListInput::default();
    let mut iter = tokens.into_iter();
    let _ = iter.next();

    for token in iter {
        if is_windows_dir_option(&token) {
            if token.eq_ignore_ascii_case("/a") || token.to_ascii_lowercase().starts_with("/a") {
                parsed.wants_all = true;
            }
            continue;
        }

        if is_unix_option(&token) {
            if token_contains_short_flag(&token, 'a') || token.eq_ignore_ascii_case("-force") {
                parsed.wants_all = true;
            }
            continue;
        }

        if parsed.path.is_none() {
            parsed.path = Some(token);
        }
    }

    parsed
}

#[derive(Default)]
struct ParsedPingInput {
    count: Option<String>,
    host: Option<String>,
}

fn parse_ping_input(detection: &Detection) -> ParsedPingInput {
    let tokens = shellish_tokens(&detection.input);
    let mut parsed = ParsedPingInput::default();
    let mut index = 1;

    while index < tokens.len() {
        let token = &tokens[index];

        if token.eq_ignore_ascii_case("-n") || token.eq_ignore_ascii_case("-c") {
            if let Some(next) = tokens.get(index + 1) {
                parsed.count = Some(next.clone());
                index += 2;
                continue;
            }
        }

        if let Some(value) = token
            .strip_prefix("-n")
            .or_else(|| token.strip_prefix("-c"))
            .filter(|value| !value.is_empty())
        {
            parsed.count = Some(value.to_string());
            index += 1;
            continue;
        }

        if !is_unix_option(token) && !is_windows_dir_option(token) {
            parsed.host = Some(token.clone());
        }
        index += 1;
    }

    parsed
}

#[derive(Default)]
struct ParsedSearchInput {
    case_insensitive: bool,
    pattern: Option<String>,
    file: Option<String>,
}

fn parse_search_input(detection: &Detection) -> ParsedSearchInput {
    let tokens = shellish_tokens(&detection.input);
    let mut parsed = ParsedSearchInput::default();
    let mut positionals = Vec::new();

    for token in tokens.into_iter().skip(1) {
        if token.eq_ignore_ascii_case("/i") || token_contains_short_flag(&token, 'i') {
            parsed.case_insensitive = true;
            continue;
        }

        if is_unix_option(&token) || is_windows_dir_option(&token) {
            continue;
        }

        positionals.push(token);
    }

    parsed.pattern = positionals.first().cloned();
    parsed.file = positionals.get(1).cloned();
    parsed
}

#[derive(Default)]
struct ParsedWordCountInput {
    lines_only: bool,
    file: Option<String>,
}

fn parse_word_count_input(detection: &Detection) -> ParsedWordCountInput {
    let tokens = shellish_tokens(&detection.input);
    let mut parsed = ParsedWordCountInput::default();

    for token in tokens.into_iter().skip(1) {
        if token == "-l" {
            parsed.lines_only = true;
            continue;
        }

        if is_unix_option(&token) || is_windows_dir_option(&token) || token == "|" {
            continue;
        }

        parsed.file = Some(token);
    }

    parsed
}

#[derive(Default)]
struct ParsedPortQueryInput {
    port_token: Option<String>,
}

fn parse_port_query_input(detection: &Detection) -> ParsedPortQueryInput {
    let tokens = shellish_tokens(&detection.input);
    let mut parsed = ParsedPortQueryInput::default();

    for token in tokens.into_iter().skip(1) {
        if token == "-i" || token.eq_ignore_ascii_case("-localport") {
            continue;
        }

        if token.eq_ignore_ascii_case("|")
            || token.eq_ignore_ascii_case("findstr")
            || token.eq_ignore_ascii_case("netstat")
            || token.eq_ignore_ascii_case("Get-NetTCPConnection")
        {
            continue;
        }

        if token.eq_ignore_ascii_case("-ano") || token.eq_ignore_ascii_case("-localport") {
            continue;
        }

        if let Some(value) = token.strip_prefix("-LocalPort") {
            if !value.is_empty() {
                parsed.port_token = Some(value.to_string());
            }
            continue;
        }

        if parsed.port_token.is_none()
            && (token.starts_with(':')
                || token.chars().all(|ch| ch.is_ascii_digit())
                || token.contains(":"))
        {
            parsed.port_token = Some(token);
        }
    }

    parsed
}

fn strip_port_prefix(port: &str) -> &str {
    port.trim_start_matches(':')
}

fn shellish_tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(active_quote) = quote {
            current.push(ch);
            if ch == '\\' {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
                continue;
            }
            if ch == active_quote {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                quote = Some(ch);
                current.push(ch);
            }
            c if c.is_whitespace() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn is_unix_option(token: &str) -> bool {
    token.starts_with('-') && token.len() > 1
}

fn is_windows_dir_option(token: &str) -> bool {
    token.starts_with('/') && token.len() > 1
}

fn token_contains_short_flag(token: &str, flag: char) -> bool {
    token
        .strip_prefix('-')
        .map(|value| {
            value
                .chars()
                .any(|candidate| candidate.to_ascii_lowercase() == flag.to_ascii_lowercase())
        })
        .unwrap_or(false)
}
