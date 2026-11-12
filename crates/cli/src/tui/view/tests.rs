use super::render;
use crate::tui::app::*;
use constitution_types::ProposalState;
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;

const V1: &str = "0x48879e182efbf2db3633088073017e8239eb1457";
const V2: &str = "0xcdff74f564ebdfb387d7f4ec4e0ca0f4a8753005";
const V3: &str = "0xaeed3e98706f4a4a6c1c5062c1f6541a595db4be";

fn hash(byte: &str) -> String {
    format!("0x{}", byte.repeat(32))
}

fn proposal(id: u64, state: ProposalState, refusal: Option<(u8, u16)>) -> ProposalView {
    ProposalView {
        id,
        proposer: "putra-dev.testnet".into(),
        state,
        refusal: refusal.map(|(r, c)| Refusal::new(r, c)),
        created_at: "2026-11-12T03:11:09Z".into(),
        expires_at: "2026-11-12T04:11:09Z".into(),
        new_impl: V2.into(),
        codehash: hash("ab"),
        expected_current_impl: V1.into(),
        init_calldata: "0x".into(),
        source_commitment: hash("cd"),
        certificate_hash: Some(hash("ef")),
        evidence_root: None,
        evm_tx_hash: None,
        evm_status: None,
        gate_url: "https://testnet.nearblocks.io/address/constitution-demo.testnet".into(),
        new_impl_url: format!("https://sepolia.basescan.org/address/{V2}"),
        current_impl_url: format!("https://sepolia.basescan.org/address/{V1}"),
        evm_tx_url: None,
        raw: json!({"proposal": {"id": id}, "state": "Rejected", "certificate": {"verdict": "reject"}}),
    }
}

fn preset(id: &str, title: &str, upgrade: bool, implementation: &str, rule: Option<u64>) -> Preset {
    Preset {
        id: id.into(),
        title: title.into(),
        contract: "src/demo/VaultV2Honest.sol:VaultV2Honest".into(),
        implementation: implementation.into(),
        init: "0x".into(),
        rule,
        upgrade,
        requires_impl: upgrade.then(|| if id == "V3" { V2 } else { V1 }.to_owned()),
        previous_run: None,
    }
}

fn fixture() -> App {
    let mut app = App {
        now_ns: 1_762_920_000_000_000_000,
        presets: vec![
            preset("a1", "Break the storage layout", false, V3, Some(1)),
            preset(
                "a2",
                "Drain the vault with an insolvent upgrade",
                false,
                V3,
                Some(2),
            ),
            preset("V2", "Honest V2", true, V2, None),
            preset("V3", "Honest V3", true, V3, None),
        ],
        snapshot: Some(Snapshot {
            observed_at: "2026-11-12T04:58:17.78Z".into(),
            gate: "constitution-demo.testnet".into(),
            access_keys: Some(0),
            rule_hash: Some(hash("a9")),
            manifest_hash: Some(hash("a9")),
            implementation: Some(V1.into()),
            delay_days: Some(1),
            proposals: vec![
                proposal(0, ProposalState::Rejected, Some((2, 20))),
                proposal(1, ProposalState::Rejected, Some((1, 10))),
                proposal(2, ProposalState::Signed, None),
            ],
            verifier: "online — identity endpoint observed, NOT an attestation appraisal".into(),
            identity: Some(VerifierIdentity {
                verifier_pubkey: Some(hash("24")),
                release_hash: Some(hash("fe")),
                compose_hash: Some(hash("f7")),
                recorded: false,
                observed_at: Some("2026-11-12T04:58:17Z".into()),
            }),
            gate_url: "https://testnet.nearblocks.io/address/constitution-demo.testnet".into(),
            proxy_url:
                "https://sepolia.basescan.org/address/0xc24a9b7b139af4e458df40a2644229a4ca5fec91"
                    .into(),
            ..Snapshot::default()
        }),
        appraisal: Some(AppraisalResult {
            checks: ["identity", "dcap_signature_chain", "compose_hash"]
                .iter()
                .map(|n| AppraisalCheck {
                    name: (*n).into(),
                    ok: true,
                    detail: "verified against the Intel production root".into(),
                })
                .collect(),
            observed_at: Some("2026-11-12T04:58:14Z".into()),
            recorded: false,
            recorded_date: None,
        }),
        ..App::default()
    };
    app.wizard.account = "putra-dev.testnet".into();
    app.wizard.key_file = ".secrets/near-credentials/testnet/putra-dev.testnet.json".into();
    app.wizard.implementation = V3.into();
    app
}

