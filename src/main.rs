use anyhow::Result;
use clap::Parser;

use cli4all::cli::{Cli, Commands};
use cli4all::detector::detect_command;
use cli4all::executor::SystemCommandExecutor;
use cli4all::explainer::explain_command;
use cli4all::fixer::suggest_fix;
use cli4all::output::{
    format_check, format_explanation, format_fix, format_risk, format_translation,
    format_unknown_command,
};
use cli4all::platform::Platform;
use cli4all::rules::load_risk_catalog;
use cli4all::safety::assess_risk;
use cli4all::shell::run_shell;
use cli4all::store::{build_command_index, load_command_store};
use cli4all::translator::translate_command;

fn main() -> Result<()> {
    let cli = Cli::parse();

    let output = match cli.command {
        Commands::BuildIndex { input, index, data } => {
            build_command_index(&input, &index, &data)?;
            format!(
                "Built command index:\n- source: {}\n- index: {}\n- data: {}",
                input.display(),
                index.display(),
                data.display()
            )
        }
        Commands::Check { input } => {
            let command_store = load_command_store()?;
            match detect_command(&input, &command_store)? {
                Some(detection) => format_check(&detection),
                None => format_unknown_command(&input),
            }
        }
        Commands::Translate { input, to } => {
            let command_store = load_command_store()?;
            match translate_command(&input, &to, &command_store)? {
                Some(result) => format_translation(&result),
                None => format_unknown_command(&input),
            }
        }
        Commands::Explain { input } => {
            let risk_catalog = load_risk_catalog()?;
            let result = explain_command(&input, &risk_catalog);
            format_explanation(&result)
        }
        Commands::Risk { input } => {
            let risk_catalog = load_risk_catalog()?;
            let result = assess_risk(&input, &risk_catalog);
            format_risk(&result)
        }
        Commands::Fix { input } => {
            let command_store = load_command_store()?;
            match suggest_fix(&input, &command_store)? {
                Some(result) => format_fix(&result),
                None => format_unknown_command(&input),
            }
        }
        Commands::Shell => {
            let command_store = load_command_store()?;
            let risk_catalog = load_risk_catalog()?;
            let current_platform = Platform::detect_current()?;
            let executor = SystemCommandExecutor::new(current_platform);
            run_shell(current_platform, &command_store, &risk_catalog, &executor)?;
            return Ok(());
        }
    };

    println!("{output}");

    Ok(())
}
