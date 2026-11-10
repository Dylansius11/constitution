use crate::failure::Failure;
use constitution_types::{Hex32, HexBytes};
use dcap_qvl::{
    QuoteCollateralV3,
    quote::{Quote, TDReport10},
    verify::QuoteVerifier,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384};

pub const DEFAULT_PCCS_URL: &str = "https://pccs.phala.network";
const CHECKS: [&str; 7] = [
    "identity",
    "dcap_signature_chain",
    "tcb_up_to_date",
    "rtmr3_replay",
    "compose_hash",
    "report_data",
    "release_hash",
];
const RECOVER: &str = "do not register this key; obtain fresh evidence from the expected Phala CVM and appraise again";

#[derive(Clone, Deserialize)]
pub struct Identity {
    pub verifier_pubkey: Hex32,
    pub transport_pubkey: Hex32,
    pub release_hash: Option<Hex32>,
    pub report_data_preimage: HexBytes,
    pub quote_hex: String,
    pub event_log: Vec<String>,
    pub compose_hash: Option<Hex32>,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub detail: String,
    pub next: Option<&'static str>,
}
impl Check {
    fn new(name: &'static str, result: Result<String, String>) -> Self {
        match result {
            Ok(detail) => Self {
                name,
                ok: true,
                detail,
                next: None,
            },
            Err(detail) => {
                let failure = Failure::new(detail, RECOVER);
                Self {
                    name,
                    ok: false,
                    detail: failure.cause,
                    next: Some(RECOVER),
                }
            }
        }
    }
}

fn hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    hex::decode(value.strip_prefix("0x").unwrap_or(value)).map_err(|e| format!("invalid hex: {e}"))
}
pub fn parse_compose_hash(value: &str) -> Result<Hex32, String> {
    hex_bytes(value)?
        .try_into()
        .map(Hex32)
        .map_err(|_| "compose hash must be 32 bytes".into())
}

pub fn verify_dcap(
    raw: &[u8],
    collateral: &QuoteCollateralV3,
    now: u64,
) -> (Check, Check, Option<TDReport10>) {
    match QuoteVerifier::new_prod().verify(raw, collateral, now) {
        Ok(verified) => {
            let report = verified.report.as_td10().copied();
            let signature = Check::new(
                "dcap_signature_chain",
                if report.is_some() {
                    Ok("Intel production root, quote signature, certificate chains and collateral verified".into())
                } else {
                    Err("quote is not Intel TDX".into())
                },
            );
            let status = tcb_check(&verified.status);
            (signature, status, report)
        }
        Err(e) => (
            Check::new("dcap_signature_chain", Err(format!("{e:#}"))),
            Check::new(
                "tcb_up_to_date",
                Err("unavailable: DCAP verification failed".into()),
            ),
            None,
        ),
    }
}

pub fn tcb_check(status: &str) -> Check {
    Check::new(
        "tcb_up_to_date",
        if status == "UpToDate" {
            Ok(status.into())
        } else {
            Err(format!(
                "TCB status {status}; required UpToDate; ask the CVM provider to update platform firmware/TCB"
            ))
        },
    )
}

#[derive(Deserialize)]
struct Event {
    imr: u32,
    event_type: u32,
    #[serde(default)]
    digest: String,
    event: String,
    event_payload: String,
    #[serde(default = "v1")]
    version: u32,
    preimage: Option<String>,
}
fn v1() -> u32 {
    1
}