fn draw(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| render(f, f.area(), app)).unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(usize::from(width))
        .map(|row| {
            row.iter()
                .map(|cell| cell.symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn refused_run(app: &mut App) {
    app.output = Some("scratch/tui-runs/attack-20261112T050000Z".into());
    app.elapsed_ms = 14_200;
    for (i, (state, ms, detail)) in [
        (StepState::Pass, 400, "source registered"),
        (StepState::Pass, 6_100, "bundle built"),
        (
            StepState::Refused,
            7_700,
            "R2/20: Bounded symbolic solvency counterexample",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        app.steps[i].state = state;
        app.steps[i].elapsed_ms = ms;
        app.steps[i].detail = detail.into();
    }
    for step in app.steps.iter_mut().skip(3) {
        step.state = StepState::NotRun;
    }
}

#[test]
fn every_screen_survives_every_terminal_size() {
    let mut app = fixture();
    app.audit = Some(AuditResult {
        checks: vec![
            AuditCheck {
                name: "gate_has_no_access_keys".into(),
                ok: true,
                warning: false,
                detail: "0 access keys".into(),
            },
            AuditCheck {
                name: "pending_changes".into(),
                ok: true,
                warning: true,
                detail: "one governance change matures in 28 hours and so on and on".into(),
            },
        ],
        exit: 0,
        observed_at: "2026-11-12T04:58:17Z".into(),
    });
    app.receipt = Some(json!({"schema": "constitution.receipt/1", "proposal": {"id": 2}}));
    for (width, height) in [(160, 45), (120, 34), (100, 30), (80, 24), (50, 14), (20, 6)] {
        for screen in Screen::ALL {
            app.screen = screen;
            for (running, help, confirm) in [
                (false, false, false),
                (true, false, false),
                (false, true, false),
                (false, false, true),
            ] {
                app.running = running;
                app.help = help;
                app.confirm_cancel = confirm;
                app.wizard.editing = running;
                app.wizard.confirmation = None;
                if running {
                    app.output = Some("scratch/tui-runs/x".into());
                    app.steps[1].state = StepState::Active;
                }
                draw(&app, width, height);
            }
            app.output = None;
            app.running = false;
            app.help = false;
            app.confirm_cancel = false;
            app.wizard.confirmation = Some("Sign as a.testnet for gate g.testnet (testnet only).\nCandidate: 0x00\nPress y only if authorized".into());
            draw(&app, width, height);
            app.wizard.confirmation = None;
            app.snapshot = None;
            draw(&app, width, height);
            app.snapshot = fixture().snapshot;
        }
    }
}

#[test]
fn overview_states_what_the_gate_guarantees_and_which_version_runs() {
    let mut app = fixture();
    app.screen = Screen::Overview;
    let text = draw(&app, 140, 40);
    assert!(text.contains("● KEYLESS"), "{text}");
    assert!(text.contains("V1 (initial)"), "{text}");
    assert!(text.contains("MATCH"), "{text}");
    assert!(text.contains("3 · 2 refused · 1 signed"), "{text}");
    app.snapshot.as_mut().unwrap().access_keys = Some(2);
    let text = draw(&app, 140, 40);
    assert!(
        text.contains("2 ACCESS KEYS") && !text.contains("● KEYLESS"),
        "{text}"
    );
}

#[test]
fn a_refusal_is_reported_as_the_system_working_not_as_an_error() {
    let mut app = fixture();
    app.screen = Screen::Run;
    refused_run(&mut app);
    let text = draw(&app, 120, 34);
    assert!(text.contains("REFUSED"), "{text}");
    assert!(text.contains("at Rules R1–R5"), "{text}");
    assert!(text.contains("no signature produced"), "{text}");
    assert!(!text.contains("STOPPED"), "{text}");

    app.steps[2].state = StepState::Error;
    let text = draw(&app, 120, 34);
    assert!(
        text.contains("STOPPED") && text.contains("never create a duplicate"),
        "{text}"
    );
}

#[test]
fn proposal_detail_explains_the_refusal_in_plain_words() {
    let mut app = fixture();
    app.screen = Screen::Proposals;
    let text = draw(&app, 130, 36);
    assert!(text.contains("Rule 2 · reason 20"), "{text}");
    assert!(text.contains("Nothing was signed"), "{text}");
    app.selected_proposal = 2;
    let text = draw(&app, 130, 36);
    assert!(
        text.contains("SIGNED") && text.contains("none · nothing was relayed"),
        "{text}"
    );
}

/// Prints every screen so a person can judge the layout: `cargo test tui_preview -- --ignored --nocapture`.
#[test]
#[ignore = "visual preview for people, prints frames"]
fn tui_preview() {
    let mut app = fixture();
    for (width, height) in [(130, 38), (96, 28)] {
        for screen in Screen::ALL {
            app.screen = screen;
            println!(
                "=== {} {width}x{height} ===\n{}",
                screen.title(),
                draw(&app, width, height)
            );
        }
        app.screen = Screen::Run;
        refused_run(&mut app);
        println!(
            "=== Run (refused) {width}x{height} ===\n{}",
            draw(&app, width, height)
        );
        app.output = None;
        app.help = true;
        println!(
            "=== Help {width}x{height} ===\n{}",
            draw(&app, width, height)
        );
        app.help = false;
        app.wizard.confirmation = Some("Observed at 2026-11-12T05:00:00Z\nSign as putra-dev.testnet for gate constitution-demo.testnet (testnet only).\nCandidate: 0xcdff74f564ebdfb387d7f4ec4e0ca0f4a8753005\nProposal storage fee: 1000 yoctoNEAR (NON-refundable).\nPress y only if authorized to sign.".into());
        println!(
            "=== Review {width}x{height} ===\n{}",
            draw(&app, width, height)
        );
        app.wizard.confirmation = None;
    }
}
