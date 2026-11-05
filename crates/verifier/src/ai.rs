//! F6: untrusted candidates can veto only after authenticated local EVM replay.
//! Protocol fakes in tests are not AI or TEE evidence.
use crate::{
    replay::{self, Call, Corpus},
    sha256,
};
use constitution_types::{
    AiReview, Hex32, HexBytes, Verdict,
    manifest::{AiCandidate, AiEvidence, StageStatus},
};
use revm::primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use std::{io::Read, time::Duration};

pub const MODEL: &str = "z-ai/glm-5.3-flash";
pub const MAX_CANDIDATE_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 96 * 1024;
pub const MAX_STEPS: usize = 16;

#[derive(Debug, Serialize)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}
#[derive(Debug, Serialize)]
pub struct ReviewRequest {
    pub files: Vec<SourceFile>,
    pub rules: Vec<String>,
    pub nonce: String,
    pub max_candidates: u32,
    pub timeout_s: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub rule: String,
    pub rationale: String,
    pub steps: Vec<Step>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub from: String,
    pub call: String,
    pub args: Vec<String>,
    pub value: String,
    #[serde(default = "proxy")]
    pub to: String,
}
fn proxy() -> String {
    "proxy".into()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Candidates {
    candidates: Vec<Candidate>,
}
fn validate(candidates: &[Candidate], max: u32) -> Result<(), &'static str> {
    if !(1..=16).contains(&max) || candidates.len() > max as usize {
        return Err("candidate count exceeds bound");
    }
    for candidate in candidates {
        if candidate.rule != "R2"
            || candidate.rationale.is_empty()
            || candidate.rationale.len() > 2048
            || candidate.steps.is_empty()
            || candidate.steps.len() > MAX_STEPS
        {
            return Err("invalid candidate schema");
        }
        for step in &candidate.steps {
            let decimal = step.args.first().is_some_and(|a| {
                a.parse::<u128>().is_ok()
                    && (a == "0" || (!a.starts_with('0') && a.bytes().all(|c| c.is_ascii_digit())))
            });
            if step.to != "proxy"
                || step.value != "0"
                || step.args.len() != 1
                || !decimal
                || !matches!(step.call.as_str(), "deposit(uint256)" | "withdraw(uint256)")
                || !(matches!(step.from.as_str(), "attacker" | "victim")
                    || (step.from.starts_with("0x")
                        && step.from.len() == 42
                        && step.from.parse::<Address>().is_ok()))
            {
                return Err("invalid candidate step");
            }
        }
    }
    Ok(())
}
pub fn parse_candidates(bytes: &[u8], max: u32) -> Result<Vec<Candidate>, &'static str> {
    if bytes.len() > MAX_CANDIDATE_BYTES {
        return Err("candidate JSON exceeds byte bound");
    }
    let parsed: Candidates = serde_json::from_slice(bytes).map_err(|_| "invalid candidate JSON")?;
    validate(&parsed.candidates, max)?;
    Ok(parsed.candidates)
}