fn replay(log: &[String]) -> Result<([u8; 48], [u8; 32]), String> {
    let mut rtmr = [0u8; 48];
    let mut compose = None;
    for (index, line) in log.iter().enumerate() {
        let event: Event = serde_json::from_str(line).map_err(|e| format!("event {index}: {e}"))?;
        if event.imr > 3 {
            return Err(format!("event {index}: invalid IMR index"));
        }
        if event.imr != 3 {
            continue;
        }
        if event.event_type != 0x08000001 {
            return Err(format!("event {index}: unknown RTMR3 event type"));
        }
        let payload = hex_bytes(&event.event_payload)?;
        let preimage = match event.version {
            1 => [
                event.event_type.to_le_bytes().as_slice(),
                b":",
                event.event.as_bytes(),
                b":",
                &payload,
            ]
            .concat(),
            2 => {
                let canonical = format!(
                    "{{\"name\":{},\"payload\":\"{}\",\"type\":134217729}}",
                    serde_json::to_string(&event.event).map_err(|e| e.to_string())?,
                    hex::encode(&payload)
                )
                .into_bytes();
                let supplied = hex_bytes(
                    event
                        .preimage
                        .as_deref()
                        .ok_or_else(|| format!("event {index}: v2 preimage missing"))?,
                )?;
                if supplied != canonical {
                    return Err(format!(
                        "event {index}: v2 preimage is not canonical event content"
                    ));
                }
                canonical
            }
            _ => {
                return Err(format!(
                    "event {index}: unsupported event version {}",
                    event.version
                ));
            }
        };
        let digest: [u8; 48] = Sha384::digest(&preimage).into();
        // dstack may omit this redundant runtime digest; the quote authenticates our
        // independently recomputed chain. A supplied digest must still agree.
        if !event.digest.is_empty() && hex_bytes(&event.digest)? != digest {
            return Err(format!(
                "event {index}: digest does not bind its name/payload"
            ));
        }
        rtmr = Sha384::new()
            .chain_update(rtmr)
            .chain_update(digest)
            .finalize()
            .into();
        if event.event == "compose-hash" {
            if compose.is_some() {
                return Err("duplicate compose-hash event".into());
            }
            compose = Some(
                payload
                    .try_into()
                    .map_err(|_| "compose-hash payload must be 32 bytes")?,
            );
        }
    }
    Ok((rtmr, compose.ok_or("compose-hash event missing")?))
}

pub fn bindings(identity: &Identity, report: &TDReport10, expected: Option<Hex32>) -> Vec<Check> {
    let replayed = replay(&identity.event_log);
    let rtmr_check = Check::new(
        "rtmr3_replay",
        match &replayed {
            Ok((rtmr, _)) if *rtmr == report.rt_mr3 => {
                Ok("event digests and RTMR3 match quote".into())
            }
            Ok(_) => Err("event log replay differs from quoted RTMR3".into()),
            Err(e) => Err(e.clone()),
        },
    );
    let compose_check = Check::new(
        "compose_hash",
        match replayed {
            Ok((_, compose))
                if identity.compose_hash == Some(Hex32(compose))
                    && expected.is_none_or(|value| value.0 == compose) =>
            {
                Ok(format!(
                    "0x{}{}",
                    hex::encode(compose),
                    if expected.is_some() {
                        " (matches expected compose)"
                    } else {
                        " (no owner expectation supplied; measurement is not approval)"
                    }
                ))
            }
            Ok(_) => Err("event compose hash differs from served or expected compose hash".into()),
            Err(e) => Err(format!("cannot authenticate compose hash: {e}")),
        },
    );
    let prefix = b"constitution.v1/key";
    let preimage = &identity.report_data_preimage.0;
    let keys_bound = preimage.len() == prefix.len() + 96
        && preimage.starts_with(prefix)
        && preimage[prefix.len()..prefix.len() + 32] == identity.verifier_pubkey.0
        && preimage[prefix.len() + 32..prefix.len() + 64] == identity.transport_pubkey.0;
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(&Sha256::digest(preimage));
    let data_check = Check::new(
        "report_data",
        if keys_bound && data == report.report_data {
            Ok(
                "domain, Ed25519 key, X25519 key, 32-byte boot nonce and zero padding match quote"
                    .into(),
            )
        } else {
            Err("report_data/preimage does not bind the served keys and boot nonce".into())
        },
    );
    let release: [u8; 32] = Sha256::new()
        .chain_update(b"constitution.v1/release")
        .chain_update(report.mr_td)
        .chain_update(report.rt_mr0)
        .chain_update(report.rt_mr1)
        .chain_update(report.rt_mr2)
        .chain_update(identity.compose_hash.map_or([0; 32], |value| value.0))
        .finalize()
        .into();
    let release_check = Check::new(
        "release_hash",
        if identity.compose_hash.is_some() && identity.release_hash == Some(Hex32(release)) {
            Ok(format!("0x{}", hex::encode(release)))
        } else {
            Err("release hash does not match quoted MRTD/RTMR0-2 and compose hash".into())
        },
    );
    vec![rtmr_check, compose_check, data_check, release_check]
}

