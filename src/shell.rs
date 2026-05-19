use anyhow::{Context, Result};
use serde::Serialize;
use std::io::{self, Write};

use crate::detector::detect_command;
use crate::executor::{CommandExecutor, ExecutionResult};
use crate::platform::{normalize_target_platform, Platform};
use crate::rules::RiskCatalog;
use crate::safety::assess_risk;
use crate::store::CommandStore;
use crate::translator::{display_platform_name, translate_detection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellDecision {
    pub original_command: String,
    pub detected_source: String,
    pub current_platform: String,
    pub intent: Option<String>,
    pub translated_command: Option<String>,
    pub risk_level: String,
    pub risk_reason: Option<String>,
    pub action: ShellAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShellAction {
    Execute,
    Confirm,
    Block,
    UnknownNoExecute,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TerminalResponse {
    pub original_command: String,
    pub detected_source: String,
    pub current_os: String,
    pub matched_intent: Option<String>,
    pub translated_command: Option<String>,
    pub risk_level: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_status: Option<i32>,
    pub action: ShellAction,
    pub risk_reason: Option<String>,
    pub message: Option<String>,
    pub confirmation_prompt: Option<String>,
}

pub fn decide_shell_command(
    input: &str,
    current_platform: &str,
    store: &impl CommandStore,
    risk_catalog: &RiskCatalog,
) -> Result<ShellDecision> {
    let platform = normalize_target_platform(current_platform)
        .with_context(|| format!("unsupported shell platform '{current_platform}'"))?;
    let original_command = input.trim().to_string();
    let original_risk = assess_risk(&original_command, risk_catalog);

    let detection = match detect_command(&original_command, store)? {
        Some(detection) => detection,
        None => {
            return Ok(ShellDecision {
                original_command,
                detected_source: "unknown".to_string(),
                current_platform: platform.key().to_string(),
                intent: None,
                translated_command: None,
                risk_level: unknown_or_risk(&original_risk.level),
                risk_reason: reason_if_known(&original_risk.reason, &original_risk.level),
                action: if is_destructive(&original_risk.level) {
                    ShellAction::Block
                } else {
                    ShellAction::UnknownNoExecute
                },
            });
        }
    };

    let translated_command = translate_detection(&detection, platform.key())?
        .and_then(|result| result.suggestions.into_iter().next());
    let translated_risk = translated_command
        .as_deref()
        .map(|command| assess_risk(command, risk_catalog));
    let risk_level = strongest_risk_level(&[
        detection.intent.risk_level.as_str(),
        original_risk.level.as_str(),
        translated_risk
            .as_ref()
            .map(|risk| risk.level.as_str())
            .unwrap_or("none"),
    ]);

    let action = if is_destructive(&risk_level) {
        ShellAction::Block
    } else if translated_command.is_none() {
        ShellAction::UnknownNoExecute
    } else if matches!(risk_level.as_str(), "medium" | "high") {
        ShellAction::Confirm
    } else {
        ShellAction::Execute
    };

    Ok(ShellDecision {
        original_command,
        detected_source: detection.source_platform,
        current_platform: platform.key().to_string(),
        intent: Some(detection.intent.intent),
        translated_command,
        risk_level,
        risk_reason: translated_risk
            .as_ref()
            .map(|risk| risk.reason.clone())
            .or_else(|| reason_if_known(&original_risk.reason, &original_risk.level)),
        action,
    })
}

pub fn run_shell(
    current_platform: Platform,
    store: &impl CommandStore,
    risk_catalog: &RiskCatalog,
    executor: &impl CommandExecutor,
) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut input = String::new();

    loop {
        print!("cli4all-{}> ", current_platform.prompt_name());
        stdout.flush().context("failed to flush shell prompt")?;

        input.clear();
        if stdin
            .read_line(&mut input)
            .context("failed to read shell input")?
            == 0
        {
            println!();
            break;
        }

        let trimmed = input.trim();
        if trimmed.is_empty() {
            continue;
        }

        if matches!(trimmed.to_ascii_lowercase().as_str(), "exit" | "quit") {
            break;
        }

        let decision = decide_shell_command(trimmed, current_platform.key(), store, risk_catalog)?;
        print_translation_section(&decision);

        match decision.action {
            ShellAction::Execute => {
                let result = execute_decision(&decision, executor)?;
                print_execution_sections(&result);
            }
            ShellAction::Confirm => {
                if confirm_execution(&decision.risk_level)? {
                    let result = execute_decision(&decision, executor)?;
                    print_execution_sections(&result);
                } else {
                    print_notice_section("CLI4ALL Notice", &["Execution cancelled."]);
                }
            }
            ShellAction::Block => {
                let reason = decision
                    .risk_reason
                    .as_deref()
                    .unwrap_or("This command matches a destructive safety rule.");
                print_notice_section(
                    "CLI4ALL Safety",
                    &[
                        "Destructive command blocked by CLI4ALL.",
                        &format!("Reason: {reason}"),
                    ],
                );
            }
            ShellAction::UnknownNoExecute => {
                print_notice_section(
                    "CLI4ALL Notice",
                    &[
                        "Unknown command mapping. CLI4ALL will not execute this command automatically in safe mode.",
                    ],
                );
            }
        }
    }

    Ok(())
}

pub fn process_terminal_command(
    input: &str,
    current_platform: Platform,
    store: &impl CommandStore,
    risk_catalog: &RiskCatalog,
    executor: &impl CommandExecutor,
) -> Result<TerminalResponse> {
    let decision = decide_shell_command(input, current_platform.key(), store, risk_catalog)?;
    let mut response = TerminalResponse {
        original_command: decision.original_command.clone(),
        detected_source: display_platform_name(&decision.detected_source).to_string(),
        current_os: current_platform.display_name().to_string(),
        matched_intent: decision.intent.clone(),
        translated_command: decision.translated_command.clone(),
        risk_level: decision.risk_level.clone(),
        stdout: String::new(),
        stderr: String::new(),
        exit_status: None,
        action: decision.action.clone(),
        risk_reason: decision.risk_reason.clone(),
        message: None,
        confirmation_prompt: None,
    };

    match decision.action {
        ShellAction::Execute => {
            let result = execute_decision(&decision, executor)?;
            response.stdout = result.stdout;
            response.stderr = result.stderr;
            response.exit_status = result.status_code;
        }
        ShellAction::Confirm => {
            response.message = Some("Confirmation required before execution.".to_string());
            response.confirmation_prompt = Some(confirmation_prompt(&decision.risk_level));
        }
        ShellAction::Block => {
            response.message = Some("Destructive command blocked by CLI4ALL.".to_string());
        }
        ShellAction::UnknownNoExecute => {
            response.message = Some(
                "Unknown command mapping. CLI4ALL will not execute this command automatically in safe mode."
                    .to_string(),
            );
        }
    }

    Ok(response)
}

fn print_translation_section(decision: &ShellDecision) {
    print_section_header("CLI4ALL Translation", 56);
    println!("Original command:   {}", decision.original_command);
    println!(
        "Detected source:    {}",
        display_platform_name(&decision.detected_source)
    );
    println!(
        "Current OS:         {}",
        display_platform_name(&decision.current_platform)
    );
    println!(
        "Matched intent:     {}",
        decision.intent.as_deref().unwrap_or("unknown")
    );
    println!(
        "Translated command: {}",
        decision
            .translated_command
            .as_deref()
            .unwrap_or("unavailable")
    );
    println!("Risk level:         {}", decision.risk_level);
    print_section_footer(53);
}

fn confirm_execution(risk_level: &str) -> Result<bool> {
    let prompt = format!("{} ", confirmation_prompt(risk_level));
    print!("{prompt}");
    io::stdout()
        .flush()
        .context("failed to flush confirmation prompt")?;

    let mut response = String::new();
    io::stdin()
        .read_line(&mut response)
        .context("failed to read confirmation response")?;

    Ok(matches!(
        response.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn execute_decision(
    decision: &ShellDecision,
    executor: &impl CommandExecutor,
) -> Result<ExecutionResult> {
    let command = decision
        .translated_command
        .as_deref()
        .context("shell decision did not include a translated command")?;

    executor.execute(command)
}

fn print_execution_sections(result: &ExecutionResult) {
    print_command_output_section(&result.stdout, &result.stderr);
    print_execution_result_section(result.status_code);
}

fn print_command_output_section(stdout: &str, stderr: &str) {
    if stdout.is_empty() && stderr.is_empty() {
        return;
    }

    print_section_header("Command Output", 48);

    if !stdout.is_empty() {
        println!("[stdout]");
        print_stream(stdout);
    }

    if !stderr.is_empty() {
        println!("[stderr]");
        print_stream(stderr);
    }

    print_section_footer(48);
}

fn print_execution_result_section(status_code: Option<i32>) {
    print_section_header("Execution Result", 51);
    match status_code {
        Some(code) => println!("Exit status: {code}"),
        None => println!("Exit status: unavailable"),
    }
    print_section_footer(50);
}

fn print_notice_section(title: &str, lines: &[&str]) {
    let width = match title {
        "CLI4ALL Safety" => 52,
        "CLI4ALL Notice" => 52,
        _ => 48,
    };
    print_section_header(title, width);
    for line in lines {
        println!("{line}");
    }
    print_section_footer(width);
}

fn print_section_header(title: &str, width: usize) {
    let left = "-".repeat(16);
    let right_len = width.saturating_sub(left.len() + title.len() + 2);
    let right = "-".repeat(right_len);
    println!("{left} {title} {right}");
}

fn print_section_footer(width: usize) {
    println!("{}", "-".repeat(width));
}

fn print_stream(stream: &str) {
    print!("{stream}");
    if !stream.ends_with('\n') {
        println!();
    }
}

fn strongest_risk_level(levels: &[&str]) -> String {
    let mut strongest = "low";
    let mut strongest_rank = 1_u8;

    for level in levels {
        let rank = risk_rank(level);
        if rank > strongest_rank {
            strongest = level;
            strongest_rank = rank;
        }
    }

    strongest.to_string()
}

fn unknown_or_risk(level: &str) -> String {
    if risk_rank(level) == 0 {
        "unknown".to_string()
    } else {
        level.to_string()
    }
}

fn is_destructive(level: &str) -> bool {
    level.eq_ignore_ascii_case("destructive")
}

fn reason_if_known(reason: &str, level: &str) -> Option<String> {
    if risk_rank(level) == 0 {
        None
    } else {
        Some(reason.to_string())
    }
}

fn confirmation_prompt(risk_level: &str) -> String {
    match risk_level {
        "high" => "Execute this high-risk command? [y/N]".to_string(),
        _ => "Execute this medium-risk command? [y/N]".to_string(),
    }
}

fn risk_rank(level: &str) -> u8 {
    match level.to_ascii_lowercase().as_str() {
        "destructive" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}
