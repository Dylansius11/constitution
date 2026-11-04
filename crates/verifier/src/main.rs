//! Chain-connected mode requires a socket-attested identity; local mode is explicit.
use constitution_verifier::{
    api::{Service, router, serve},
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
    let gate = if local {
        None
    } else {
        Some(
            required("GATE_ACCOUNT_ID")?
                .parse()
                .map_err(|_| "invalid GATE_ACCOUNT_ID")?,
        )
    };
    let peer_source = match std::env::var("VERIFIER_PEER_SOURCE") {
        Ok(value) if value == "socket" => value,
        Ok(value)
            if value
                .strip_prefix("header:")
                .is_some_and(|name| !name.is_empty()) =>
        {
            value
        }
        Err(_) if local => "socket".to_owned(),
        _ => return Err("VERIFIER_PEER_SOURCE must be socket or header:<name>".into()),
    };
    if let Some(name) = peer_source.strip_prefix("header:") {
        axum::http::HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| "invalid VERIFIER_PEER_SOURCE header")?;
    }
    let identity = Identity::new()
        .for_mode(
            local,
            std::path::Path::new(constitution_verifier::identity::DSTACK_SOCKET),
        )
        .await?;
    let identity_label = identity.view([0; 32]).label;
    let mut service = if local {
        let registry = required("LOCAL_PROPOSAL_REGISTRY")?;
        Service::new(
            identity,
            manifest,
            repo,
            Box::new(LocalRegistry::load(registry)?),
        )?
    } else {
        let gate = gate.ok_or("missing GATE_ACCOUNT_ID")?;
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
        let source = GateSource::new(&near_rpc, &base_rpc, &manifest, gate)?;
        let submitter = NearSubmitter::new(&near_rpc, &account, &keyfile)?;
        Service::with_submitter(
            identity,
            manifest,
            repo,
            Box::new(source),
            Box::new(submitter),
        )?
    };
    if let Some(header) = peer_source.strip_prefix("header:") {
        Service::trust_proxy_header(&mut service, header)?;
    }
    let app = router(service, &origin)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787").await?;
    eprintln!(
        "verifier listening on 127.0.0.1:8787; {identity_label}; Pass requires provider_tee review; mode={}; admission peers keyed by {}",
        if local {
            "local registry (no submission)"
        } else {
            "gate RPC and verdict submission"
        },
        peer_source,
    );
    serve(listener, app).await?;
    Ok(())
}