/// The only outbound AI connection is fixed IPv4 loopback; never accepts a URL/key.
pub trait Transport: Send + Sync {
    fn review(&self, request: Vec<u8>, timeout_s: u32) -> Result<Vec<u8>, &'static str>;
}
pub struct LocalSidecar;
impl Transport for LocalSidecar {
    fn review(&self, request: Vec<u8>, timeout_s: u32) -> Result<Vec<u8>, &'static str> {
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(u64::from(timeout_s) + 2))
            .build()
            .map_err(|_| "AI client unavailable")?;
        let response = client
            .post("http://127.0.0.1:8788/review")
            .header("content-type", "application/json")
            .body(request)
            .send()
            .map_err(|_| "AI sidecar unavailable or timed out")?;
        if !response.status().is_success() {
            return Err("AI sidecar HTTP failure");
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_RESPONSE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "AI sidecar response unreadable")?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err("AI sidecar response exceeds bound");
        }
        Ok(bytes)
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Response {
    status: String,
    reason: String,
    provider: String,
    candidates: Vec<Candidate>,
    signature_kind: String,
    completion_id: String,
    request_sha256: String,
    response_sha256: String,
    usage: serde_json::Value,
    cost_usd: serde_json::Value,
    #[serde(default)]
    receipt: Option<Receipt>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Receipt {
    kind: String,
    signing_algo: String,
    signing_address: String,
    signed_text: String,
    signature: String,
}
impl Receipt {
    fn valid(&self) -> bool {
        let mut parts = self.signed_text.split(':');
        let mut signature = [0; 64];
        self.kind == "provider_tee"
            && self.signing_algo == "ed25519"
            && hex_hash(&self.signing_address).is_some()
            && hex::decode_to_slice(&self.signature, &mut signature).is_ok()
            && parts.next() == Some(MODEL)
            && parts.next().and_then(hex_hash).is_some()
            && parts.next().and_then(hex_hash).is_some()
            && parts.next().is_none()
    }
}
pub struct Report {
    pub review: AiReview,
    pub status: StageStatus,
    pub evidence: AiEvidence,
    pub detail: String,
}
impl Report {
    pub fn incomplete(detail: &'static str) -> Self {
        Self {
            review: AiReview::NotRun,
            status: StageStatus::Incomplete,
            evidence: AiEvidence {
                model: MODEL.into(),
                completion_id: String::new(),
                request_sha256: Hex32([0; 32]),
                response_sha256: Hex32([0; 32]),
                signature_kind: String::new(),
                signature: HexBytes(vec![]),
                preflight_digest: Hex32([0; 32]),
                candidates: vec![],
            },
            detail: detail.into(),
        }
    }
    pub fn verdict(&self) -> Option<Verdict> {
        matches!(self.review, AiReview::Vetoed { .. }).then_some(Verdict::Reject {
            rule_id: 2,
            reason_code: 22,
        })
    }
}
fn hex_hash(s: &str) -> Option<Hex32> {
    let mut bytes = [0; 32];
    hex::decode_to_slice(s, &mut bytes).ok()?;
    Some(Hex32(bytes))
}
pub fn review(
    transport: &dyn Transport,
    request: &ReviewRequest,
    mut reproduce: impl FnMut(&Candidate) -> Result<bool, String>,
) -> Report {
    let mut report = Report::incomplete("AI incomplete; provider near; no verified candidates");
    let Ok(bytes) = serde_json::to_vec(request) else {
        return report;
    };
    report.evidence.request_sha256 = Hex32(sha256(&bytes));
    if !(1..=16).contains(&request.max_candidates) || !(1..=300).contains(&request.timeout_s) {
        return report;
    }
    let response_bytes = match transport.review(bytes, request.timeout_s) {
        Ok(bytes) if bytes.len() <= MAX_RESPONSE_BYTES => bytes,
        _ => return report,
    };
    report.evidence.preflight_digest = Hex32(sha256(&response_bytes));
    let Ok(response) = serde_json::from_slice::<Response>(&response_bytes) else {
        return report;
    };
    // Only publish allowlisted metadata, never arbitrary untrusted text/errors.
    if response.completion_id.len() <= 256
        && response
            .completion_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        report.evidence.completion_id = response.completion_id;
    }
    if matches!(
        response.signature_kind.as_str(),
        "provider_tee" | "gateway" | "gateway_tee"
    ) {
        report.evidence.signature_kind = response.signature_kind.clone();
    }
    if let Some(hash) = hex_hash(&response.request_sha256) {
        report.evidence.request_sha256 = hash;
    }
    if let Some(hash) = hex_hash(&response.response_sha256) {
        report.evidence.response_sha256 = hash;
    }
    // Usage/cost are committed in preflight_digest, never interpreted as safety evidence.
    let _ = (&response.usage, &response.cost_usd, &response.reason);
    if response.status != "complete"
        || response.provider != "near"
        || response.signature_kind != "provider_tee"
        || report.evidence.completion_id.is_empty()
        || hex_hash(&response.request_sha256).is_none()
        || hex_hash(&response.response_sha256).is_none()
        || validate(&response.candidates, request.max_candidates).is_err()
        || serde_json::to_vec(&response.candidates).map_or(true, |b| b.len() > MAX_CANDIDATE_BYTES)
    {
        return report;
    }
    let Some(receipt) = response.receipt.filter(Receipt::valid) else {
        return report;
    };
    report.evidence.signature = HexBytes(serde_json::to_vec(&receipt).expect("receipt serialize"));
    let evidence_hash = sha256(&response_bytes);
    let mut incomplete = false;
    let mut counterexample = None;
    for candidate in &response.candidates {
        let hash = sha256(&serde_json::to_vec(candidate).expect("candidate serialize"));
        let reproduced = match reproduce(candidate) {
            Ok(value) => value,
            Err(_) => {
                incomplete = true;
                false
            }
        };
        report.evidence.candidates.push(AiCandidate {
            sha256: Hex32(hash),
            reproduced,
        });
        if reproduced && counterexample.is_none() {
            counterexample = Some(hash);
        }
    }
    let count = report.evidence.candidates.len();
    if let Some(counterexample_hash) = counterexample {
        report.review = AiReview::Vetoed {
            evidence_hash,
            counterexample_hash,
        };
        report.status = StageStatus::Fail;
        report.detail = format!(
            "AI veto reproduced; provider near; signature provider_tee; candidates {count}; reason 22"
        );
    } else if !incomplete {
        report.review = AiReview::Clean {
            evidence_hash,
            signature_kind: 1,
        };
        report.status = StageStatus::Pass;
        report.detail = format!(
            "AI clean; provider near; signature provider_tee; candidates {count}; no reproduced violation"
        );
    }
    report
}

