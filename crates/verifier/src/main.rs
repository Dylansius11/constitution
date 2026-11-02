//! [I] Local-only service. Explicit insecure flag is required, never a testnet identity.
use constitution_verifier::{
    api::{Service, router},
    identity::Identity,
    manifest::load_manifest,
    proposals::LocalRegistry,
};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() != Some("--insecure-dev-identity") {
        return Err("requires --insecure-dev-identity (local only, not attested)".into());
    }
    let repo = std::env::current_dir()?;
    let manifest = load_manifest(repo.join("rules/manifest.json"))?;
    let registry = std::env::var("LOCAL_PROPOSAL_REGISTRY")?;
    let service = Service::new(
        Identity::new(),
        manifest,
        repo,
        Box::new(LocalRegistry::load(registry)?),
    )?;
    let origin = std::env::var("PAGE_ORIGIN")?;
    let app = router(service, &origin)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8787").await?;
    eprintln!("local verifier listening on 127.0.0.1:8787; NOT ATTESTED; no Pass certificates");
    axum::serve(listener, app).await?;
    Ok(())
}
