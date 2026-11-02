//! `constitution` CLI. Interface: docs/technical/ARCHITECTURE.md "CLI".
mod audit;
mod failure;
mod relay;
mod rpc;
mod rules;
mod signed_tx;

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
    /// Broadcast the raw transaction the gate signed, after recovering its sender off-chain.
    Relay {
        /// Proposal id (or change id with --change).
        id: u64,
        /// Relay an applied MigrateAuthority change instead of a proposal.
        #[arg(long)]
        change: bool,
        /// Wait up to this many seconds for inclusion and report the receipt.
        #[arg(long, value_name = "SECONDS")]
        watch: Option<u64>,
        #[arg(long, env = "GATE_ACCOUNT_ID")]
        gate: String,
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
        #[arg(
            long,
            env = "BASE_SEPOLIA_SEND_URL",
            default_value = "https://sepolia-sequencer.base.org"
        )]
        send_rpc: String,
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
        Command::Relay {
            id,
            change,
            watch,
            gate,
            near_rpc,
            evm_rpc,
            send_rpc,
        } => relay::relay(
            &rpc::Near::new(&near_rpc),
            &gate,
            if change {
                relay::Target::Change(id)
            } else {
                relay::Target::Proposal(id)
            },
            &rpc::Evm::new(&send_rpc),
            &rpc::Evm::new(&evm_rpc),
            watch.map(std::time::Duration::from_secs),
        ),
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
