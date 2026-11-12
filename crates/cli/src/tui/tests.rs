use super::app::*;

#[test]
fn tui_navigation_and_back() {
    let mut app = App::default();
    update(&mut app, Event::NextScreen);
    assert_eq!(app.screen, Screen::Proposals);
    update(&mut app, Event::Navigate(Screen::Receipt));
    assert_eq!(app.screen, Screen::Receipt);
    update(&mut app, Event::Back);
    assert_eq!(app.screen, Screen::Overview);
    update(&mut app, Event::Back);
    assert!(app.quit);
}

#[test]
fn tui_final_profile_disables_run() {
    let mut app = App::default();
    update(&mut app, Event::SwitchProfile);
    assert_eq!(app.profile, Profile::Final);
    assert!(update(&mut app, Event::PrepareRun).is_empty());
    assert!(app.error.as_ref().unwrap().contains("read-only"));
}

#[test]
fn tui_wizard_rejects_missing_key_file() {
    let mut wizard = Wizard {
        account: "operator.testnet".into(),
        key_file: "missing.json".into(),
        implementation: "0x1111111111111111111111111111111111111111".into(),
        ..Wizard::default()
    };
    assert!(wizard.validate(false).unwrap_err().contains("key file"));
    wizard.key_file = "present.json".into();
    assert!(wizard.validate(true).is_ok());
}

#[test]
fn tui_wizard_rejects_malformed_hex() {
    let mut wizard = Wizard {
        account: "operator.testnet".into(),
        key_file: "present.json".into(),
        implementation: "0xno".into(),
        ..Wizard::default()
    };
    assert!(
        wizard
            .validate(true)
            .unwrap_err()
            .contains("implementation")
    );
    wizard.implementation = "0x1111111111111111111111111111111111111111".into();
    wizard.init = "0x1".into();
    assert!(wizard.validate(true).unwrap_err().contains("calldata"));
}

#[test]
fn tui_stepper_refusal_leaves_later_stages_not_run() {
    let mut app = App::default();
    update(
        &mut app,
        Event::Stage {
            index: 2,
            state: StepState::Active,
            detail: "running".into(),
            elapsed_ms: 12,
        },
    );
    update(
        &mut app,
        Event::Stage {
            index: 2,
            state: StepState::Refused,
            detail: "R2 / 20".into(),
            elapsed_ms: 50,
        },
    );
    assert_eq!(app.steps[2].state, StepState::Refused);
    assert_eq!(app.steps[3].state, StepState::NotRun);
    assert_eq!(app.steps[2].elapsed_ms, 50);
}

#[test]
fn tui_stepper_error_preserves_observed_message() {
    let mut app = App::default();
    update(
        &mut app,
        Event::Stage {
            index: 0,
            state: StepState::Error,
            detail: "RPC unavailable; refresh".into(),
            elapsed_ms: 40,
        },
    );
    assert_eq!(app.steps[0].state, StepState::Error);
    assert_eq!(app.steps[0].detail, "RPC unavailable; refresh");
}

#[test]
fn tui_applied_upgrade_unavailable() {
    let preset = Preset {
        id: "v2".into(),
        title: "Honest V2".into(),
        contract: "src/demo/VaultV2Honest.sol:VaultV2Honest".into(),
        implementation: "0x1111111111111111111111111111111111111111".into(),
        init: "0x".into(),
        rule: None,
        upgrade: true,
        requires_impl: None,
        previous_run: None,
    };
    assert_eq!(
        preset.availability(Some(&preset.implementation)),
        Availability::Done
    );
    assert_eq!(preset.availability(None), Availability::Unknown);
}

#[test]
fn tui_cancel_after_write_requires_confirmation() {
    let mut app = App {
        running: true,
        write_sent: true,
        ..App::default()
    };
    assert!(update(&mut app, Event::Cancel).is_empty());
    assert!(app.confirm_cancel);
    assert!(matches!(
        update(&mut app, Event::ConfirmCancel).as_slice(),
        [Action::CancelRun]
    ));
}