pub fn candidate_corpus(
    candidate: &Candidate,
    actor: Address,
    confirmations: u32,
) -> Result<Corpus, String> {
    use alloy_sol_types::SolCall;
    validate(std::slice::from_ref(candidate), 1)?;
    let calls = candidate
        .steps
        .iter()
        .map(|step| {
            let from = match step.from.as_str() {
                "attacker" => actor,
                "victim" => Address::with_last_byte(2),
                other => other.parse().map_err(|_| "invalid candidate caller")?,
            };
            let amount = U256::from(step.args[0].parse::<u128>().map_err(|_| "invalid amount")?);
            let data = match step.call.as_str() {
                "deposit(uint256)" => vault::depositCall { amount }.abi_encode(),
                "withdraw(uint256)" => vault::withdrawCall { amount }.abi_encode(),
                _ => return Err("unsupported candidate call"),
            };
            Ok(Call {
                from,
                to: "proxy".into(),
                value: "0".into(),
                data: data.into(),
                label: "AI candidate (untrusted)".into(),
            })
        })
        .collect::<Result<Vec<_>, &str>>()?;
    Ok(Corpus {
        calls,
        max_txs: MAX_STEPS as u32,
        confirmations,
        sha256: sha256(&serde_json::to_vec(candidate).map_err(|_| "candidate encoding")?),
    })
}

/// Candidate-specific discovery additionally includes the R2 predicate prestate.
/// Tracer values are hints only; R5 authenticates every replay input against this block.
#[allow(clippy::too_many_arguments)]
pub fn fetch_candidate_witness(
    rpc: &str,
    proof_rpc: &str,
    block: replay::EvidenceBlock,
    corpus: &Corpus,
    proxy: Address,
    new_impl: Address,
    codehash: revm::primitives::B256,
    init: &[u8],
    actor: Address,
    deadline: std::time::Instant,
) -> Result<replay::Witness, replay::Incomplete> {
    replay::fetch_ai_witness(
        rpc, proof_rpc, block, corpus, proxy, new_impl, codehash, init, actor, deadline,
    )
}
alloy_sol_types::sol! {
    interface vault {
        function deposit(uint256 amount);
        function withdraw(uint256 amount);
    }
}
/// Pure revm replay: no network, process, filesystem or state defaults.
pub fn reproduce(
    witness: &replay::Witness,
    corpus: &Corpus,
    current: &[u8],
    new: &[u8],
    init: &[u8],
    initializer_sender: Address,
) -> Result<bool, String> {
    use alloy_sol_types::SolCall;
    use replay::vault_abi;
    if corpus.calls.is_empty() || corpus.calls.len() > MAX_STEPS {
        return Err("candidate exceeds step bound".into());
    }
    let mut session =
        replay::CandidateReplay::new(witness, corpus, current, new, init, initializer_sender)
            .map_err(|e| e.to_string())?;
    let sender = initializer_sender;
    // VaultV1 has no token() getter. Bind R2's token to the proof-authenticated
    // ERC-7201 prestate, not a candidate-controlled address or RPC return value.
    let token_slot = replay::vault_token_slot();
    let token_word = witness
        .accounts
        .get(&witness.proxy)
        .and_then(|account| account.storage.get(&token_slot))
        .ok_or("missing witnessed vault token")?;
    if *token_word == U256::ZERO || *token_word >> 160 != U256::ZERO {
        return Err("invalid witnessed vault token".into());
    }
    let token = Address::from_slice(&token_word.to_be_bytes::<32>()[12..]);
    let solvent = |session: &replay::CandidateReplay| -> Result<bool, String> {
        let total_data = session
            .query(
                sender,
                witness.proxy,
                vault_abi::totalDepositsCall {}.abi_encode().into(),
            )
            .map_err(|e| e.to_string())?;
        let total = vault_abi::totalDepositsCall::abi_decode_returns(&total_data)
            .map_err(|_| "invalid total result")?;
        let balance_data = session
            .query(
                sender,
                token,
                vault_abi::balanceOfCall {
                    account: witness.proxy,
                }
                .abi_encode()
                .into(),
            )
            .map_err(|e| e.to_string())?;
        let balance = vault_abi::balanceOfCall::abi_decode_returns(&balance_data)
            .map_err(|_| "invalid token balance result")?;
        Ok(total <= balance)
    };
    if !solvent(&session)? {
        return Err("evidence prestate already insolvent".into());
    }
    for call in &corpus.calls {
        if call.to != "proxy" || call.value != "0" || call.data.len() > 1024 {
            return Err("invalid replay call".into());
        }
        session
            .call_proxy(call.from, call.data.clone())
            .map_err(|e| e.to_string())?;
        if !solvent(&session)? {
            return Ok(true);
        }
    }
    Ok(false)
}
