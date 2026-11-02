//! `constitution` CLI. Interface: docs/technical/ARCHITECTURE.md "CLI".
mod audit;
mod failure;
mod rpc;
mod rules;

use clap::{Parser, Subcommand};
use failure::Failure;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "constitution",
    version,
    about = "Operate and audit a CONSTITUTION gate"
)]
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
    /// Read-only F7 audit: prove the claim's preconditions from public state. Exits 1 on any failure.
    Audit {
        /// Deployment record written at activation.
        #[arg(long, default_value = "deployments/testnet.json")]
        deployment: PathBuf,
        /// Committed rule-set manifest the gate must enforce.
        #[arg(long, default_value = "rules/manifest.json")]
        manifest: PathBuf,
        #[arg(
            long,
            env = "NEAR_RPC_URL",
            default_value = "https://test.rpc.fastnear.com"
        )]
        near_rpc: String,
        #[arg(
            long,
            env = "BASE_SEPOLIA_RPC_URL",
            default_value = "https://sepolia.base.org"
        )]
        evm_rpc: String,
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
        Command::Audit {
            deployment,
            manifest,
            near_rpc,
            evm_rpc,
        } => {
            let manifest_hash = rules::hash_file(&manifest)?;
            let deployment = audit::read_deployment(&deployment)?;
            let observed = audit::observe(
                &deployment,
                &rpc::Near::new(&near_rpc),
                &rpc::Evm::new(&evm_rpc),
            )?;
            audit::report(&audit::evaluate(&deployment, manifest_hash, &observed))
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
