//! `constitution` CLI. Interface: docs/technical/ARCHITECTURE.md "CLI".
mod appraise;
mod audit;
mod executor_build;
mod failure;
mod govern;
mod near_tx;
mod propose;
mod receipt;
mod relay;
mod rpc;
mod rules;
mod signed_tx;
mod status;

use clap::{Args, Parser, Subcommand};
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
    /// Register canonical source and send its sealed bundle to a verifier (identity not appraised).
    Propose {
        #[command(flatten)]
        signer: SignerArgs,
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        contract: String,
        #[arg(long = "impl")]
        implementation: String,
        #[arg(long, default_value = "0x")]
        init: String,
        #[arg(long, env = "VERIFIER_URL", hide_env_values = true)]
        verifier: String,
        #[arg(
            long,
            env = "BASE_SEPOLIA_RPC_URL",
            hide_env_values = true,
            hide_default_value = true,
            default_value = "https://sepolia.base.org"
        )]
        evm_rpc: String,
    },
    /// Read gate state and optionally verifier stages; poll every two seconds with --watch.
    Status {
        id: u64,
        #[command(flatten)]
        read: ReadArgs,
        #[arg(long)]
        run: Option<String>,
        #[arg(long, value_name = "SECONDS")]
        watch: Option<u64>,
    },
    /// Assemble constitution.receipt/1 from observed public state.
    Receipt {
        id: u64,
        #[command(flatten)]
        read: ReadArgs,
        #[arg(long)]
        run: Option<String>,
        #[arg(long)]
        json: bool,
    },
    /// Rule-set manifest tools.
    Rules {
        #[command(subcommand)]
        command: RulesCommand,
    },
    /// Independently appraise a verifier's Intel TDX evidence before registration.
    Verifier {
        #[command(subcommand)]
        command: VerifierCommand,
    },
    /// Print the masked executor runtime keccak256 from a local committed Foundry artifact.
    ExecutorRuntimeHash {
        #[arg(long, default_value = executor_build::DEFAULT_ARTIFACT)]
        executor_artifact: PathBuf,
    },
    /// Read-only F7 audit: prove the claim's preconditions from public state. Exits 1 on any failure.
    Audit {
        /// Deployment record written at activation.
        #[arg(long, default_value = "deployments/testnet.json")]
        deployment: PathBuf,
        /// Committed rule-set manifest the gate must enforce.
        #[arg(long, default_value = "rules/manifest.json")]
        manifest: PathBuf,
        /// Local artifact built from the committed executor source; defines immutable positions.
        #[arg(long, default_value = executor_build::DEFAULT_ARTIFACT)]
        executor_artifact: PathBuf,
        #[arg(
            long,
            env = "NEAR_RPC_URL",
            hide_env_values = true,
            hide_default_value = true,
            default_value = "https://test.rpc.fastnear.com"
        )]
        near_rpc: String,
        #[arg(
            long,
            env = "BASE_SEPOLIA_RPC_URL",
            hide_env_values = true,
            hide_default_value = true,
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
            hide_env_values = true,
            hide_default_value = true,
            default_value = "https://test.rpc.fastnear.com"
        )]
        near_rpc: String,
        #[arg(
            long,
            env = "BASE_SEPOLIA_RPC_URL",
            hide_env_values = true,
            hide_default_value = true,
            default_value = "https://sepolia.base.org"
        )]
        evm_rpc: String,
        #[arg(
            long,
            env = "BASE_SEPOLIA_SEND_URL",
            hide_env_values = true,
            hide_default_value = true,
            default_value = "https://sepolia-sequencer.base.org"
        )]
        send_rpc: String,
    },
    /// Governance: schedule, apply or cancel a timelocked change.
    Govern {
        #[command(flatten)]
        signer: SignerArgs,
        #[command(subcommand)]
        command: GovernCommand,
    },
}

