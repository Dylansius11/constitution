//! Pure application state and reducer. Filesystem/network/process work belongs to data.rs.
use constitution_types::ProposalState;
use serde_json::Value;

/// Source: skills/constitution/references/refusals.md, inspected 2026-11-12.
/// Numeric certificate reasons are not NEAR E.. transaction panic codes.
pub const REFUSALS: &[(u8, u16, &str)] = &[
    (
        0,
        1,
        "Source commitment mismatch (specified code; current pipeline normally withholds bad_bundle)",
    ),
    (
        0,
        2,
        "Candidate compilation/runtime failure or deployed code hash mismatch",
    ),
    (
        0,
        3,
        "Current implementation differs from the proposal's expected implementation",
    ),
    (
        0,
        4,
        "Pinned reference/harness source missing or changed, or current runtime not reproduced",
    ),
    (
        1,
        10,
        "Incompatible storage layout or proposal-attributed layout tool failure",
    ),
    (1, 11, "Committed reference storage layout is missing"),
    (
        2,
        20,
        "Bounded symbolic solvency or implementation-slot counterexample",
    ),
    (2, 21, "Seeded Foundry fuzz counterexample"),
    (
        2,
        22,
        "AI-suggested counterexample deterministically reproduced",
    ),
    (
        2,
        23,
        "Solver/path incompleteness or proposal-attributed solvency tool failure",
    ),
    (
        3,
        30,
        "Forbidden runtime opcode: SELFDESTRUCT, CALLCODE or DELEGATECALL",
    ),
    (
        3,
        31,
        "Forbidden ERC-1967 slot/label or Foundry/Halmos cheatcode constant",
    ),
    (4, 40, "Missing or unknown initializer selector"),
    (
        4,
        41,
        "Wrong initializer ABI length or argument exceeds 2^128-1",
    ),
    (
        5,
        50,
        "Reference/candidate replay differs or the candidate upgrade/initializer fails",
    ),
    (
        5,
        51,
        "Authenticated candidate execution/witness-bound incompleteness",
    ),
];
#[derive(Clone, Debug)]
pub struct Refusal {
    pub rule_id: u8,
    pub reason_code: u16,
    pub meaning: &'static str,
}
impl Refusal {
    pub fn new(rule_id: u8, reason_code: u16) -> Self {
        Self {
            rule_id,
            reason_code,
            meaning: REFUSALS
                .iter()
                .find(|(r, c, _)| *r == rule_id && *c == reason_code)
                .map_or(
                    "Unknown refusal code; inspect accepted certificate and evidence",
                    |(_, _, meaning)| *meaning,
                ),
        }
    }
}
#[derive(Debug)]
pub struct ProposalView {
    pub id: u64,
    pub proposer: String,
    pub state: ProposalState,
    pub refusal: Option<Refusal>,
    pub created_at: String,
    pub expires_at: String,
    pub new_impl: String,
    pub codehash: String,
    pub expected_current_impl: String,
    pub init_calldata: String,
    pub source_commitment: String,
    pub certificate_hash: Option<String>,
    pub evidence_root: Option<String>,
    pub evm_tx_hash: Option<String>,
    pub evm_status: Option<String>,
    pub gate_url: String,
    pub new_impl_url: String,
    pub current_impl_url: String,
    pub evm_tx_url: Option<String>,
    pub raw: Value,
}
#[derive(Debug)]
pub struct PendingChangeView {
    pub id: u64,
    pub kind: String,
    pub status: String,
    pub eta_ns: u64,
    pub eta_utc: String,
    pub remaining_ns: u64,
    pub remaining_seconds: u64,
    pub matured: bool,
    pub raw: Value,
}
impl PendingChangeView {
    pub fn update_countdown(&mut self, now_ns: u64) {
        self.remaining_ns = self.eta_ns.saturating_sub(now_ns);
        self.remaining_seconds = self.remaining_ns.div_ceil(1_000_000_000);
        self.matured = now_ns >= self.eta_ns;
    }
}
#[derive(Debug)]
pub struct VerifierIdentity {
    pub verifier_pubkey: Option<String>,
    pub release_hash: Option<String>,
    pub compose_hash: Option<String>,
    pub recorded: bool,
    pub observed_at: Option<String>,
}
#[derive(Clone, Debug)]
pub struct AppraisalCheck {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}
#[derive(Clone, Debug)]
pub struct AppraisalResult {
    pub checks: Vec<AppraisalCheck>,
    pub observed_at: Option<String>,
    pub recorded: bool,
    pub recorded_date: Option<String>,
}
impl AppraisalResult {
    pub fn ok(&self) -> bool {
        !self.checks.is_empty() && self.checks.iter().all(|c| c.ok)
    }
    pub fn passed(&self) -> usize {
        self.checks.iter().filter(|c| c.ok).count()
    }
}
#[derive(Clone, Debug)]
pub struct RecordedDuration {
    pub elapsed_ms: u64,
    pub observed_at: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Success,
    Error,
}
#[derive(Debug)]
pub struct ActionResult {
    pub level: ToastLevel,
    pub message: String,
    pub observed_at: Option<String>,
}

