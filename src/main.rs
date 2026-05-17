use anyhow::Result;
use clap::Parser;

use cli4all::cli::{Cli, Commands};
use cli4all::detector::detect_command;
use cli4all::explainer::explain_command;
use cli4all::fixer::suggest_fix;
use cli4all::output::{
    format_check, format_explanation, format_fix, format_risk, format_translation,
    format_unknown_command,
};
use cli4all::rules::{load_command_catalog, load_risk_catalog};
use cli4all::safety::assess_risk;
use cli4all::translator::translate_command;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let command_catalog = load_command_catalog()?;
    let risk_catalog = load_risk_catalog()?;

    let output = match cli.command {
        Commands::Check { input } => match detect_command(&input, &command_catalog) {
            Some(detection) => format_check(&detection),
            None => format_unknown_command(&input),
        },
        Commands::Translate { input, to } => {
            match translate_command(&input, &to, &command_catalog)? {
                Some(result) => format_translation(&result),
                None => format_unknown_command(&input),
            }
        }
        Commands::Explain { input } => {
            let result = explain_command(&input, &risk_catalog);
            format_explanation(&result)
        }
        Commands::Risk { input } => {
            let result = assess_risk(&input, &risk_catalog);
            format_risk(&result)
        }
        Commands::Fix { input } => match suggest_fix(&input, &command_catalog) {
            Some(result) => format_fix(&result),
            None => format_unknown_command(&input),
        },
    };

    println!("{output}");

    Ok(())
}
