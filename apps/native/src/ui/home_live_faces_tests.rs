//! R146: Home teammate cards draw live animated faces and pets on `BotStage` with `Subtle` motion
//! instead of static stills.

use super::*;
use locust_core::bot_rig::FaceActivity;
use locust_core::bot_spec::{PetRef, PetSource};
use locust_core::native::FaceLook;

#[test]
fn home_teammate_cards_register_live_subtle_faces_and_animate_when_working() {
    let mut s = State::default();
    crate::team_scene(&mut s);
    // argon is busy (Working), codec is idle (Idle) in team_scene.
    let mut rt = rune::Runtime::new(
        s,
        |s, _| home_view(s, &Palette::new(true)),
        (1215.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);

    // Live bot slots registered (never static `bot-static-*`).
    assert!(rt.find("bot-home-argon").is_some(), "working teammate uses live bot canvas");
    assert!(rt.find("bot-home-codec").is_some(), "idle teammate uses live bot canvas");
    assert!(
        !rt.elements()
            .iter()
            .any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("bot-static-"))),
        "Home cards draw no static faces"
    );
    assert_eq!(rt.state.bot_stage.desired_activity("home-argon"), Some(FaceActivity::Working));
    assert_eq!(rt.state.bot_stage.desired_activity("home-codec"), Some(FaceActivity::Idle));

    // Stepping the production BotStage advances the working teammate's pose while idle stays still.
    let mut tx = rune::Tx::default();
    rt.state.bot_stage.poll(0.0, false, true, &mut tx);
    let argon0 = format!("{:?}", crate::look::faces::drawn_shape("home-argon").unwrap());
    let codec0 = format!("{:?}", crate::look::faces::drawn_shape("home-codec").unwrap());
    rt.state.bot_stage.poll(0.345, false, true, &mut tx);
    let argon1 = format!("{:?}", crate::look::faces::drawn_shape("home-argon").unwrap());
    let codec1 = format!("{:?}", crate::look::faces::drawn_shape("home-codec").unwrap());
    assert_ne!(argon0, argon1, "working Home face animates across stage frames");
    assert_eq!(codec0, codec1, "idle Home face stays at rest");
    assert_eq!(rt.state.bot_stage.last_hop("home-argon"), Some(false), "Home card uses subtle motion, not full hop");
}

#[test]
fn home_teammate_wearing_codex_buddy_draws_live_buddy_with_screen_and_respects_as_drawn() {
    let make = |screen: Option<bool>| {
        let mut s = State::default();
        crate::team_scene(&mut s);
        s.settings_panel.settings.terminal_faces = true;
        s.looks.insert(
            "codec".into(),
            FaceLook {
                hue: "blue".into(),
                parts: None,
                bot: None,
                bot_screen: None,
                pet: Some(PetRef {
                    source: PetSource::Gallery,
                    id: "codex-buddy".into(),
                    screen,
                }),
            },
        );
        let mut rt = rune::Runtime::new(
            s,
            |s, _| home_view(s, &Palette::new(true)),
            (1215.0, 900.0),
            rune::Env::new(true, 1.0),
        );
        rt.frame(0.0);
        rt.state
            .bot_stage
            .buddy_snapshot("home-codec")
            .expect("Codex Buddy registered live on BotStage for Home card")
    };
    assert!(make(None).screen, "Codex Buddy wears his terminal screen on Home by default");
    assert!(!make(Some(false)).screen, "As drawn turns his screen off on Home");
}
