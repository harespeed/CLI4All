use crate::detector::Detection;
use crate::explainer::ExplanationResult;
use crate::fixer::FixResult;
use crate::safety::RiskAssessment;
use crate::translator::TranslationResult;

pub fn format_check(result: &Detection) -> String {
    format!(
        "Detected source: {}\nTarget platform: Ubuntu\nSummary: {}\nUbuntu alternatives:\n{}",
        crate::translator::display_platform_name(&result.source_platform),
        result.intent.description,
        format_list(result.intent.ubuntu_commands())
    )
}

pub fn format_translation(result: &TranslationResult) -> String {
    format!(
        "Detected source: {}\nTarget platform: {}\nSummary: {}\nNative equivalents:\n{}",
        result.source,
        capitalize(&result.target),
        result.description,
        format_list(&result.suggestions)
    )
}

pub fn format_explanation(result: &ExplanationResult) -> String {
    let details = result
        .items
        .iter()
        .map(|item| format!("- {}: {}", item.token, item.explanation))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "Command: {}\nExplanation:\n{}\nRisk level: {}\nReason: {}",
        result.command, details, result.risk.level, result.risk.reason
    )
}

pub fn format_risk(result: &RiskAssessment) -> String {
    format!("Risk level: {}\nReason: {}", result.level, result.reason)
}

pub fn format_fix(result: &FixResult) -> String {
    format!(
        "{} is not a default Ubuntu command. It comes from {}.\nSuggested Ubuntu commands:\n{}",
        result.command,
        result.source,
        format_list(&result.suggestions)
    )
}

pub fn format_unknown_command(input: &str) -> String {
    format!(
        "No deterministic rule matched '{}'. Add a command record and rebuild the C4DB index to support this command.",
        input
    )
}

fn format_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}
