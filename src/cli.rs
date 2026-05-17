use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "cli4all")]
#[command(about = "Deterministic cross-platform command helper for Ubuntu users")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
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
}