#[derive(Subcommand)]
enum RulesCommand {
    /// Print the rule-set hash, sha256(borsh(manifest)), of a validated manifest.
    Hash { manifest: PathBuf },
}
#[derive(Subcommand)]
enum VerifierCommand {
    /// Verify Intel DCAP collateral, TCB, runtime measurements and key bindings.
    Appraise {
        url: String,
        #[arg(long, value_parser = appraise::parse_compose_hash)]
        expect_compose_hash: Option<constitution_types::Hex32>,
        /// Collateral service; signatures remain anchored to Intel's production root.
        #[arg(long, env = "PCCS_URL", default_value = appraise::DEFAULT_PCCS_URL, hide_env_values = true, hide_default_value = true)]
        pccs_url: String,
        #[arg(long)]
        json: bool,
    },
    /// Re-seal the committed source for an existing Pending proposal; no on-chain write.
    Run {
        #[arg(long)]
        proposal: u64,
        /// Source directory (requires --contract), or canonical Borsh SourceBundle file.
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        contract: Option<String>,
        #[arg(long, env = "GATE_ACCOUNT_ID")]
        gate: String,
        #[arg(
            long,
            env = "NEAR_RPC_URL",
            hide_env_values = true,
            hide_default_value = true,
            default_value = "https://test.rpc.fastnear.com"
        )]
        near_rpc: String,
        #[arg(long, env = "VERIFIER_URL", hide_env_values = true)]
        verifier_url: String,
    },
}

#[derive(Args)]
struct ReadArgs {
    #[arg(long, env = "GATE_ACCOUNT_ID")]
    gate: String,
    #[arg(
        long,
        env = "NEAR_RPC_URL",
        hide_env_values = true,
        hide_default_value = true,
        default_value = "https://test.rpc.fastnear.com"
    )]
    near_rpc: String,
    #[arg(
        long,
        env = "BASE_SEPOLIA_RPC_URL",
        hide_env_values = true,
        hide_default_value = true,
        default_value = "https://sepolia.base.org"
    )]
    evm_rpc: String,
    #[arg(long, env = "VERIFIER_URL", hide_env_values = true)]
    verifier: Option<String>,
    #[arg(long, default_value = "testnet", value_parser = ["testnet", "local"])]
    network: String,
}

impl ReadArgs {
    fn verifier(&self, run: Option<&str>) -> Result<Option<rpc::Verifier>, Failure> {
        run.map(|_| {
            let url = self.verifier.as_deref().ok_or_else(|| {
                Failure::new(
                    "--run requires a verifier URL",
                    "pass --verifier or set VERIFIER_URL",
                )
            })?;
            rpc::Verifier::new(url)
        })
        .transpose()
    }
}

/// Who signs a NEAR write. The key file never leaves the machine.
#[derive(Args)]
struct SignerArgs {
    /// Signing NEAR account.
    #[arg(long, global = true, env = "NEAR_ACCOUNT_ID")]
    account: Option<String>,
    /// near-cli legacy key file; default .secrets/near-credentials/<network>/<account>.json.
    #[arg(long, global = true)]
    key_file: Option<PathBuf>,
    #[arg(
        long,
        global = true,
        env = "NEAR_NETWORK_ID",
        default_value = "testnet"
    )]
    network: String,
    #[arg(long, global = true, env = "GATE_ACCOUNT_ID")]
    gate: Option<String>,
    #[arg(
        long,
        global = true,
        env = "NEAR_RPC_URL",
        hide_env_values = true,
        hide_default_value = true,
        default_value = "https://test.rpc.fastnear.com"
    )]
    near_rpc: String,
}

impl SignerArgs {
    /// Returns the gate id and a writer for the signing account.
    fn writer(&self) -> Result<(String, near_tx::Writer), Failure> {
        let gate = self
            .gate
            .clone()
            .ok_or_else(|| Failure::new("no gate account", "pass --gate or set GATE_ACCOUNT_ID"))?;
        let account = self.account.as_deref().ok_or_else(|| {
            Failure::new(
                "no signing account",
                "pass --account or set NEAR_ACCOUNT_ID",
            )
        })?;
        let key_file = self
            .key_file
            .clone()
            .unwrap_or_else(|| near_tx::default_key_file(&self.network, account));
        Ok((
            gate,
            near_tx::Writer::new(&self.near_rpc, account, &key_file)?,
        ))
    }
}

