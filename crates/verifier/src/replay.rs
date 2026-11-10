//! R5: authenticated, bounded, stateful differential replay (see rules/r5/README.md).
use crate::{keccak, sha256};
use constitution_types::manifest::StageStatus;
use revm::{
    Context, Database, ExecuteEvm, MainBuilder, MainContext,
    bytecode::Bytecode,
    context::{BlockEnv, TxEnv},
    context_interface::result::ExecutionResult,
    database_interface::DBErrorMarker,
    primitives::{Address, B256, Bytes, Log, TxKind, U256, hardfork::SpecId},
    state::{AccountInfo, EvmState},
};
use rlp::{Rlp, RlpStream};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    time::{Duration, Instant},
};

const GAS_LIMIT: u64 = 3_000_000;
const MAX_CORPUS: usize = 64;
const IMPL: U256 = revm::primitives::uint!(
    0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc_U256
);
const CAP: U256 = revm::primitives::uint!(
    0x199cdf1a4abaec4823e24931ac666120eb2410beefcb1fe17a5d0d2c4f416803_U256
);
const INITIALIZABLE: U256 = revm::primitives::uint!(
    0xf0c57e16840df040f15088dc2f81fe391c3923bec73e23a9662efc9c229c6a00_U256
);
fn hash(data: &[u8]) -> B256 {
    B256::from(keccak(data))
}

/// Why a witness or replay could not complete. Only an authenticated contradiction may
/// become a signed rejection; everything else is a verifier-side or provider-side fault.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncompleteKind {
    /// Provider lag, null results, transport glitches, unverifiable provider data:
    /// nothing here authenticates a fact about the proposal.
    Environment,
    /// A fact established from authenticated data (verified header, valid proofs, pure
    /// deterministic replay) that the proposal cannot satisfy.
    Contradiction,
}