#[test]
fn tui_stale_confirmation_cannot_authorize_changed_inputs() {
    let mut app = App::default();
    let original = app.wizard.clone();
    app.wizard.implementation = "0x1111111111111111111111111111111111111111".into();
    update(&mut app, Event::Prepared(original, "old inputs".into()));
    assert!(app.wizard.confirmation.is_none());
    assert!(update(&mut app, Event::StartRun).is_empty());
}

#[test]
fn tui_final_profile_rejects_even_existing_confirmation() {
    let mut app = App {
        profile: Profile::Final,
        ..App::default()
    };
    app.wizard.confirmation = Some("previous demo inputs".into());
    assert!(update(&mut app, Event::StartRun).is_empty());
    assert!(!app.running);
}

#[test]
fn tui_unobserved_completion_is_not_fabricated_as_pass_or_not_run() {
    let mut app = App {
        running: true,
        ..App::default()
    };
    update(&mut app, Event::RunFinished(Ok(())));
    assert_eq!(app.steps[1].state, StepState::Pending);
    assert!(app.steps[1].detail.contains("No completion observed"));
}

#[test]
#[ignore = "live read-only NEAR/Base/identity endpoint smoke; run explicitly from repository root"]
fn tui_live_read_only_render() {
    use ratatui::{Terminal, backend::TestBackend};
    std::env::set_current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("repository root");
    let snapshot = super::data::snapshot(Profile::Demo).expect("read deployment and public state");
    assert!(
        snapshot.errors.is_empty(),
        "live read errors: {:?}",
        snapshot.errors
    );
    assert_eq!(snapshot.gate, "constitution-demo.testnet");
    assert!(snapshot.access_keys.is_some());
    assert!(snapshot.implementation.is_some());
    assert!(
        !snapshot.proposals.is_empty(),
        "demo gate should have observed proposals"
    );
    let mut app = App {
        snapshot: Some(snapshot),
        ..App::default()
    };
    let mut terminal = Terminal::new(TestBackend::new(160, 55)).unwrap();
    for screen in [Screen::Overview, Screen::Proposals] {
        update(&mut app, Event::Navigate(screen));
        terminal
            .draw(|f| super::view::render(f, f.area(), &app))
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .chunks(160)
            .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        println!("=== LIVE {} ===\n{}", screen.title(), text);
        assert!(text.contains("constitution-demo.testnet") || screen == Screen::Proposals);
        assert!(text.contains("Observed at"));
    }
}

#[test]
fn tui_appraisal_success_records_checks_and_toast() {
    let mut app = App::default();
    assert!(matches!(
        update(&mut app, Event::Appraise).as_slice(),
        [Action::Appraise(Profile::Demo)]
    ));
    assert!(app.appraising);
    update(
        &mut app,
        Event::Appraised(
            Profile::Demo,
            AppraisalResult {
                checks: vec![AppraisalCheck {
                    name: "identity".into(),
                    ok: true,
                    detail: "observed".into(),
                }],
                observed_at: Some("2026-11-12T04:00:00Z".into()),
                recorded: false,
                recorded_date: None,
            },
        ),
    );
    assert!(!app.appraising);
    assert!(app.appraisal.as_ref().unwrap().ok());
    assert_eq!(
        app.last_action_result.as_ref().unwrap().level,
        ToastLevel::Success
    );
}

#[test]
fn tui_appraisal_failure_is_not_a_pass() {
    let mut app = App::default();
    update(
        &mut app,
        Event::Appraised(
            Profile::Demo,
            AppraisalResult {
                checks: vec![AppraisalCheck {
                    name: "compose_hash".into(),
                    ok: false,
                    detail: "mismatch".into(),
                }],
                observed_at: Some("2026-11-12T04:00:00Z".into()),
                recorded: false,
                recorded_date: None,
            },
        ),
    );
    assert!(!app.appraisal.as_ref().unwrap().ok());
    assert_eq!(
        app.last_action_result.as_ref().unwrap().level,
        ToastLevel::Error
    );
    assert!(app.error.as_ref().unwrap().contains("mismatch"));
}

