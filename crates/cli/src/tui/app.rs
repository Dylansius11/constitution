//! Pure application state and reducer. Filesystem/network/process work belongs to data.rs.
use serde_json::Value;

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
    pub changes: Vec<Value>,
    pub proposals: Vec<Value>,
    pub verifier: String,
    pub identity: Option<Value>,
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
    pub detail: Option<Value>,
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
            wizard: Wizard::default(),
            steps: STAGES
                .iter()
                .map(|name| Step {
                    name,
                    state: StepState::Pending,
                    detail: String::new(),
                    elapsed_ms: 0,
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
#[derive(Debug)]
pub enum Action {
    Refresh(Profile),
    Audit(Profile),
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
    Detail(Profile, Value),
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
}
pub fn update(app: &mut App, event: Event) -> Vec<Action> {
    match event {
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
            return vec![Action::Refresh(app.profile)];
        }
        Event::Refresh if !app.refreshing => {
            app.refreshing = true;
            return vec![Action::Refresh(app.profile)];
        }
        Event::Snapshot(profile, snapshot) if profile == app.profile => {
            app.snapshot = Some(snapshot);
            app.refreshing = false;
        }
        Event::Failure(error) => {
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
                    .and_then(|p| p["id"].as_u64())
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
                .and_then(|p| p["id"].as_u64());
            if selected.is_some() && selected == value["proposal"]["id"].as_u64() {
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
            for step in &mut app.steps {
                step.state = StepState::Pending;
                step.detail.clear();
                step.elapsed_ms = 0;
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
        }
        Event::SaveReceipt => {
            if let Some(value) = &app.receipt {
                return vec![Action::SaveReceipt(app.receipt_path.clone(), value.clone())];
            }
        }
        Event::Saved(path) => app.error = Some(format!("Saved observed receipt to {path}")),
        _ => {}
    }
    Vec::new()
}
