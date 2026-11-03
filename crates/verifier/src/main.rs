//! Chain-connected by default. Identity remains explicitly NOT ATTESTED.
use constitution_verifier::{
    api::{Service, router},
    chain::GateSource,
    identity::Identity,
    manifest::load_manifest,
    proposals::LocalRegistry,
    submit::NearSubmitter,
};
fn required(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("missing required environment variable {name}"))
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let local = match args.as_slice() {
        [] => false,
        [flag] if flag == "--insecure-dev-identity" => true,
        _ => return Err("usage: constitution-verifier [--insecure-dev-identity]".into()),
    };
    let repo = std::env::current_dir()?;
    let manifest = load_manifest(repo.join("rules/manifest.json"))?;
    let origin = required("PAGE_ORIGIN")?;
    let identity = Identity::new();
    let service = if local {
        let registry = required("LOCAL_PROPOSAL_REGISTRY")?;
        Service::new(
            identity,
            manifest,
            repo,
            Box::new(LocalRegistry::load(registry)?),
        )?
    } else {
        let near_rpc = required("NEAR_RPC_URL")?;
        let base_rpc = match std::env::var("BASE_SEPOLIA_ARCHIVE_RPC_URL") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => required("BASE_SEPOLIA_RPC_URL")?,
        };
        let account = required("VERIFIER_RELAY_ACCOUNT_ID")?;
        let keyfile = match std::env::var("VERIFIER_RELAY_KEY_FILE") {
            Ok(value) if !value.trim().is_empty() => std::path::PathBuf::from(value),
            _ => repo
                .join(".secrets/near-credentials")
                .join(required("NEAR_NETWORK_ID")?)
                .join(format!("{account}.json")),
        };
        let source = GateSource::new(&near_rpc, &base_rpc, &manifest)?;
        let submitter = NearSubmitter::new(&near_rpc, &account, &keyfile)?;
        Service::with_submitter(
            identity,
            manifest,
            repo,
            Box::new(source),
            Box::new(submitter),
        )?
    };
    let app = router(service, &origin)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787").await?;
    eprintln!(
        "verifier listening on 127.0.0.1:8787; NOT ATTESTED; no Pass certificates; mode={}",
        if local {
            "local registry (no submission)"
        } else {
            "gate RPC and verdict submission"
        }
    );
    axum::serve(listener, app).await?;
    Ok(())
}