#[test]
fn tui_final_appraisal_never_dispatches_a_probe() {
    let mut app = App {
        profile: Profile::Final,
        ..App::default()
    };
    assert!(update(&mut app, Event::Appraise).is_empty());
    assert!(!app.appraising);
}

#[test]
fn tui_eta_countdown_matures_at_exact_boundary() {
    let change = PendingChangeView {
        id: 0,
        kind: "SetParams".into(),
        status: "Pending".into(),
        eta_ns: 2_000_000_001,
        eta_utc: utc_from_ns(2_000_000_001),
        remaining_ns: 0,
        remaining_seconds: 0,
        matured: false,
        raw: serde_json::Value::Null,
    };
    let mut app = App {
        snapshot: Some(Snapshot {
            changes: vec![change],
            ..Snapshot::default()
        }),
        ..App::default()
    };
    update(&mut app, Event::Clock(2_000_000_000));
    assert_eq!(app.snapshot.as_ref().unwrap().changes[0].remaining_ns, 1);
    assert_eq!(
        app.snapshot.as_ref().unwrap().changes[0].remaining_seconds,
        1
    );
    assert!(!app.snapshot.as_ref().unwrap().changes[0].matured);
    update(&mut app, Event::Clock(2_000_000_001));
    assert_eq!(
        app.snapshot.as_ref().unwrap().changes[0].remaining_seconds,
        0
    );
    assert!(app.snapshot.as_ref().unwrap().changes[0].matured);
    update(&mut app, Event::Clock(3_000_000_000));
    assert_eq!(app.snapshot.as_ref().unwrap().changes[0].remaining_ns, 0);
}

#[test]
fn tui_raw_json_toggle_round_trips() {
    let mut app = App::default();
    update(&mut app, Event::ToggleRawJson);
    assert!(app.show_raw_json);
    update(&mut app, Event::ToggleRawJson);
    assert!(!app.show_raw_json);
}

#[test]
fn tui_stage_observation_timestamps_do_not_move_on_repeated_results() {
    let mut app = App::default();
    update(&mut app, Event::Clock(1_000_000_000));
    update(
        &mut app,
        Event::Stage {
            index: 0,
            state: StepState::Active,
            detail: "observed".into(),
            elapsed_ms: 0,
        },
    );
    update(&mut app, Event::Clock(3_000_000_000));
    update(
        &mut app,
        Event::Stage {
            index: 0,
            state: StepState::Pass,
            detail: "registered".into(),
            elapsed_ms: 2000,
        },
    );
    update(&mut app, Event::Clock(5_000_000_000));
    update(
        &mut app,
        Event::Stage {
            index: 0,
            state: StepState::Pass,
            detail: "registered".into(),
            elapsed_ms: 2000,
        },
    );
    assert_eq!(
        app.steps[0].started_at.as_deref(),
        Some("1970-01-01T00:00:01Z")
    );
    assert_eq!(
        app.steps[0].finished_at.as_deref(),
        Some("1970-01-01T00:00:03Z")
    );
}

#[test]
#[ignore = "explicit read-only Intel collateral / pinned demo verifier appraisal"]
fn tui_live_pinned_appraisal_read_only() {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut workers = super::data::Workers::default();
    let mut app = App::default();
    update(&mut app, Event::Clock(super::data::unix_ns()));
    for action in update(&mut app, Event::Appraise) {
        workers.dispatch(action, &tx);
    }
    let event = rx
        .recv_timeout(std::time::Duration::from_secs(180))
        .expect("bounded live appraisal reply");
    update(&mut app, event);
    let result = app.appraisal.as_ref().expect("structured appraisal result");
    for check in &result.checks {
        println!(
            "{} {}: {}",
            if check.ok { "PASS" } else { "FAIL" },
            check.name,
            check.detail
        );
    }
    println!("Appraisal observed at {:?}", result.observed_at);
    assert_eq!(result.checks.len(), 7);
    assert!(result.ok(), "live appraisal failed: {:?}", result.checks);
    assert!(!result.recorded);
}
