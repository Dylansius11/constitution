//! `constitution` CLI. Interface: docs/technical/ARCHITECTURE.md "CLI".
mod failure;
mod rules;

use clap::{Parser, Subcommand};
use failure::Failure;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "constitution", version, about = "Operate and audit a CONSTITUTION gate")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Rule-set manifest tools.
    Rules {
        #[command(subcommand)]
        command: RulesCommand,
    },
}

#[derive(Subcommand)]
enum RulesCommand {
    /// Print the rule-set hash, sha256(borsh(manifest)), of a validated manifest.
    Hash { manifest: PathBuf },
}

fn run(cli: Cli) -> Result<(), Failure> {
    match cli.command {
        Command::Rules {
            command: RulesCommand::Hash { manifest },
        } => {
            println!("0x{}", hex::encode(rules::hash_file(&manifest)?));
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("{failure}");
            ExitCode::FAILURE
        }
    }
}