#[derive(Subcommand)]
enum GovernCommand {
    /// Governor: schedule a change, for example --kind '"Unpause"'.
    Schedule {
        #[arg(long)]
        kind: String,
    },
    /// Anyone: apply a matured change.
    Apply { change_id: u64 },
    /// Guardian: cancel a pending change.
    Cancel { change_id: u64 },
}

fn run(cli: Cli) -> Result<(), Failure> {
    match cli.command {
        Command::Verifier {
            command:
                VerifierCommand::Appraise {
                    url,
                    expect_compose_hash,
                    pccs_url,
                    json,
                },
        } => appraise::appraise(&url, &pccs_url, expect_compose_hash, json),
        Command::Verifier {
            command:
                VerifierCommand::Run {
                    proposal,
                    bundle,
                    contract,
                    gate,
                    near_rpc,
                    verifier_url,
                },
        } => propose::verifier_run(
            &bundle,
            contract.as_deref(),
            &gate,
            proposal,
            &rpc::Near::new(&near_rpc),
            &rpc::Verifier::new(&verifier_url)?,
        ),
        Command::Propose {
            signer,
            bundle,
            contract,
            implementation,
            init,
            verifier,
            evm_rpc,
        } => {
            let bytes = propose::canonical_bytes(&propose::build_bundle(&bundle, &contract)?)?;
            let (gate, writer) = signer.writer()?;
            let account = signer
                .account
                .as_deref()
                .expect("writer requires an account");
            propose::propose(
                propose::Input {
                    bytes: &bytes,
                    implementation: &implementation,
                    init: &init,
                    account,
                },
                &gate,
                &writer,
                &rpc::Near::new(&signer.near_rpc),
                &rpc::Evm::new(&evm_rpc),
                &rpc::Verifier::new(&verifier)?,
            )
        }
        Command::Status {
            id,
            read,
            run,
            watch,
        } => {
            let verifier = read.verifier(run.as_deref())?;
            status::status(
                &rpc::Near::new(&read.near_rpc),
                &read.gate,
                id,
                &rpc::Evm::new(&read.evm_rpc),
                &read.network,
                verifier.as_ref().zip(run.as_deref()),
                watch.map(std::time::Duration::from_secs),
            )
        }
        Command::Receipt {
            id,
            read,
            run,
            json,
        } => {
            let verifier = read.verifier(run.as_deref())?;
            receipt::receipt(
                &rpc::Near::new(&read.near_rpc),
                &read.gate,
                id,
                &rpc::Evm::new(&read.evm_rpc),
                &read.network,
                verifier.as_ref().zip(run.as_deref()),
                json,
            )
        }
        Command::Rules {
            command: RulesCommand::Hash { manifest },
        } => {
            println!("0x{}", hex::encode(rules::hash_file(&manifest)?));
            Ok(())
        }
        Command::ExecutorRuntimeHash { executor_artifact } => {
            println!(
                "0x{}",
                hex::encode(executor_build::ExecutorBuild::read(&executor_artifact)?.hash())
            );
            Ok(())
        }
        Command::Audit {
            deployment,
            manifest,
            executor_artifact,
            near_rpc,
            evm_rpc,
        } => {
            let manifest_hash = rules::hash_file(&manifest)?;
            let deployment = audit::read_deployment(&deployment)?;
            let build = executor_build::ExecutorBuild::read(&executor_artifact)?;
            let observed = audit::observe(
                &deployment,
                &rpc::Near::new(&near_rpc),
                &rpc::Evm::new(&evm_rpc),
                &build,
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
        Command::Govern { signer, command } => {
            let (gate, writer) = signer.writer()?;
            match command {
                GovernCommand::Schedule { kind } => govern::schedule(&writer, &gate, &kind),
                GovernCommand::Apply { change_id } => govern::apply(&writer, &gate, change_id),
                GovernCommand::Cancel { change_id } => govern::cancel(&writer, &gate, change_id),
            }
        }
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            print!("{error}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!(
                "{}",
                Failure::new(
                    error.to_string(),
                    "run constitution <command> --help and correct the arguments"
                )
            );
            return ExitCode::FAILURE;
        }
    };
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            eprintln!("{failure}");
            ExitCode::FAILURE
        }
    }
}