pub fn evaluate(
    identity: &Identity,
    collateral: &QuoteCollateralV3,
    now: u64,
    expected: Option<Hex32>,
) -> Vec<Check> {
    let raw = match hex_bytes(&identity.quote_hex) {
        Ok(raw) => raw,
        Err(e) => return unavailable(e),
    };
    let (dcap, tcb, report) = verify_dcap(&raw, collateral, now);
    let mut checks = vec![
        Check::new("identity", Ok("identity document decoded".into())),
        dcap,
        tcb,
    ];
    match report {
        Some(report) => checks.extend(bindings(identity, &report, expected)),
        None => checks.extend(CHECKS[3..].iter().map(|name| {
            Check::new(
                name,
                Err("unavailable: authenticated TDX report required".into()),
            )
        })),
    }
    checks
}

fn unavailable(cause: String) -> Vec<Check> {
    CHECKS
        .iter()
        .map(|name| Check::new(name, Err(cause.clone())))
        .collect()
}

fn observe(url: &str, pccs: &str, expected: Option<Hex32>) -> Result<Vec<Check>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let identity: Identity = client
        .get(format!("{}/v1/identity", url.trim_end_matches('/')))
        .send()
        .map_err(|e| {
            format!(
                "identity fetch: {}",
                crate::rpc::safe_error(url, e.without_url())
            )
        })?
        .error_for_status()
        .map_err(|e| {
            format!(
                "identity HTTP: {}",
                crate::rpc::safe_error(url, e.without_url())
            )
        })?
        .json()
        .map_err(|e| {
            format!(
                "identity JSON: {}",
                crate::rpc::safe_error(url, e.without_url())
            )
        })?;
    let raw = hex_bytes(&identity.quote_hex)?;
    Quote::parse(&raw)
        .map_err(|e| format!("identity is NOT ATTESTED or quote is invalid: {e:#}"))?;
    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let collateral = runtime.block_on(async {
        let client =
            dcap_qvl::collateral::CollateralClient::with_default_http(pccs).map_err(|e| {
                format!(
                    "PCCS client: {}",
                    crate::rpc::safe_error(pccs, format!("{e:#}"))
                )
            })?;
        client.fetch(&raw).await.map_err(|e| {
            format!(
                "PCCS collateral fetch: {}; check --pccs-url/PCCS_URL and connectivity",
                crate::rpc::safe_error(pccs, format!("{e:#}"))
            )
        })
    })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    Ok(evaluate(&identity, &collateral, now, expected))
}

pub fn appraise(url: &str, pccs: &str, expected: Option<Hex32>, json: bool) -> Result<(), Failure> {
    let checks = observe(url, pccs, expected).unwrap_or_else(unavailable);
    let ok = checks.iter().all(|check| check.ok);
    if json {
        println!(
            "{}",
            serde_json::json!({"schema":"constitution.appraisal/1", "ok":ok, "checks":checks})
        );
    } else {
        for check in &checks {
            println!(
                "{} {}: {}",
                if check.ok { "PASS" } else { "FAIL" },
                check.name,
                check.detail
            );
            if let Some(next) = check.next {
                println!("next: {next}");
            }
        }
    }
    if ok {
        Ok(())
    } else {
        Err(Failure::new("verifier appraisal failed", RECOVER))
    }
}