/// Pure UTC formatting; nanoseconds are preserved when present, with no locale dependency.
pub fn utc_from_ns(ns: u64) -> String {
    let seconds = ns / 1_000_000_000;
    let z = (seconds / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(m <= 2);
    let base = format!(
        "{year:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60
    );
    if ns.is_multiple_of(1_000_000_000) {
        format!("{base}Z")
    } else {
        format!("{base}.{:09}Z", ns % 1_000_000_000)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Profile {
    #[default]
    Demo,
    Final,
}
impl Profile {
    pub fn label(self) -> &'static str {
        match self {
            Self::Demo => "Demo gate",
            Self::Final => "Final gate (read-only)",
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Overview,
    Proposals,
    Run,
    Audit,
    Receipt,
}
impl Screen {
    pub const ALL: [Self; 5] = [
        Self::Overview,
        Self::Proposals,
        Self::Run,
        Self::Audit,
        Self::Receipt,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Proposals => "Proposals",
            Self::Run => "Run",
            Self::Audit => "Audit",
            Self::Receipt => "Receipt",
        }
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Active,
    Pass,
    Refused,
    Error,
    NotRun,
}
#[derive(Clone, Debug)]
pub struct Step {
    pub name: &'static str,
    pub state: StepState,
    pub detail: String,
    pub elapsed_ms: u64,
    /// Client-observed stage transitions, not invented remote execution timestamps.
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}
pub const STAGES: [&str; 10] = [
    "Register source",
    "Verifier build",
    "Rules R1–R5",
    "AI review",
    "Certificate",
    "Gate verdict",
    "Signature",
    "Base relay",
    "Receipt",
    "Outcome",
];
#[derive(Clone, Debug)]
pub struct Preset {
    pub id: String,
    pub title: String,
    pub contract: String,
    pub implementation: String,
    pub init: String,
    pub rule: Option<u64>,
    pub upgrade: bool,
    pub requires_impl: Option<String>,
    pub previous_run: Option<RecordedDuration>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Availability {
    Ready,
    Done,
    RequiresPrevious,
    Unknown,
}
impl Preset {
    pub fn availability(&self, current: Option<&str>) -> Availability {
        if !self.upgrade {
            return Availability::Ready;
        }
        let Some(current) = current else {
            return Availability::Unknown;
        };
        if current.eq_ignore_ascii_case(&self.implementation) {
            Availability::Done
        } else if self
            .requires_impl
            .as_ref()
            .is_some_and(|r| !r.eq_ignore_ascii_case(current))
        {
            Availability::RequiresPrevious
        } else {
            Availability::Ready
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wizard {
    pub account: String,
    pub key_file: String,
    pub bundle: String,
    pub contract: String,
    pub implementation: String,
    pub init: String,
    pub upgrade: bool,
    pub preset: usize,
    pub field: usize,
    pub editing: bool,
    pub confirmation: Option<String>,
}
impl Default for Wizard {
    fn default() -> Self {
        Self {
            account: String::new(),
            key_file: String::new(),
            bundle: "contracts".into(),
            contract: "src/demo/VaultV2Honest.sol:VaultV2Honest".into(),
            implementation: String::new(),
            init: "0x".into(),
            upgrade: false,
            preset: 0,
            field: 0,
            editing: false,
            confirmation: None,
        }
    }
}
impl Wizard {
    pub const LABELS: [&str; 6] = [
        "NEAR account",
        "Key-file path (never displayed contents)",
        "Bundle directory",
        "Contract path:Name",
        "Implementation (20-byte hex)",
        "Init calldata",
    ];
    pub fn field_mut(&mut self) -> &mut String {
        match self.field {
            0 => &mut self.account,
            1 => &mut self.key_file,
            2 => &mut self.bundle,
            3 => &mut self.contract,
            4 => &mut self.implementation,
            _ => &mut self.init,
        }
    }
    pub fn fields(&self) -> [&str; 6] {
        [
            &self.account,
            &self.key_file,
            &self.bundle,
            &self.contract,
            &self.implementation,
            &self.init,
        ]
    }
    pub fn validate(&self, key_exists: bool) -> Result<(), String> {
        if self
            .account
            .parse::<constitution_types::AccountId>()
            .is_err()
        {
            return Err("Enter a valid NEAR account id".into());
        }
        if self.key_file.is_empty() || !key_exists {
            return Err("The key file must exist and be a regular file; correct its path".into());
        }
        let address: constitution_types::Hex20 =
            serde_json::from_value(Value::String(self.implementation.clone())).map_err(|_| {
                "Malformed implementation: use lowercase 0x + 40 hex digits".to_owned()
            })?;
        if address.0 == [0; 20] {
            return Err("The implementation cannot be zero".into());
        }
        let init: constitution_types::HexBytes =
            serde_json::from_value(Value::String(self.init.clone())).map_err(|_| {
                "Malformed init calldata: use lowercase 0x-prefixed even-length hex".to_owned()
            })?;
        if init.0.len() > constitution_types::MAX_INIT_CALLDATA {
            return Err("Init calldata exceeds 1024 bytes".into());
        }
        if self.bundle.is_empty() || !self.contract.contains(':') {
            return Err("Set bundle directory and src/path.sol:Name".into());
        }
        Ok(())
    }
}
#[derive(Debug, Default)]
pub struct Snapshot {
    pub observed_at: String,
    pub gate: String,
    pub access_keys: Option<usize>,
    pub rule_hash: Option<String>,
    pub manifest_hash: Option<String>,
    pub implementation: Option<String>,
    pub delay_days: Option<u32>,
    pub changes: Vec<PendingChangeView>,
    pub proposals: Vec<ProposalView>,
    pub verifier: String,
    pub identity: Option<VerifierIdentity>,
    pub recorded_appraisal: Option<AppraisalResult>,
    pub gate_url: String,
    pub proxy_url: String,
    pub errors: Vec<String>,
}
#[derive(Debug)]
pub struct AuditCheck {
    pub name: String,
    pub ok: bool,
    pub warning: bool,
    pub detail: String,
}
#[derive(Debug)]
pub struct AuditResult {
    pub checks: Vec<AuditCheck>,
    pub exit: u8,
    pub observed_at: String,
}
#[derive(Debug)]
pub struct App {
    pub profile: Profile,
    pub screen: Screen,
    pub help: bool,
    pub quit: bool,
    pub snapshot: Option<Snapshot>,
    pub refreshing: bool,
    pub error: Option<String>,
    pub presets: Vec<Preset>,
    pub selected_proposal: usize,
    pub detail: Option<ProposalView>,
    pub appraisal: Option<AppraisalResult>,
    pub appraising: bool,
    pub show_raw_json: bool,
    pub last_action_result: Option<ActionResult>,
    pub now_ns: u64,
    pub run_started_at: Option<String>,
    pub run_finished_at: Option<String>,
    pub wizard: Wizard,
    pub steps: Vec<Step>,
    pub running: bool,
    pub preparing: bool,
    pub write_sent: bool,
    pub confirm_cancel: bool,
    pub cancelling: bool,
    pub elapsed_ms: u64,
    pub output: Option<String>,
    pub run_log: Vec<String>,
    pub audit: Option<AuditResult>,
    pub auditing: bool,
    pub receipt_id: String,
    pub receipt_path: String,
    pub receipt_editing: bool,
    pub receipt_save_editing: bool,
    pub receipt: Option<Value>,
    pub receipt_loading: bool,
    pub receipt_scroll: u16,
    pub scroll: u16,
}
impl Default for App {
    fn default() -> Self {
        Self {
            profile: Profile::Demo,
            screen: Screen::Overview,
            help: false,
            quit: false,
            snapshot: None,
            refreshing: false,
            error: None,
            presets: Vec::new(),
            selected_proposal: 0,
            detail: None,
            appraisal: None,
            appraising: false,
            show_raw_json: false,
            last_action_result: None,
            now_ns: 0,
            run_started_at: None,
            run_finished_at: None,
            wizard: Wizard::default(),
            steps: STAGES
                .iter()
                .map(|name| Step {
                    name,
                    state: StepState::Pending,
                    detail: String::new(),
                    elapsed_ms: 0,
                    started_at: None,
                    finished_at: None,
                })
                .collect(),
            running: false,
            preparing: false,
            write_sent: false,
            confirm_cancel: false,
            cancelling: false,
            elapsed_ms: 0,
            output: None,
            run_log: Vec::new(),
            audit: None,
            auditing: false,
            receipt_id: String::new(),
            receipt_path: String::new(),
            receipt_editing: false,
            receipt_save_editing: false,
            receipt: None,
            receipt_loading: false,
            receipt_scroll: 0,
            scroll: 0,
        }
    }
}
impl App {
    fn toast(&mut self, level: ToastLevel, message: String) {
        self.last_action_result = Some(ActionResult {
            level,
            message,
            observed_at: (self.now_ns != 0).then(|| utc_from_ns(self.now_ns)),
        });
    }
}
#[derive(Debug)]
pub enum Action {
    Refresh(Profile),
    Audit(Profile),
    Appraise(Profile),
    Detail(Profile, u64),
    Prepare(Wizard),
    Run(Wizard),
    CancelRun,
    Receipt(Profile, u64),
    SaveReceipt(String, Value),
}
#[derive(Debug)]
pub enum Event {
    Navigate(Screen),
    NextScreen,
    Back,
    Help,
    SwitchProfile,
    Refresh,
    Snapshot(Profile, Snapshot),
    Failure(String),
    Presets(Vec<Preset>),
    SelectProposal(isize),
    Detail(Profile, ProposalView),
    SelectPreset(isize),
    PrepareRun,
    Prepared(Wizard, String),
    StartRun,
    Stage {
        index: usize,
        state: StepState,
        detail: String,
        elapsed_ms: u64,
    },
    WriteSent,
    Log(String),
    RunStarted(String),
    RunFinished(Result<(), String>),
    Tick(u64),
    Cancel,
    ConfirmCancel,
    Audit,
    Audited(Profile, AuditResult),
    FetchReceipt,
    Receipt(Profile, Value),
    SaveReceipt,
    Saved(String),
    Appraise,
    Appraised(Profile, AppraisalResult),
    ToggleRawJson,
    Clock(u64),
}
pub fn update(app: &mut App, event: Event) -> Vec<Action> {
    match event {
        Event::Clock(now_ns) => {
            app.now_ns = now_ns;
            if let Some(snapshot) = &mut app.snapshot {
                for change in &mut snapshot.changes {
                    change.update_countdown(now_ns);
                }
            }
        }
        Event::ToggleRawJson => app.show_raw_json = !app.show_raw_json,
        Event::Appraise if !app.appraising => {
            if app.profile == Profile::Final {
                app.toast(ToastLevel::Info,"Final verifier retired: only recorded appraisal is available; no probe attempted".into());
            } else {
                app.appraising = true;
                app.error = None;
                app.toast(
                    ToastLevel::Info,
                    "Appraising the demo verifier against the pinned compose hash".into(),
                );
                return vec![Action::Appraise(app.profile)];
            }
        }
        Event::Appraised(profile, result) if profile == app.profile => {
            app.appraising = false;
            let ok = result.ok();
            let message = if ok {
                format!(
                    "Verifier appraised {}/{} checks",
                    result.passed(),
                    result.checks.len()
                )
            } else {
                format!(
                    "Verifier appraisal failed: {}; recovery: do not register/use this key; inspect the checks, obtain fresh expected-CVM evidence, then appraise again",
                    result
                        .checks
                        .iter()
                        .filter(|c| !c.ok)
                        .map(|c| format!("{}: {}", c.name, c.detail))
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            };
            if !ok {
                app.error = Some(message.clone());
            }
            app.toast(
                if ok {
                    ToastLevel::Success
                } else {
                    ToastLevel::Error
                },
                message,
            );
            app.appraisal = Some(result);
        }
        Event::Navigate(screen) => {
            app.screen = screen;
            app.scroll = 0;
        }
        Event::NextScreen => {
            app.screen = Screen::ALL[(app.screen.index() + 1) % 5];
            app.scroll = 0;
        }
        Event::Back => {
            if app.confirm_cancel {
                app.confirm_cancel = false;
            } else if app.help {
                app.help = false;
            } else if app.wizard.editing || app.receipt_editing || app.receipt_save_editing {
                app.wizard.editing = false;
                app.receipt_editing = false;
                app.receipt_save_editing = false;
            } else if app.wizard.confirmation.take().is_some() {
            } else if app.running {
                return update(app, Event::Cancel);
            } else if app.screen != Screen::Overview {
                app.screen = Screen::Overview;
            } else {
                app.quit = true;
            }
        }
        Event::Help => app.help = !app.help,
        Event::SwitchProfile if !app.running => {
            app.profile = if app.profile == Profile::Demo {
                Profile::Final
            } else {
                Profile::Demo
            };
            app.snapshot = None;
            app.detail = None;
            app.audit = None;
            app.receipt = None;
            app.selected_proposal = 0;
            app.wizard.confirmation = None;
            app.error = None;
            app.refreshing = true;
            app.auditing = false;
            app.receipt_loading = false;
            app.preparing = false;
            app.appraising = false;
            app.appraisal = None;
            app.show_raw_json = false;
            app.toast(
                ToastLevel::Info,
                format!("Switched to {}", app.profile.label()),
            );
            return vec![Action::Refresh(app.profile)];
        }
        Event::Refresh if !app.refreshing => {
            app.refreshing = true;
            return vec![Action::Refresh(app.profile)];
        }
        Event::Snapshot(profile, snapshot) if profile == app.profile => {
            if profile == Profile::Final {
                app.appraisal = snapshot.recorded_appraisal.clone();
            }
            app.snapshot = Some(snapshot);
            if let Some(snapshot) = &mut app.snapshot {
                for change in &mut snapshot.changes {
                    change.update_countdown(app.now_ns);
                }
            }
            app.toast(
                ToastLevel::Info,
                "Public-state observation refreshed".into(),
            );
            app.refreshing = false;
        }
        Event::Failure(error) => {
            app.toast(ToastLevel::Error, error.clone());
            app.error = Some(error);
            app.refreshing = false;
            app.auditing = false;
            app.receipt_loading = false;
            app.preparing = false;
        }
        Event::Presets(presets) => app.presets = presets,
        Event::SelectProposal(delta) => {
            let len = app.snapshot.as_ref().map_or(0, |s| s.proposals.len());
            if len > 0 {
                app.selected_proposal =
                    (app.selected_proposal as isize + delta).rem_euclid(len as isize) as usize;
                if let Some(id) = app
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.proposals.get(app.selected_proposal))
                    .map(|p| p.id)
                {
                    app.detail = None;
                    return vec![Action::Detail(app.profile, id)];
                }
            }
        }
        Event::Detail(profile, value) if profile == app.profile => {
            let selected = app
                .snapshot
                .as_ref()
                .and_then(|s| s.proposals.get(app.selected_proposal))
                .map(|p| p.id);
            if selected == Some(value.id) {
                app.detail = Some(value);
            }
        }
        Event::SelectPreset(delta) if !app.running && !app.presets.is_empty() => {
            app.wizard.preset = (app.wizard.preset as isize + delta)
                .rem_euclid(app.presets.len() as isize) as usize;
            let p = &app.presets[app.wizard.preset];
            app.output = None;
            app.wizard.contract.clone_from(&p.contract);
            app.wizard.implementation.clone_from(&p.implementation);
            app.wizard.init.clone_from(&p.init);
            app.wizard.upgrade = p.upgrade;
            app.wizard.bundle = "contracts".into();
            app.wizard.confirmation = None;
        }
        Event::PrepareRun if !app.running && !app.preparing => {
            if app.profile == Profile::Final {
                app.error = Some(
                    "Final gate is read-only: recorded evidence only; use Demo gate for writes"
                        .into(),
                );
            } else {
                app.error = None;
                app.preparing = true;
                return vec![Action::Prepare(app.wizard.clone())];
            }
        }
        Event::Prepared(wizard, text) => {
            app.preparing = false;
            if app.profile == Profile::Demo && !app.running && app.wizard == wizard {
                app.wizard.confirmation = Some(text);
            }
        }
        Event::StartRun
            if app.profile == Profile::Demo
                && !app.running
                && app.wizard.confirmation.is_some() =>
        {
            app.running = true;
            // The child may reach a write before its stdout notification is consumed.
            // Conservatively require confirmation from launch, never miss an in-flight write.
            app.write_sent = true;
            app.cancelling = false;
            app.elapsed_ms = 0;
            app.output = None;
            app.run_log.clear();
            app.error = None;
            app.run_started_at = (app.now_ns != 0).then(|| utc_from_ns(app.now_ns));
            app.run_finished_at = None;
            app.toast(
                ToastLevel::Info,
                "Launching the authorized demo flow".into(),
            );
            for step in &mut app.steps {
                step.state = StepState::Pending;
                step.detail.clear();
                step.elapsed_ms = 0;
                step.started_at = None;
                step.finished_at = None;
            }
            app.wizard.confirmation = None;
            return vec![Action::Run(app.wizard.clone())];
        }
        Event::Stage {
            index,
            state,
            detail,
            elapsed_ms,
        } => {
            if let Some(step) = app.steps.get_mut(index) {
                if state == StepState::Active && step.started_at.is_none() && app.now_ns != 0 {
                    step.started_at = Some(utc_from_ns(app.now_ns));
                }
                if matches!(
                    state,
                    StepState::Pass | StepState::Refused | StepState::Error | StepState::NotRun
                ) && step.finished_at.is_none()
                    && app.now_ns != 0
                {
                    step.finished_at = Some(utc_from_ns(app.now_ns));
                }
                step.state = state;
                step.detail = detail;
                step.elapsed_ms = elapsed_ms;
            }
            if matches!(state, StepState::Refused | StepState::Error) {
                for step in app.steps.iter_mut().skip(index + 1) {
                    if step.state == StepState::Pending {
                        step.state = StepState::NotRun;
                    }
                }
            }
        }
        Event::WriteSent => app.write_sent = true,
        Event::Log(line) => {
            app.run_log.push(line);
            if app.run_log.len() > 100 {
                app.run_log.remove(0);
            }
        }
        Event::RunStarted(path) => {
            app.output = Some(path);
            app.write_sent = true;
        }
        Event::RunFinished(result) => {
            app.running = false;
            app.confirm_cancel = false;
            app.run_finished_at = (app.now_ns != 0).then(|| utc_from_ns(app.now_ns));
            app.toast(
                if result.is_ok() {
                    ToastLevel::Success
                } else {
                    ToastLevel::Error
                },
                result.as_ref().map_or_else(
                    |e| e.clone(),
                    |()| "Demo flow verified its observed outcome".into(),
                ),
            );
            for step in &mut app.steps {
                if matches!(step.state, StepState::Pending | StepState::Active) {
                    step.state = if result.is_err() && step.state == StepState::Active {
                        StepState::Error
                    } else {
                        StepState::Pending
                    };
                    if step.detail.is_empty() {
                        step.detail = "No completion observed; inspect retained artifacts".into();
                    }
                }
            }
            if let Err(e) = result {
                app.error = Some(e);
            }
        }
        Event::Tick(elapsed) => {
            let delta = elapsed.saturating_sub(app.elapsed_ms);
            for step in &mut app.steps {
                if step.state == StepState::Active {
                    step.elapsed_ms = step.elapsed_ms.saturating_add(delta);
                }
            }
            app.elapsed_ms = elapsed;
        }
        Event::Cancel if app.running && !app.cancelling => {
            if app.write_sent {
                app.confirm_cancel = true;
            } else {
                app.cancelling = true;
                return vec![Action::CancelRun];
            }
        }
        Event::ConfirmCancel if app.confirm_cancel => {
            app.confirm_cancel = false;
            app.cancelling = true;
            return vec![Action::CancelRun];
        }
        Event::Audit if !app.auditing => {
            app.auditing = true;
            app.error = None;
            return vec![Action::Audit(app.profile)];
        }
        Event::Audited(profile, result) if profile == app.profile => {
            app.audit = Some(result);
            app.auditing = false;
            let exit = app.audit.as_ref().map_or(1, |a| a.exit);
            app.toast(
                if exit == 0 {
                    ToastLevel::Success
                } else {
                    ToastLevel::Error
                },
                format!("Audit completed with exit {exit}"),
            );
        }
        Event::FetchReceipt if !app.receipt_loading => match app.receipt_id.parse() {
            Ok(id) => {
                app.receipt_loading = true;
                app.receipt = None;
                app.error = None;
                return vec![Action::Receipt(app.profile, id)];
            }
            Err(_) => app.error = Some("Enter a numeric proposal id, then press Enter".into()),
        },
        Event::Receipt(profile, value)
            if profile == app.profile
                && app.receipt_id.parse::<u64>().ok() == value["proposal"]["id"].as_u64() =>
        {
            app.receipt = Some(value);
            app.receipt_loading = false;
            app.receipt_scroll = 0;
            app.toast(ToastLevel::Success, "Observed receipt assembled".into());
        }
        Event::SaveReceipt => {
            if let Some(value) = &app.receipt {
                return vec![Action::SaveReceipt(app.receipt_path.clone(), value.clone())];
            }
        }
        Event::Saved(path) => app.toast(
            ToastLevel::Success,
            format!("Saved observed receipt to {path}"),
        ),
        _ => {}
    }
    Vec::new()
}