/// Closed, non-secret categories for operator-visible environmental failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IncompleteReason {
    RpcTransport,
    Timeout,
    RpcRateLimited,
    RpcHttp4xx,
    RpcHttp5xx,
    RpcHttpOther,
    RpcMethodUnsupported,
    ProofWindowExceeded,
    RpcError,
    MalformedResponse,
    HeaderMismatch,
    StateProofMismatch,
    CodeMismatch,
    InsufficientConfirmations,
    SenderCodeUnexpected,
    SenderNonceUnexpected,
    SenderInsufficientBalance,
    EvmTransactionInvalid,
    EvmHeaderInvalid,
    PreconditionUnmet,
    ExpectedOutcomeMismatch,
    ReplayIncomplete,
}
impl IncompleteReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::RpcTransport => "rpc_transport",
            Self::Timeout => "timeout",
            Self::RpcRateLimited => "rpc_rate_limited",
            Self::RpcHttp4xx => "rpc_http_4xx",
            Self::RpcHttp5xx => "rpc_http_5xx",
            Self::RpcHttpOther => "rpc_http_other",
            Self::RpcMethodUnsupported => "rpc_method_unsupported",
            Self::ProofWindowExceeded => "proof_window_exceeded",
            Self::RpcError => "rpc_error",
            Self::MalformedResponse => "malformed_response",
            Self::HeaderMismatch => "header_mismatch",
            Self::StateProofMismatch => "state_proof_mismatch",
            Self::CodeMismatch => "code_mismatch",
            Self::InsufficientConfirmations => "insufficient_confirmations",
            Self::SenderCodeUnexpected => "sender_code_unexpected",
            Self::SenderNonceUnexpected => "sender_nonce_unexpected",
            Self::SenderInsufficientBalance => "sender_insufficient_balance",
            Self::EvmTransactionInvalid => "evm_transaction_invalid",
            Self::EvmHeaderInvalid => "evm_header_invalid",
            Self::PreconditionUnmet => "precondition_unmet",
            Self::ExpectedOutcomeMismatch => "expected_outcome_mismatch",
            Self::ReplayIncomplete => "replay_incomplete",
        }
    }
}
/// A diagnostic never includes an RPC URL or response body (URLs may contain keys).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Incomplete {
    pub message: String,
    pub kind: IncompleteKind,
    pub reason: IncompleteReason,
    pub rpc_transport: bool,
}
impl fmt::Display for Incomplete {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for Incomplete {}
impl DBErrorMarker for Incomplete {}
/// Environment by default: the safe reading of anything not explicitly authenticated.
fn incomplete(message: &str) -> Incomplete {
    Incomplete {
        message: message.into(),
        kind: IncompleteKind::Environment,
        reason: match message {
            "run deadline exceeded" => IncompleteReason::Timeout,
            "evidence block mismatch"
            | "execution header hash mismatch"
            | "evidence block reorged during fetch"
            | "proof evidence block reorged during fetch" => IncompleteReason::HeaderMismatch,
            "account does not match state-root proof"
            | "storage value does not match proof"
            | "absent account has nonempty state"
            | "absent storage is nonzero" => IncompleteReason::StateProofMismatch,
            "proof code hash mismatch"
            | "candidate code changed during witness fetch"
            | "candidate runtime does not match proposal code hash" => {
                IncompleteReason::CodeMismatch
            }
            "insufficient confirmations" => IncompleteReason::InsufficientConfirmations,
            "corpus precondition unmet" => IncompleteReason::PreconditionUnmet,
            "old call contradicts expected outcome" => IncompleteReason::ExpectedOutcomeMismatch,
            _ => IncompleteReason::ReplayIncomplete,
        },
        rpc_transport: false,
    }
}
fn contradiction(message: &str) -> Incomplete {
    Incomplete {
        message: message.into(),
        kind: IncompleteKind::Contradiction,
        reason: IncompleteReason::ReplayIncomplete,
        rpc_transport: false,
    }
}
fn require(condition: bool, message: &str) -> Result<(), Incomplete> {
    if condition {
        Ok(())
    } else {
        Err(incomplete(message))
    }
}
fn require_contradiction(condition: bool, message: &str) -> Result<(), Incomplete> {
    if condition {
        Ok(())
    } else {
        Err(contradiction(message))
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Expect {
    Success,
    Revert,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preconditions {
    pub actor: Address,
    pub min_token_balance: String,
    pub min_allowance: String,
    pub min_vault_balance: String,
    pub max_vault_balance: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CorpusFile {
    preconditions: Preconditions,
    calls: Vec<Call>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Call {
    pub from: Address,
    pub to: String,
    /// Decimal wei; constrained to u128 at the input boundary.
    pub value: String,
    pub data: Bytes,
    pub expect: Expect,
    pub label: String,
}
#[derive(Clone, Debug)]
pub struct Corpus {
    pub preconditions: Preconditions,
    pub calls: Vec<Call>,
    pub max_txs: u32,
    pub confirmations: u32,
    pub sha256: [u8; 32],
}
impl Corpus {
    pub fn from_json(bytes: &[u8], max_txs: u32, confirmations: u32) -> Result<Self, Incomplete> {
        require(bytes.len() <= 128 * 1024, "corpus exceeds byte bound")?;
        let file: CorpusFile =
            serde_json::from_slice(bytes).map_err(|_| incomplete("invalid corpus JSON"))?;
        let corpus = Self {
            preconditions: file.preconditions,
            calls: file.calls,
            max_txs,
            confirmations,
            sha256: sha256(bytes),
        };
        corpus.validate()?;
        Ok(corpus)
    }
    fn validate(&self) -> Result<(), Incomplete> {
        require(
            self.max_txs > 0
                && self.max_txs as usize <= MAX_CORPUS
                && !self.calls.is_empty()
                && self.calls.len() <= self.max_txs as usize,
            "corpus exceeds transaction bound or is empty",
        )?;
        let p = &self.preconditions;
        let min_vault = decimal_amount(&p.min_vault_balance)?;
        let max_vault = decimal_amount(&p.max_vault_balance)?;
        decimal_amount(&p.min_token_balance)?;
        decimal_amount(&p.min_allowance)?;
        require(min_vault <= max_vault, "invalid vault balance bounds")?;
        for call in &self.calls {
            require(
                call.to == "proxy"
                    && call.data.len() <= 1024
                    && !call.label.is_empty()
                    && call.label.len() <= 128,
                "invalid corpus entry",
            )?;
            call.value
                .parse::<u128>()
                .map_err(|_| incomplete("invalid call value"))?;
        }
        Ok(())
    }
}

fn decimal_amount(value: &str) -> Result<U256, Incomplete> {
    require(
        !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()),
        "invalid precondition amount",
    )?;
    U256::from_str_radix(value, 10).map_err(|_| incomplete("invalid precondition amount"))
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct EvidenceBlock {
    pub number: u64,
    pub hash: B256,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountWitness {
    pub balance: U256,
    pub nonce: u64,
    pub code: Vec<u8>,
    pub storage: BTreeMap<U256, U256>,
    pub account_proof: Vec<Vec<u8>>,
    pub storage_root: B256,
    pub storage_proofs: BTreeMap<U256, Vec<Vec<u8>>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Witness {
    pub block: EvidenceBlock,
    /// Original execution header; its RLP hash authenticates stateRoot and the environment.
    pub header: Value,
    pub state_root: B256,
    pub environment: BlockEnv,
    pub proxy: Address,
    pub new_impl: Address,
    pub proxy_admin: Address,
    pub executor: Address,
    pub signer: Address,
    pub accounts: BTreeMap<Address, AccountWitness>,
}

fn text(v: &Value) -> Result<&str, Incomplete> {
    v.as_str().ok_or_else(|| incomplete("missing RPC string"))
}
fn quantity(v: &Value) -> Result<U256, Incomplete> {
    text(v)?
        .parse()
        .map_err(|_| incomplete("invalid RPC quantity"))
}
fn number(v: &Value) -> Result<u64, Incomplete> {
    quantity(v)?
        .try_into()
        .map_err(|_| incomplete("RPC integer overflow"))
}
fn fixed(v: &Value) -> Result<B256, Incomplete> {
    text(v)?.parse().map_err(|_| incomplete("invalid RPC hash"))
}
fn address(v: &Value) -> Result<Address, Incomplete> {
    text(v)?
        .parse()
        .map_err(|_| incomplete("invalid RPC address"))
}
fn bytes(v: &Value) -> Result<Vec<u8>, Incomplete> {
    hex::decode(
        text(v)?
            .strip_prefix("0x")
            .ok_or_else(|| incomplete("missing hex prefix"))?,
    )
    .map_err(|_| incomplete("invalid RPC bytes"))
}
fn proof(v: &Value) -> Result<Vec<Vec<u8>>, Incomplete> {
    v.as_array()
        .ok_or_else(|| incomplete("missing proof array"))?
        .iter()
        .map(bytes)
        .collect()
}

/// Header field order follows Ethereum's execution header (EIPs 1559/4895/4844/4788/7685).
/// Optional suffix holes are encoded as empty strings, as in geth's RLP header encoding.
pub(crate) fn header_hash(header: &Value) -> Result<B256, Incomplete> {
    const FIELDS: [(&str, usize); 23] = [
        ("parentHash", 32),
        ("sha3Uncles", 32),
        ("miner", 20),
        ("stateRoot", 32),
        ("transactionsRoot", 32),
        ("receiptsRoot", 32),
        ("logsBloom", 256),
        ("difficulty", 0),
        ("number", 0),
        ("gasLimit", 0),
        ("gasUsed", 0),
        ("timestamp", 0),
        ("extraData", usize::MAX),
        ("mixHash", 32),
        ("nonce", 8),
        ("baseFeePerGas", 0),
        ("withdrawalsRoot", 32),
        ("blobGasUsed", 0),
        ("excessBlobGas", 0),
        ("parentBeaconBlockRoot", 32),
        ("requestsHash", 32),
        ("blockAccessListHash", 32),
        ("slotNumber", 0),
    ];
    let count = FIELDS
        .iter()
        .rposition(|(name, _)| !header[*name].is_null())
        .map_or(0, |last| last + 1);
    require(count >= 16, "incomplete execution header")?;
    let mut stream = RlpStream::new_list(count);
    for (index, (name, width)) in FIELDS[..count].iter().enumerate() {
        let value = &header[*name];
        if value.is_null() {
            require(index >= 16, "missing required header field")?;
            stream.append_empty_data();
        } else if *width == 0 {
            stream.append_raw(&rlp_quantity(quantity(value)?), 1);
        } else {
            let data = bytes(value)?;
            require(
                if *width == usize::MAX {
                    data.len() <= 1024
                } else {
                    data.len() == *width
                },
                "invalid execution header field width",
            )?;
            stream.append(&data);
        }
    }
    Ok(hash(&stream.out()))
}
fn verified_header(header: &Value, block: EvidenceBlock) -> Result<(B256, BlockEnv), Incomplete> {
    require(
        fixed(&header["hash"])? == block.hash && number(&header["number"])? == block.number,
        "evidence block mismatch",
    )?;
    require(
        header_hash(header)? == block.hash,
        "execution header hash mismatch",
    )?;
    let mut environment = BlockEnv {
        number: U256::from(block.number),
        timestamp: quantity(&header["timestamp"])?,
        beneficiary: address(&header["miner"])?,
        gas_limit: number(&header["gasLimit"])?,
        basefee: number(&header["baseFeePerGas"])?,
        difficulty: quantity(&header["difficulty"])?,
        prevrandao: Some(fixed(&header["mixHash"])?),
        ..BlockEnv::default()
    };
    environment.set_blob_excess_gas_and_price(number(&header["excessBlobGas"])?, 5_007_716);
    require(
        environment.gas_limit >= GAS_LIMIT,
        "block gas limit below replay bound",
    )?;
    Ok((fixed(&header["stateRoot"])?, environment))
}

/// One archive RPC session bound by the run's wall-clock deadline.
struct Rpc {
    client: reqwest::blocking::Client,
    url: String,
    deadline: Instant,
}
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
const RPC_RETRY_PAUSE: Duration = Duration::from_millis(250);
const RPC_RATE_LIMIT_ATTEMPTS: u32 = 7;
const RPC_RATE_LIMIT_MAX_PAUSE: Duration = Duration::from_secs(8);
impl Rpc {
    fn new(url: &str, deadline: Instant) -> Result<Self, Incomplete> {
        Ok(Self {
            client: reqwest::blocking::Client::builder()
                .timeout(RPC_TIMEOUT)
                .build()
                .map_err(|_| incomplete("RPC client unavailable"))?,
            url: url.into(),
            deadline,
        })
    }
    /// Every read is attempted twice: a lagging or load-balanced provider often serves a
    /// null/error for a fresh block and the correct value moments later. A rate-limited read
    /// backs off exponentially (bounded by the run deadline) because free proof providers
    /// throttle bursts and a proof read is idempotent. A final failure is reported as the
    /// provider's fault, never as a fact about the proposal.
    fn call(&self, method: &str, params: Value) -> Result<Value, Incomplete> {
        let mut pause = RPC_RETRY_PAUSE;
        let mut attempt = 1;
        loop {
            let error = match self.call_once(method, &params) {
                Ok(value) => return Ok(value),
                Err(error) => error,
            };
            let limited = matches!(error.reason, IncompleteReason::RpcRateLimited);
            let attempts = if limited { RPC_RATE_LIMIT_ATTEMPTS } else { 2 };
            if attempt >= attempts || Instant::now() + pause >= self.deadline {
                return Err(error);
            }
            std::thread::sleep(pause);
            if limited {
                pause = (pause * 2).min(RPC_RATE_LIMIT_MAX_PAUSE);
            }
            attempt += 1;
        }
    }
    fn call_once(&self, method: &str, params: &Value) -> Result<Value, Incomplete> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())
            .ok_or_else(|| incomplete("run deadline exceeded"))?;
        let response = self
            .client
            .post(&self.url)
            .timeout(remaining.min(RPC_TIMEOUT))
            .json(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
            .send()
            .map_err(|error| Incomplete {
                message: format!("{method}: transport failure"),
                kind: IncompleteKind::Environment,
                reason: if error.is_timeout() {
                    IncompleteReason::Timeout
                } else {
                    IncompleteReason::RpcTransport
                },
                rpc_transport: true,
            })?;
        let status = response.status();
        if !status.is_success() {
            let reason = if status.as_u16() == 429 {
                IncompleteReason::RpcRateLimited
            } else if status.is_client_error() {
                IncompleteReason::RpcHttp4xx
            } else if status.is_server_error() {
                IncompleteReason::RpcHttp5xx
            } else {
                IncompleteReason::RpcHttpOther
            };
            let mut error = incomplete(&format!("{method}: HTTP {}", status.as_u16()));
            error.reason = reason;
            return Err(error);
        }
        // Bound hostile provider output before JSON allocation; no silent truncation.
        use std::io::Read;
        let mut buffer = Vec::new();
        response
            .take(16 * 1024 * 1024 + 1)
            .read_to_end(&mut buffer)
            .map_err(|_| incomplete(&format!("{method}: unreadable response")))?;
        require(
            buffer.len() <= 16 * 1024 * 1024,
            "RPC response exceeds bound",
        )?;
        let value: Value = serde_json::from_slice(&buffer).map_err(|_| {
            let mut error = incomplete(&format!("{method}: invalid JSON"));
            error.reason = IncompleteReason::MalformedResponse;
            error
        })?;
        if let Some(rpc_error) = value.get("error") {
            let code = rpc_error["code"].as_i64();
            let message = rpc_error["message"]
                .as_str()
                .unwrap_or("")
                .to_ascii_lowercase();
            let mut error = incomplete(&format!("{method}: RPC error"));
            error.reason = if code == Some(-32005)
                || code == Some(-32011)
                || message.contains("rate limit")
                || message.contains("request limit")
                || message.contains("too many requests")
            {
                IncompleteReason::RpcRateLimited
            } else if method == "eth_getProof" && message.contains("proof window") {
                IncompleteReason::ProofWindowExceeded
            } else if code == Some(-32601) {
                IncompleteReason::RpcMethodUnsupported
            } else {
                IncompleteReason::RpcError
            };
            return Err(error);
        }
        value
            .get("result")
            .filter(|v| !v.is_null())
            .cloned()
            .ok_or_else(|| incomplete(&format!("{method}: missing result")))
    }
}

/// Accounts and slots discovered by the RPC; no tracer values are replay inputs.
type Accesses = BTreeMap<Address, BTreeSet<U256>>;

fn collect_accesses(needed: &mut Accesses, trace: &Value) -> Result<(), Incomplete> {
    let accounts = trace
        .as_object()
        .ok_or_else(|| incomplete("prestateTracer missing accounts"))?;
    for (a, state) in accounts {
        let a = a
            .parse()
            .map_err(|_| incomplete("invalid tracer account"))?;
        let slots = needed.entry(a).or_default();
        if let Some(storage) = state.get("storage") {
            for key in storage
                .as_object()
                .ok_or_else(|| incomplete("invalid tracer storage"))?
                .keys()
            {
                slots.insert(key.parse().map_err(|_| incomplete("invalid tracer slot"))?);
            }
        }
    }
    // A repeated oversized candidate discovery is attributed to the proposal.
    require_contradiction(
        needed.len() <= 256 && needed.values().map(BTreeSet::len).sum::<usize>() <= 4096,
        "witness exceeds account/slot bound",
    )
}

/// Tracer poststate is an untrusted discovery hint, not a source of replay values.
/// Replay receives only the original pinned prestate after state-root proof validation.
fn initializer_overrides(
    overrides: &mut Value,
    diff: &Value,
    needed: &mut Accesses,
) -> Result<(), Incomplete> {
    collect_accesses(needed, &diff["pre"])?;
    collect_accesses(needed, &diff["post"])?;
    let pre = diff["pre"]
        .as_object()
        .ok_or_else(|| incomplete("initializer tracer missing prestate"))?;
    let post = diff["post"]
        .as_object()
        .ok_or_else(|| incomplete("initializer tracer missing poststate"))?;
    for (a, state) in post {
        let target = overrides
            .as_object_mut()
            .ok_or_else(|| incomplete("invalid override map"))?
            .entry(a.clone())
            .or_insert_with(|| json!({}));
        for name in ["balance", "code"] {
            if let Some(value) = state.get(name) {
                target[name] = value.clone();
            }
        }
        if let Some(value) = state.get("nonce") {
            let nonce = if let Some(n) = value.as_u64() {
                n
            } else {
                number(value)?
            };
            target["nonce"] = json!(format!("0x{nonce:x}"));
        }
        let mut storage = serde_json::Map::new();
        if let Some(before) = pre.get(a).and_then(|account| account.get("storage")) {
            for key in before
                .as_object()
                .ok_or_else(|| incomplete("invalid initializer prestate storage"))?
                .keys()
            {
                storage.insert(key.clone(), json!(format!("0x{:064x}", 0)));
            }
        }
        if let Some(after) = state.get("storage") {
            storage.extend(
                after
                    .as_object()
                    .ok_or_else(|| incomplete("invalid initializer poststate storage"))?
                    .clone(),
            );
        }
        if !storage.is_empty() {
            target["stateDiff"] = Value::Object(storage);
        }
    }
    Ok(())
}

/// Discover with prestateTracer; authenticate every returned value with EIP-1186 proofs.
/// Synchronous/blocking: integration should use its blocking worker, never an async reactor.
///
/// Candidate code is read at `new_impl` in the pinned block and must match the proposal
/// `new_impl_codehash`. Discovery unions current calls, candidate calls with a code
/// override at the current implementation address, and candidate initializer accesses.
/// Initializer poststate only shapes discovery: every replay input is independently
/// fetched with eth_getProof at the original block and checked against its stateRoot.
/// Missing discoveries remain Incomplete during pure replay; they never become zeros.
#[allow(clippy::too_many_arguments)]
pub fn fetch_witness(
    rpc_url: &str,
    proof_rpc_url: &str,
    block: EvidenceBlock,
    corpus: &Corpus,
    proxy: Address,
    new_impl: Address,
    new_impl_codehash: B256,
    new_init_calldata: &[u8],
    deadline: Instant,
) -> Result<Witness, Incomplete> {
    fetch_witness_inner(
        rpc_url,
        proof_rpc_url,
        block,
        corpus,
        proxy,
        new_impl,
        new_impl_codehash,
        new_init_calldata,
        None,
        deadline,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn fetch_ai_witness(
    rpc_url: &str,
    proof_rpc_url: &str,
    block: EvidenceBlock,
    corpus: &Corpus,
    proxy: Address,
    new_impl: Address,
    new_impl_codehash: B256,
    init: &[u8],
    actor: Address,
    deadline: Instant,
) -> Result<Witness, Incomplete> {
    fetch_witness_inner(
        rpc_url,
        proof_rpc_url,
        block,
        corpus,
        proxy,
        new_impl,
        new_impl_codehash,
        init,
        Some(actor),
        deadline,
    )
}

pub(crate) mod vault_abi {
    alloy_sol_types::sol! {
        function totalDeposits() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function allowance(address owner, address spender) external view returns (uint256);
    }
}

mod executor_abi {
    alloy_sol_types::sol! {
        function execute(bytes32 certId, address newImpl, bytes32 newImplCodehash, address expectedCurrentImpl, uint64 deadline, bytes initCalldata) external;
    }
}

fn upgrade_data(candidate: Address, code: &[u8], current: Address, init: &[u8]) -> Bytes {
    use alloy_sol_types::SolCall;
    executor_abi::executeCall {
        certId: B256::ZERO,
        newImpl: candidate,
        newImplCodehash: hash(code),
        expectedCurrentImpl: current,
        deadline: u64::MAX,
        initCalldata: Bytes::copy_from_slice(init),
    }
    .abi_encode()
    .into()
}
pub(crate) fn vault_token_slot() -> U256 {
    use alloy_sol_types::SolValue;
    let namespace = U256::from_be_slice(&crate::keccak(b"constitution.storage.Vault")) - U256::ONE;
    U256::from_be_slice(&crate::keccak(&namespace.abi_encode())) & !U256::from(255)
}

#[allow(clippy::too_many_arguments)]
fn fetch_witness_inner(
    rpc_url: &str,
    proof_rpc_url: &str,
    block: EvidenceBlock,
    corpus: &Corpus,
    proxy: Address,
    new_impl: Address,
    new_impl_codehash: B256,
    new_init_calldata: &[u8],
    ai_actor: Option<Address>,
    deadline: Instant,
) -> Result<Witness, Incomplete> {
    corpus.validate()?;
    require(
        new_init_calldata.len() <= 1024,
        "initializer exceeds byte bound",
    )?;
    let rpc = Rpc::new(rpc_url, deadline)?;
    let proof_rpc = Rpc {
        client: rpc.client.clone(),
        url: proof_rpc_url.to_owned(),
        deadline,
    };
    require(
        number(&rpc.call("eth_chainId", json!([]))?)? == 84532,
        "wrong chain id",
    )?;
    let tag = format!("0x{:x}", block.number);
    let header = rpc.call("eth_getBlockByNumber", json!([tag, false]))?;
    let (state_root, environment) = verified_header(&header, block)?;
    require(
        number(&proof_rpc.call("eth_chainId", json!([]))?)? == 84532,
        "wrong proof chain id",
    )?;
    // Different providers are allowed, different evidence blocks are not.
    let proof_header = proof_rpc.call("eth_getBlockByNumber", json!([tag, false]))?;
    verified_header(&proof_header, block).map_err(|mut error| {
        error.message = format!("proof provider: {}", error.message);
        error
    })?;
    // A second provider node may be a block behind the one that served the header; read the
    // tip once more before concluding it is too shallow. Either outcome is environmental.
    let mut deep_enough = false;
    for attempt in 0..2 {
        let latest = number(&rpc.call("eth_blockNumber", json!([]))?)?;
        deep_enough = latest
            .checked_sub(block.number)
            .is_some_and(|depth| depth >= u64::from(corpus.confirmations));
        if deep_enough || attempt == 1 {
            break;
        }
        std::thread::sleep(RPC_RETRY_PAUSE);
    }
    require(deep_enough, "insufficient confirmations")?;
    // eth_getCode alone is not authenticated by the header, even on repeated reads.
    let new_runtime = bytes(&rpc.call("eth_getCode", json!([new_impl, tag]))?)?;
    require(
        !new_runtime.is_empty()
            && new_runtime.len() <= 24_576
            && hash(&new_runtime) == new_impl_codehash,
        "candidate runtime does not match proposal code hash",
    )?;
    let implementation_word = quantity(&rpc.call(
        "eth_getStorageAt",
        json!([proxy, format!("0x{IMPL:064x}"), tag]),
    )?)?;
    require(
        implementation_word != U256::ZERO && implementation_word >> 160 == U256::ZERO,
        "invalid current implementation address",
    )?;
    let implementation = Address::from_slice(&implementation_word.to_be_bytes::<32>()[12..]);
    // OZ transparent proxies create their ProxyAdmin at CREATE nonce one. These
    // RPC reads are discovery hints; the corresponding slots are proof-checked below.
    let mut create = vec![0xd6, 0x94];
    create.extend_from_slice(proxy.as_slice());
    create.push(1);
    let proxy_admin = Address::from_slice(&keccak(&create)[12..]);
    let owner_word = quantity(&rpc.call("eth_getStorageAt", json!([proxy_admin, "0x0", tag]))?)?;
    require(
        owner_word != U256::ZERO && owner_word >> 160 == U256::ZERO,
        "invalid proxy admin owner",
    )?;
    let executor = Address::from_slice(&owner_word.to_be_bytes::<32>()[12..]);
    let signer_word = quantity(&rpc.call("eth_getStorageAt", json!([executor, "0x0", tag]))?)?;
    require(
        signer_word != U256::ZERO && signer_word >> 160 == U256::ZERO,
        "invalid executor signer",
    )?;
    let signer = Address::from_slice(&signer_word.to_be_bytes::<32>()[12..]);
    let mut needed = Accesses::new();
    needed.entry(environment.beneficiary).or_default();
    needed.entry(new_impl).or_default();
    needed.entry(implementation).or_default();
    needed.entry(proxy_admin).or_default().insert(U256::ZERO);
    needed.entry(executor).or_default().insert(U256::ZERO);
    needed.entry(signer).or_default();
    if let Some(actor) = ai_actor {
        needed.entry(actor).or_default();
    }
    let mut overrides = json!({});
    let data = upgrade_data(new_impl, &new_runtime, implementation, new_init_calldata);
    let request = json!({"from":signer, "to":executor, "value":"0x0", "data":data, "gas":format!("0x{GAS_LIMIT:x}")});
    let trace = rpc.call(
        "debug_traceCall",
        json!([request, tag, {"tracer":"prestateTracer"}]),
    )?;
    collect_accesses(&mut needed, &trace)?;
    let diff = rpc.call(
        "debug_traceCall",
        json!([request, tag, {"tracer":"prestateTracer", "tracerConfig":{"diffMode":true}}]),
    )?;
    initializer_overrides(&mut overrides, &diff, &mut needed)?;
    for call in &corpus.calls {
        needed.entry(call.from).or_default();
        let request = json!({"from":call.from, "to":proxy, "value":format!("0x{:x}", call.value.parse::<u128>().map_err(|_| incomplete("invalid value"))?), "data":call.data, "gas":format!("0x{GAS_LIMIT:x}")});
        for options in [
            json!({"tracer":"prestateTracer"}),
            json!({"tracer":"prestateTracer", "stateOverrides":overrides}),
        ] {
            let trace = rpc.call("debug_traceCall", json!([request, tag, options]))?;
            require(
                trace
                    .as_object()
                    .is_some_and(|accounts| !accounts.is_empty()),
                "prestateTracer returned no accounts",
            )?;
            collect_accesses(&mut needed, &trace)?;
        }
    }
    if ai_actor.is_none() {
        use alloy_sol_types::SolCall;
        let actor = corpus.preconditions.actor;
        let token_slot = vault_token_slot();
        let token_word = quantity(&rpc.call(
            "eth_getStorageAt",
            json!([proxy, format!("0x{token_slot:064x}"), tag]),
        )?)?;
        require(
            token_word != U256::ZERO && token_word >> 160 == U256::ZERO,
            "invalid vault token discovery",
        )?;
        let token = Address::from_slice(&token_word.to_be_bytes::<32>()[12..]);
        needed.entry(proxy).or_default().insert(token_slot);
        needed.entry(actor).or_default();
        for (to, data) in [
            (
                token,
                vault_abi::balanceOfCall { account: actor }.abi_encode(),
            ),
            (
                token,
                vault_abi::allowanceCall {
                    owner: actor,
                    spender: proxy,
                }
                .abi_encode(),
            ),
            (
                proxy,
                vault_abi::balanceOfCall { account: actor }.abi_encode(),
            ),
        ] {
            let request = json!({"from":actor, "to":to, "value":"0x0", "data":format!("0x{}", hex::encode(data)), "gas":format!("0x{GAS_LIMIT:x}")});
            let trace = rpc.call(
                "debug_traceCall",
                json!([request, tag, {"tracer":"prestateTracer"}]),
            )?;
            collect_accesses(&mut needed, &trace)?;
        }
    }
    if let Some(actor) = ai_actor {
        use alloy_sol_types::SolCall;
        // Discovery hints only. Every value used in replay is fetched/proof-checked below.
        let token_slot = vault_token_slot();
        let token_word = quantity(&rpc.call(
            "eth_getStorageAt",
            json!([proxy, format!("0x{token_slot:064x}"), tag]),
        )?)?;
        require(
            token_word != U256::ZERO && token_word >> 160 == U256::ZERO,
            "invalid vault token discovery",
        )?;
        let token = Address::from_slice(&token_word.to_be_bytes::<32>()[12..]);
        needed
            .entry(proxy)
            .or_default()
            .extend([token_slot, token_slot + U256::from(2)]);
        needed.entry(token).or_default();
        for (to, data, options) in [
            (
                proxy,
                vault_abi::totalDepositsCall {}.abi_encode(),
                json!({"tracer":"prestateTracer", "stateOverrides":overrides}),
            ),
            (
                token,
                vault_abi::balanceOfCall { account: proxy }.abi_encode(),
                json!({"tracer":"prestateTracer"}),
            ),
        ] {
            let request = json!({"from":actor, "to":to, "value":"0x0", "data":format!("0x{}",hex::encode(data)), "gas":format!("0x{GAS_LIMIT:x}")});
            let trace = rpc.call("debug_traceCall", json!([request, tag, options]))?;
            collect_accesses(&mut needed, &trace)?;
        }
    }
    let proxy_slots = needed.entry(proxy).or_default();
    proxy_slots.extend([IMPL, CAP, INITIALIZABLE]);
    require_contradiction(
        needed.len() <= 256 && needed.values().map(BTreeSet::len).sum::<usize>() <= 4096,
        "witness exceeds account/slot bound",
    )?;
    let mut witness = Witness {
        block,
        header,
        state_root,
        environment,
        proxy,
        new_impl,
        proxy_admin,
        executor,
        signer,
        accounts: BTreeMap::new(),
    };
    for (a, slots) in needed {
        let keys: Vec<_> = slots.iter().map(|s| format!("0x{s:064x}")).collect();
        let p = proof_rpc.call("eth_getProof", json!([a, keys, tag]))?;
        require(address(&p["address"])? == a, "proof address mismatch")?;
        let code = bytes(&rpc.call("eth_getCode", json!([a, tag]))?)?;
        let mut account = AccountWitness {
            balance: quantity(&p["balance"])?,
            nonce: number(&p["nonce"])?,
            code,
            storage: BTreeMap::new(),
            account_proof: proof(&p["accountProof"])?,
            storage_root: fixed(&p["storageHash"])?,
            storage_proofs: BTreeMap::new(),
        };
        require(
            hash(&account.code) == fixed(&p["codeHash"])?,
            "proof code hash mismatch",
        )?;
        if a == new_impl {
            require(
                account.code == new_runtime,
                "candidate code changed during witness fetch",
            )?;
        }
        for entry in p["storageProof"]
            .as_array()
            .ok_or_else(|| incomplete("missing storage proofs"))?
        {
            let key = quantity(&entry["key"])?;
            require(
                slots.contains(&key) && !account.storage.contains_key(&key),
                "unexpected/duplicate proof slot",
            )?;
            account.storage.insert(key, quantity(&entry["value"])?);
            account.storage_proofs.insert(key, proof(&entry["proof"])?);
        }
        require(
            account.storage.len() == slots.len(),
            "missing requested storage proof",
        )?;
        validate_account(witness.state_root, a, &account)?;
        witness.accounts.insert(a, account);
    }
    require(
        witness
            .accounts
            .get(&proxy)
            .and_then(|account| account.storage.get(&IMPL))
            == Some(&implementation_word),
        "implementation discovery does not match authenticated prestate",
    )?;
    let final_header = rpc.call("eth_getBlockByNumber", json!([tag, false]))?;
    require(
        fixed(&final_header["hash"])? == block.hash,
        "evidence block reorged during fetch",
    )?;
    let final_proof_header = proof_rpc.call("eth_getBlockByNumber", json!([tag, false]))?;
    require(
        fixed(&final_proof_header["hash"])? == block.hash,
        "proof evidence block reorged during fetch",
    )?;
    Ok(witness)
}

fn rlp_error(_: rlp::DecoderError) -> Incomplete {
    incomplete("invalid proof RLP")
}
fn rlp_quantity(v: U256) -> Vec<u8> {
    let word = v.to_be_bytes::<32>();
    let first = word.iter().position(|b| *b != 0).unwrap_or(32);
    rlp::encode(&word[first..].as_ref()).to_vec()
}
fn validate_account(root: B256, a: Address, account: &AccountWitness) -> Result<bool, Incomplete> {
    let found = trie_value(root, hash(a.as_slice()), &account.account_proof)?;
    let exists = found.is_some();
    let empty_root = hash(&[0x80]);
    if let Some(encoded) = found {
        let mut expected = RlpStream::new_list(4);
        expected.append_raw(&rlp_quantity(U256::from(account.nonce)), 1);
        expected.append_raw(&rlp_quantity(account.balance), 1);
        expected.append(&account.storage_root.as_slice());
        expected.append(&hash(&account.code).as_slice());
        require(
            encoded == expected.out().as_ref(),
            "account does not match state-root proof",
        )?;
    } else {
        require(
            account.balance == U256::ZERO
                && account.nonce == 0
                && account.code.is_empty()
                && account.storage_root == empty_root,
            "absent account has nonempty state",
        )?;
    }
    for (key, value) in &account.storage {
        let nodes = account
            .storage_proofs
            .get(key)
            .ok_or_else(|| incomplete("missing storage proof"))?;
        let found = trie_value(account.storage_root, hash(&key.to_be_bytes::<32>()), nodes)?;
        match found {
            Some(encoded) => require(
                encoded == rlp_quantity(*value),
                "storage value does not match proof",
            )?,
            None => require(*value == U256::ZERO, "absent storage is nonzero")?,
        }
    }
    Ok(exists)
}

/// Ethereum hexary MPT inclusion/exclusion. No trie node or branch is trusted implicitly.
fn trie_value(root: B256, key: B256, proof: &[Vec<u8>]) -> Result<Option<Vec<u8>>, Incomplete> {
    require(
        proof.len() <= 128 && proof.iter().all(|n| n.len() <= 4096),
        "proof exceeds node bound",
    )?;
    if root == hash(&[0x80]) {
        return Ok(None);
    }
    let nodes: BTreeMap<_, _> = proof
        .iter()
        .map(|node| (hash(node), node.as_slice()))
        .collect();
    let mut reference = root.to_vec();
    let nibbles: Vec<_> = key.iter().flat_map(|b| [b >> 4, b & 15]).collect();
    let mut offset = 0;
    for _ in 0..=64 {
        let node: &[u8] = if reference.len() == 32 {
            nodes
                .get(&B256::from_slice(&reference))
                .copied()
                .ok_or_else(|| incomplete("proof missing referenced node"))?
        } else {
            reference.as_slice()
        };
        let rlp = Rlp::new(node);
        let count = rlp.item_count().map_err(rlp_error)?;
        let child = |index| -> Result<Vec<u8>, Incomplete> {
            let value = rlp.at(index).map_err(rlp_error)?;
            if value.is_list() {
                require(value.as_raw().len() < 32, "oversized inline trie node")?;
                Ok(value.as_raw().to_vec())
            } else {
                let data = value.data().map_err(rlp_error)?;
                require(
                    data.is_empty() || data.len() == 32,
                    "invalid trie child reference",
                )?;
                Ok(data.to_vec())
            }
        };
        match count {
            17 => {
                if offset == 64 {
                    let data = rlp.at(16).map_err(rlp_error)?.data().map_err(rlp_error)?;
                    return Ok((!data.is_empty()).then(|| data.to_vec()));
                }
                reference = child(usize::from(nibbles[offset]))?;
                offset += 1;
            }
            2 => {
                let compact = rlp.at(0).map_err(rlp_error)?.data().map_err(rlp_error)?;
                require(
                    !compact.is_empty() && compact[0] >> 4 <= 3,
                    "invalid compact trie path",
                )?;
                let flag = compact[0] >> 4;
                require(
                    flag & 1 == 1 || compact[0] & 15 == 0,
                    "invalid compact path padding",
                )?;
                let mut path = Vec::new();
                if flag & 1 != 0 {
                    path.push(compact[0] & 15);
                }
                path.extend(compact[1..].iter().flat_map(|b| [b >> 4, b & 15]));
                if !nibbles[offset..].starts_with(&path) {
                    return Ok(None);
                }
                offset += path.len();
                if flag & 2 != 0 {
                    if offset != 64 {
                        return Ok(None);
                    }
                    return Ok(Some(
                        rlp.at(1)
                            .map_err(rlp_error)?
                            .data()
                            .map_err(rlp_error)?
                            .to_vec(),
                    ));
                }
                require(!path.is_empty(), "empty extension path")?;
                reference = child(1)?;
            }
            _ => return Err(incomplete("invalid trie node arity")),
        }
        if reference.is_empty() {
            return Ok(None);
        }
    }
    Err(incomplete("trie traversal exceeds key bound"))
}

#[derive(Clone, PartialEq, Eq)]
struct AccountState {
    exists: bool,
    balance: U256,
    nonce: u64,
    code: Bytecode,
    storage: BTreeMap<U256, U256>,
}
#[derive(Clone)]
struct StrictDb {
    accounts: BTreeMap<Address, AccountState>,
    reads: BTreeSet<(Address, U256)>,
}
impl Database for StrictDb {
    type Error = Incomplete;
    fn basic(&mut self, a: Address) -> Result<Option<AccountInfo>, Incomplete> {
        let s = self
            .accounts
            .get(&a)
            .ok_or_else(|| incomplete("unwitnessed account read"))?;
        Ok(s.exists
            .then(|| AccountInfo::new(s.balance, s.nonce, s.code.hash_slow(), s.code.clone())))
    }
    fn code_by_hash(&mut self, h: B256) -> Result<Bytecode, Incomplete> {
        self.accounts
            .values()
            .find(|a| a.code.hash_slow() == h)
            .map(|a| a.code.clone())
            .ok_or_else(|| incomplete("unwitnessed code read"))
    }
    fn storage(&mut self, a: Address, key: U256) -> Result<U256, Incomplete> {
        self.reads.insert((a, key));
        self.accounts
            .get(&a)
            .and_then(|s| s.storage.get(&key))
            .copied()
            .ok_or_else(|| incomplete("unwitnessed storage read"))
    }
    fn block_hash(&mut self, _: u64) -> Result<B256, Incomplete> {
        Err(incomplete("unwitnessed blockhash read"))
    }
}
impl StrictDb {
    fn commit(&mut self, state: EvmState) -> Result<(), Incomplete> {
        for (a, changed) in state {
            require_contradiction(
                !changed.is_selfdestructed() && !changed.is_created(),
                "creation or destruction outside replay scope",
            )?;
            let account = self
                .accounts
                .get_mut(&a)
                .ok_or_else(|| incomplete("unwitnessed changed account"))?;
            if changed.is_touched() {
                account.exists = !changed.is_empty();
            }
            account.balance = changed.info.balance;
            account.nonce = changed.info.nonce;
            if let Some(code) = changed.info.code {
                account.code = code;
            }
            for (key, value) in changed.storage {
                require(
                    account.storage.contains_key(&key),
                    "unwitnessed changed slot",
                )?;
                account.storage.insert(key, value.present_value());
            }
        }
        Ok(())
    }
}
#[derive(PartialEq, Eq)]
struct Observation {
    success: bool,
    output: Bytes,
    logs: Vec<Log>,
}
fn evm_incomplete(error: revm::context_interface::result::EVMError<Incomplete>) -> Incomplete {
    use revm::context_interface::result::{EVMError, InvalidTransaction};
    let reason = match error {
        EVMError::Database(error) => return error,
        EVMError::Transaction(InvalidTransaction::RejectCallerWithCode) => {
            IncompleteReason::SenderCodeUnexpected
        }
        EVMError::Transaction(
            InvalidTransaction::NonceOverflowInTransaction
            | InvalidTransaction::NonceTooHigh { .. }
            | InvalidTransaction::NonceTooLow { .. },
        ) => IncompleteReason::SenderNonceUnexpected,
        EVMError::Transaction(InvalidTransaction::LackOfFundForMaxFee { .. }) => {
            IncompleteReason::SenderInsufficientBalance
        }
        EVMError::Transaction(_) => IncompleteReason::EvmTransactionInvalid,
        EVMError::Header(_) => IncompleteReason::EvmHeaderInvalid,
        EVMError::Custom(_) | EVMError::CustomAny(_) => IncompleteReason::ReplayIncomplete,
    };
    let mut error = incomplete("EVM execution incomplete");
    error.reason = reason;
    error
}
fn execute(
    db: &mut StrictDb,
    environment: &BlockEnv,
    from: Address,
    proxy: Address,
    value: U256,
    data: Bytes,
) -> Result<Observation, Incomplete> {
    db.reads.clear();
    let nonce = db
        .accounts
        .get(&from)
        .ok_or_else(|| incomplete("unwitnessed caller"))?
        .nonce;
    let tx = TxEnv {
        caller: from,
        kind: TxKind::Call(proxy),
        nonce,
        value,
        data,
        gas_limit: GAS_LIMIT,
        gas_price: u128::from(environment.basefee),
        chain_id: Some(84532),
        ..TxEnv::default()
    };
    let result = Context::mainnet()
        .with_db(&mut *db)
        .with_block(environment.clone())
        .modify_cfg_chained(|cfg| {
            cfg.set_spec_and_mainnet_gas_params(SpecId::PRAGUE);
            cfg.chain_id = 84532;
            cfg.disable_fee_charge = true;
        })
        .build_mainnet()
        .transact(tx)
        .map_err(evm_incomplete)?;
    require_contradiction(
        !matches!(result.result, ExecutionResult::Halt { .. }),
        "EVM halted",
    )?;
    let observation = Observation {
        success: result.result.is_success(),
        output: result
            .result
            .output()
            .cloned()
            .ok_or_else(|| incomplete("missing EVM output"))?,
        logs: if result.result.is_success() {
            result.result.logs().to_vec()
        } else {
            Vec::new()
        },
    };
    db.commit(result.state)?;
    Ok(observation)
}

/// Runs no network/process/file I/O. Returns rule 5 detail codes 0, 50 or 51.
pub fn replay(
    witness: &Witness,
    corpus: &Corpus,
    current_runtime: &[u8],
    new_runtime: &[u8],
    new_init_calldata: &[u8],
) -> (StageStatus, u16) {
    match replay_detailed(
        witness,
        corpus,
        current_runtime,
        new_runtime,
        new_init_calldata,
    ) {
        Ok(true) => (StageStatus::Pass, 0),
        Ok(false) => (StageStatus::Fail, 50),
        Err(_) => (StageStatus::Incomplete, 51),
    }
}
/// Like [`replay`], but keeps the reason an incomplete replay could not finish:
/// a failure of the *candidate's* execution over an authenticated witness is a
/// `Contradiction`; anything else (inconsistent witness, current-runtime execution) is
/// `Environment`.
pub fn replay_detailed(
    witness: &Witness,
    corpus: &Corpus,
    current_runtime: &[u8],
    new_runtime: &[u8],
    new_init_calldata: &[u8],
) -> Result<bool, Incomplete> {
    replay_inner(
        witness,
        corpus,
        current_runtime,
        new_runtime,
        new_init_calldata,
    )
}
fn environmental(mut error: Incomplete) -> Incomplete {
    error.kind = IncompleteKind::Environment;
    error
}
fn candidate_fault(mut error: Incomplete) -> Incomplete {
    error.kind = IncompleteKind::Contradiction;
    error
}
fn check_preconditions(old: &StrictDb, w: &Witness, corpus: &Corpus) -> Result<(), Incomplete> {
    let p = &corpus.preconditions;
    let token_word = old
        .accounts
        .get(&w.proxy)
        .and_then(|account| account.storage.get(&vault_token_slot()))
        .copied()
        .ok_or_else(|| incomplete("missing vault token slot"))?;
    require(
        token_word >> 160 == U256::ZERO,
        "invalid token address word",
    )?;
    let token = Address::from_slice(&token_word.to_be_bytes::<32>()[12..]);
    let view = |target: Address, signature: &[u8], args: &[Address]| -> Result<U256, Incomplete> {
        let mut data = keccak(signature)[..4].to_vec();
        for address in args {
            data.extend_from_slice(&[0; 12]);
            data.extend_from_slice(address.as_slice());
        }
        let mut db = old.clone();
        let result = execute(
            &mut db,
            &w.environment,
            p.actor,
            target,
            U256::ZERO,
            data.into(),
        )
        .map_err(environmental)?;
        require(
            result.success && result.output.len() == 32,
            "corpus precondition view failed",
        )?;
        Ok(U256::from_be_slice(&result.output))
    };
    let balance = view(token, b"balanceOf(address)", &[p.actor])?;
    let allowance = view(token, b"allowance(address,address)", &[p.actor, w.proxy])?;
    let vault = view(w.proxy, b"balanceOf(address)", &[p.actor])?;
    require(
        balance >= decimal_amount(&p.min_token_balance)?
            && allowance >= decimal_amount(&p.min_allowance)?
            && vault >= decimal_amount(&p.min_vault_balance)?
            && vault <= decimal_amount(&p.max_vault_balance)?,
        "corpus precondition unmet",
    )?;
    // Establish every reference outcome before any candidate fault can become a rejection.
    let mut reference = old.clone();
    for call in &corpus.calls {
        let value = U256::from(
            call.value
                .parse::<u128>()
                .map_err(|_| incomplete("invalid call value"))?,
        );
        let observation = execute(
            &mut reference,
            &w.environment,
            call.from,
            w.proxy,
            value,
            call.data.clone(),
        )
        .map_err(environmental)?;
        require(
            observation.success == (call.expect == Expect::Success),
            "old call contradicts expected outcome",
        )?;
    }
    Ok(())
}

fn replay_inner(
    w: &Witness,
    corpus: &Corpus,
    current: &[u8],
    new: &[u8],
    init: &[u8],
) -> Result<bool, Incomplete> {
    let Some((mut old, mut upgraded)) = prepared_states(
        w,
        corpus,
        current,
        new,
        init,
        corpus.calls.first().map(|c| c.from),
        true,
    )?
    else {
        return Ok(false);
    };
    for call in &corpus.calls {
        let value = U256::from(
            call.value
                .parse::<u128>()
                .map_err(|_| incomplete("invalid call value"))?,
        );
        let metadata_before = [IMPL, CAP, INITIALIZABLE]
            .map(|key| upgraded.accounts[&w.proxy].storage.get(&key).copied());
        // The current implementation is the reference; its failing to execute over the
        // witness says nothing about the candidate.
        let original = execute(
            &mut old,
            &w.environment,
            call.from,
            w.proxy,
            value,
            call.data.clone(),
        )
        .map_err(environmental)?;
        require(
            original.success == (call.expect == Expect::Success),
            "old call contradicts expected outcome",
        )?;
        let changed = execute(
            &mut upgraded,
            &w.environment,
            call.from,
            w.proxy,
            value,
            call.data.clone(),
        )
        .map_err(candidate_fault)?;
        if original != changed
            || metadata_before
                != [IMPL, CAP, INITIALIZABLE]
                    .map(|key| upgraded.accounts[&w.proxy].storage.get(&key).copied())
        {
            return Ok(false);
        }
        let filtered_reads = |db: &StrictDb| {
            db.reads
                .iter()
                .copied()
                .filter(|(a, k)| *a != w.proxy || *k != CAP)
                .collect::<BTreeSet<_>>()
        };
        if filtered_reads(&old) != filtered_reads(&upgraded) {
            return Ok(false);
        }
        for (a, state) in &old.accounts {
            let other = upgraded
                .accounts
                .get(a)
                .ok_or_else(|| incomplete("missing comparison account"))?;
            if state.balance != other.balance
                || state.exists != other.exists
                || state.nonce != other.nonce
                || state.code != other.code
            {
                return Ok(false);
            }
            for (key, value) in &state.storage {
                if *a == w.proxy && *key == IMPL {
                    if other.storage.get(key) != Some(&U256::from_be_slice(w.new_impl.as_slice())) {
                        return Ok(false);
                    }
                    continue;
                }
                if *a == w.proxy && (*key == CAP || *key == INITIALIZABLE) {
                    continue;
                }
                if other.storage.get(key) != Some(value) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn prepared_states(
    w: &Witness,
    corpus: &Corpus,
    current: &[u8],
    new: &[u8],
    init: &[u8],
    _initializer_sender: Option<Address>,
    check_corpus: bool,
) -> Result<Option<(StrictDb, StrictDb)>, Incomplete> {
    corpus.validate()?;
    let (root, environment) = verified_header(&w.header, w.block)?;
    require(
        root == w.state_root && environment == w.environment,
        "witness environment differs from pinned header",
    )?;
    require(
        !current.is_empty()
            && !new.is_empty()
            && current.len() <= 24_576
            && new.len() <= 24_576
            && init.len() <= 1024,
        "invalid runtime/init size",
    )?;
    require(
        w.accounts.len() <= 256 && w.environment.number == U256::from(w.block.number),
        "invalid witness environment",
    )?;
    let old = StrictDb {
        accounts: w
            .accounts
            .iter()
            .map(|(a, s)| {
                let exists = validate_account(w.state_root, *a, s)?;
                Ok((
                    *a,
                    AccountState {
                        exists,
                        balance: s.balance,
                        nonce: s.nonce,
                        code: Bytecode::new_raw(Bytes::copy_from_slice(&s.code)),
                        storage: s.storage.clone(),
                    },
                ))
            })
            .collect::<Result<_, Incomplete>>()?,
        reads: BTreeSet::new(),
    };
    let proxy = old
        .accounts
        .get(&w.proxy)
        .ok_or_else(|| incomplete("missing proxy"))?;
    require(!proxy.code.is_empty(), "proxy runtime absent")?;
    let impl_word = *proxy
        .storage
        .get(&IMPL)
        .ok_or_else(|| incomplete("missing implementation slot"))?;
    require(
        impl_word >> 160 == U256::ZERO,
        "invalid implementation address word",
    )?;
    let implementation = Address::from_slice(&impl_word.to_be_bytes::<32>()[12..]);
    require(
        old.accounts
            .get(&implementation)
            .is_some_and(|a| a.code.original_byte_slice() == current),
        "current runtime differs from witness",
    )?;
    if check_corpus {
        check_preconditions(&old, w, corpus)?;
    }
    require(
        old.accounts
            .get(&w.new_impl)
            .is_some_and(|a| a.exists && a.nonce == 1 && a.code.original_byte_slice() == new),
        "candidate runtime differs from authenticated account",
    )?;
    require(
        old.accounts
            .get(&w.proxy_admin)
            .and_then(|a| a.storage.get(&U256::ZERO))
            == Some(&U256::from_be_slice(w.executor.as_slice()))
            && old
                .accounts
                .get(&w.executor)
                .and_then(|a| a.storage.get(&U256::ZERO))
                == Some(&U256::from_be_slice(w.signer.as_slice())),
        "upgrade authority differs from authenticated prestate",
    )?;
    if !init.is_empty() {
        require(
            proxy.storage.contains_key(&CAP) && proxy.storage.contains_key(&INITIALIZABLE),
            "missing declared initializer prestate",
        )?;
        if proxy.storage[&CAP] != U256::ZERO || proxy.storage[&INITIALIZABLE] != U256::ONE {
            return Ok(None);
        }
    }
    let mut upgraded = old.clone();
    let outcome = execute(
        &mut upgraded,
        &w.environment,
        w.signer,
        w.executor,
        U256::ZERO,
        upgrade_data(w.new_impl, new, implementation, init),
    )
    .map_err(candidate_fault)?;
    if !outcome.success {
        return Ok(None);
    }
    if upgraded.accounts[&w.proxy].storage.get(&IMPL)
        != Some(&U256::from_be_slice(w.new_impl.as_slice()))
        || (!init.is_empty()
            && upgraded.accounts[&w.proxy].storage.get(&INITIALIZABLE) != Some(&U256::from(2)))
    {
        return Ok(None);
    }
    // The executor's documented writes and the transaction sender's nonce are
    // the only non-proxy differences allowed by the upgrade itself.
    let mut mapping_key = [0u8; 64];
    mapping_key[63] = 2;
    let used_slot = U256::from_be_slice(&keccak(&mapping_key));
    let executor_before = old
        .accounts
        .get(&w.executor)
        .ok_or_else(|| incomplete("missing executor"))?;
    let executor_after = upgraded
        .accounts
        .get_mut(&w.executor)
        .ok_or_else(|| incomplete("missing executor"))?;
    if executor_before.storage.get(&U256::ONE) != Some(&impl_word)
        || executor_after.storage.get(&U256::ONE)
            != Some(&U256::from_be_slice(w.new_impl.as_slice()))
        || executor_before.storage.get(&used_slot) != Some(&U256::ZERO)
        || executor_after.storage.get(&used_slot) != Some(&U256::ONE)
    {
        return Ok(None);
    }
    executor_after.storage.insert(U256::ONE, impl_word);
    executor_after.storage.insert(used_slot, U256::ZERO);
    upgraded
        .accounts
        .get_mut(&w.signer)
        .ok_or_else(|| incomplete("missing signer"))?
        .nonce = old
        .accounts
        .get(&w.signer)
        .ok_or_else(|| incomplete("missing signer"))?
        .nonce;
    for (a, state) in &upgraded.accounts {
        let original = &old.accounts[a];
        if state.balance != original.balance
            || state.exists != original.exists
            || state.nonce != original.nonce
            || state.code != original.code
        {
            return Ok(None);
        }
        for (key, value) in &state.storage {
            if *a == w.proxy
                && (*key == IMPL || (!init.is_empty() && (*key == CAP || *key == INITIALIZABLE)))
            {
                continue;
            }
            if original.storage.get(key) != Some(value) {
                return Ok(None);
            }
        }
    }
    Ok(Some((old, upgraded)))
}

/// Crate-private authenticated upgraded fork; all execution uses R5's strict DB.
pub(crate) struct CandidateReplay {
    db: StrictDb,
    environment: BlockEnv,
    proxy: Address,
}
impl CandidateReplay {
    pub(crate) fn new(
        w: &Witness,
        corpus: &Corpus,
        current: &[u8],
        new: &[u8],
        init: &[u8],
        initializer_sender: Address,
    ) -> Result<Self, Incomplete> {
        let (_, db) = prepared_states(
            w,
            corpus,
            current,
            new,
            init,
            Some(initializer_sender),
            false,
        )?
        .ok_or_else(|| incomplete("candidate initializer failed replay policy"))?;
        Ok(Self {
            db,
            environment: w.environment.clone(),
            proxy: w.proxy,
        })
    }
    pub(crate) fn call_proxy(&mut self, from: Address, data: Bytes) -> Result<(), Incomplete> {
        require(
            self.db
                .accounts
                .get(&from)
                .is_some_and(|a| a.code.is_empty()),
            "candidate caller must be a witnessed EOA",
        )?;
        execute(
            &mut self.db,
            &self.environment,
            from,
            self.proxy,
            U256::ZERO,
            data,
        )?;
        Ok(())
    }
    /// Predicate reads run on a discarded clone, so untrusted getters cannot change replay state.
    pub(crate) fn query(
        &self,
        from: Address,
        to: Address,
        data: Bytes,
    ) -> Result<Bytes, Incomplete> {
        let result = execute(
            &mut self.db.clone(),
            &self.environment,
            from,
            to,
            U256::ZERO,
            data,
        )?;
        require(result.success, "R2 predicate getter reverted")?;
        Ok(result.output)
    }
}

#[cfg(test)]
mod attribution_tests {
    use super::*;

    #[test]
    fn evm_errors_keep_safe_witness_and_sender_validation_reasons() {
        use revm::context_interface::result::{EVMError, InvalidTransaction};
        let missing = evm_incomplete(EVMError::Database(incomplete("unwitnessed account read")));
        assert_eq!(missing.message, "unwitnessed account read");
        let code = evm_incomplete(EVMError::Transaction(
            InvalidTransaction::RejectCallerWithCode,
        ));
        assert_eq!(code.reason.code(), "sender_code_unexpected");
        let nonce = evm_incomplete(EVMError::Transaction(
            InvalidTransaction::NonceOverflowInTransaction,
        ));
        assert_eq!(nonce.reason.code(), "sender_nonce_unexpected");
    }

    #[test]
    fn rpc_failures_are_classified_without_response_secrets() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        for (status, body, expected) in [
            ("429 Too Many Requests", "secret", "rpc_rate_limited"),
            ("403 Forbidden", "secret", "rpc_http_4xx"),
            ("503 Service Unavailable", "secret", "rpc_http_5xx"),
            (
                "200 OK",
                r#"{"error":{"code":-32601,"message":"secret"}}"#,
                "rpc_method_unsupported",
            ),
            (
                "200 OK",
                r#"{"error":{"code":-32602,"message":"distance to target block exceeds maximum proof window secret"}}"#,
                "proof_window_exceeded",
            ),
            (
                "200 OK",
                r#"{"error":{"code":-32005,"message":"secret"}}"#,
                "rpc_rate_limited",
            ),
            (
                "200 OK",
                r#"{"error":{"code":-32000,"message":"secret"}}"#,
                "rpc_error",
            ),
            ("200 OK", "secret", "malformed_response"),
            ("200 OK", r#"{"result":"0x14a34"}"#, "success"),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!(
                "http://{}/secret?token=secret",
                listener.local_addr().unwrap()
            );
            let body = body.to_owned();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            });
            let rpc = Rpc::new(&url, Instant::now() + Duration::from_secs(5)).unwrap();
            let result = rpc.call_once("eth_getProof", &json!([]));
            if expected == "success" {
                assert_eq!(result.unwrap(), json!("0x14a34"));
            } else {
                let error = result.unwrap_err();
                assert_eq!(error.reason.code(), expected);
                assert!(!format!("{error}").contains("secret"));
            }
            server.join().unwrap();
        }
    }

    #[test]
    fn rate_limited_reads_back_off_then_succeed_and_respect_the_deadline() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        let serve = |responses: Vec<(&'static str, &'static str)>| {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let handle = std::thread::spawn(move || {
                for (status, body) in responses {
                    let (mut stream, _) = listener.accept().unwrap();
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf);
                    write!(
                        stream,
                        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .unwrap();
                }
            });
            (url, handle)
        };
        // Two throttled answers, one provider-style JSON-RPC limit, then success.
        let (url, handle) = serve(vec![
            ("429 Too Many Requests", "slow down"),
            (
                "200 OK",
                r#"{"error":{"code":-32011,"message":"request limit reached"}}"#,
            ),
            (
                "200 OK",
                r#"{"error":{"code":-32000,"message":"Rate limit exceeded"}}"#,
            ),
            ("200 OK", r#"{"result":"0x14a34"}"#),
        ]);
        let rpc = Rpc::new(&url, Instant::now() + Duration::from_secs(30)).unwrap();
        assert_eq!(
            rpc.call("eth_chainId", json!([])).unwrap(),
            json!("0x14a34")
        );
        handle.join().unwrap();
        // A non-throttle failure still gets exactly two attempts, not the long backoff.
        let (url, handle) = serve(vec![
            ("200 OK", r#"{"error":{"code":-32000,"message":"boom"}}"#),
            ("200 OK", r#"{"error":{"code":-32000,"message":"boom"}}"#),
        ]);
        let rpc = Rpc::new(&url, Instant::now() + Duration::from_secs(30)).unwrap();
        assert_eq!(
            rpc.call("eth_chainId", json!([]))
                .unwrap_err()
                .reason
                .code(),
            "rpc_error"
        );
        handle.join().unwrap();
        // A throttled provider cannot hold the run past its deadline.
        let (url, handle) = serve(vec![("429 Too Many Requests", "x"); 3]);
        let started = Instant::now();
        let rpc = Rpc::new(&url, started + Duration::from_millis(900)).unwrap();
        assert_eq!(
            rpc.call("eth_chainId", json!([]))
                .unwrap_err()
                .reason
                .code(),
            "rpc_rate_limited"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        handle.join().unwrap();
    }

    #[test]
    fn candidate_incomplete_and_discovery_bounds_are_contradictions() {
        for message in ["unwitnessed blockhash read", "EVM execution incomplete"] {
            assert_eq!(
                candidate_fault(incomplete(message)).kind,
                IncompleteKind::Contradiction
            );
        }
        let trace = Value::Object(
            (0..257u32)
                .map(|i| (format!("0x{i:040x}"), json!({})))
                .collect(),
        );
        assert_eq!(
            collect_accesses(&mut Accesses::new(), &trace)
                .unwrap_err()
                .kind,
            IncompleteKind::Contradiction
        );
    }
}
