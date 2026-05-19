use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "cli4all")]
#[command(about = "Deterministic cross-platform command translation terminal")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    BuildIndex {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        index: PathBuf,
        #[arg(long)]
        data: PathBuf,
    },
    Check {
        input: String,
    },
    Translate {
        input: String,
        #[arg(long)]
        to: String,
    },
    Explain {
        input: String,
    },
    Risk {
        input: String,
    },
    Fix {
        input: String,
    },
    Shell,
}
