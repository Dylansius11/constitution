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
