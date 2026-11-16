//! `every-teammate-is-a-bot.test.tsx`, carried: one Electron case, one test, named after it, with Electron's values.
//!
//! The pure half of the file lives here (the shapes, how a bot is derived, picked, shuffled and validated, and how a
//! bot moves), and the dialog's colour list. The dialog's two cases (every shape and the face; Shuffle once, with the
//! look) draw the window's look picker: `look_picker::tests` in the window crate. Not carried yet: the three cases that
//! read a teammate bot's React markup (no Rust test checks them).
//! Copied from Hydrogen's uncommitted work.

use crate::bot_rig::{bot_motion, BotMotionLevel, FaceActivity};
use crate::bot_spec::{
    bot_for, clean_avatar, is_valid_bot_shape, seed_avatar, shuffled_avatar, AvatarSpec, BotFace, BotSpec, PetRef,
    PetSource, ACCESSORY_COUNT, BOT_AVATAR_PRESETS, BOT_SHAPES, HEADWEAR_COUNT, HUES, MOUTH_COUNT,
};
use std::collections::BTreeSet;

fn picked_hopper() -> AvatarSpec {
    AvatarSpec {
        bot: Some(BotSpec { shape: "hopper".into(), face: BotFace::Mouth, screen: None }),
        ..seed_avatar("tm_wren")
    }
}

// --- the shapes ---

#[test]
fn are_the_librarys_eighteen_then_locusts_own_two_then_the_three_that_nod_to_the_agents() {
    let library: Vec<&str> = BOT_AVATAR_PRESETS.iter().map(|p| p.id).collect();
    assert_eq!(library.len(), 18);
    assert_eq!(BOT_SHAPES[..18].to_vec(), library);
    assert_eq!(BOT_SHAPES[18..].to_vec(), vec!["hopper", "swarm", "critter", "prompt", "spark"]);
}

// --- a teammate's bot ---

#[test]
fn is_derived_from_the_seeded_look_so_every_teammate_made_before_bots_has_one_that_never_changes() {
    let look = seed_avatar("tm_wren");
    assert_eq!(bot_for(&look), bot_for(&seed_avatar("tm_wren")));
}

#[test]
fn can_be_any_of_the_first_twenty_from_the_looks_the_seed_can_make_and_a_new_shape_moves_no_one() {
    let mut reached = BTreeSet::new();
    for headwear in 0..HEADWEAR_COUNT {
        for accessory in 0..ACCESSORY_COUNT {
            for mouth in 0..MOUTH_COUNT {
                let look = AvatarSpec { headwear, accessory, mouth, bot: None, pet: None };
                reached.insert(bot_for(&look).shape);
            }
        }
    }
    let first_twenty: BTreeSet<String> = BOT_SHAPES[..20].iter().map(|s| (*s).to_owned()).collect();
    assert_eq!(reached, first_twenty);
}

#[test]
fn is_the_same_derived_bot_it_was_before_the_critter_and_the_prompt_joined() {
    // Measured on 0.558 for these seeds; a shape added to the list must not move them.
    let before = |id: &str| {
        let look = seed_avatar(id);
        let index = (usize::from(look.headwear) * usize::from(ACCESSORY_COUNT) * usize::from(MOUTH_COUNT)
            + usize::from(look.accessory) * usize::from(MOUTH_COUNT)
            + usize::from(look.mouth))
            % 20;
        BOT_SHAPES[index]
    };
    for id in ["tm_wren", "tm_atlas", "tm_clay", "tm_sol", "tm_gem"] {
        assert_eq!(bot_for(&seed_avatar(id)).shape, before(id), "{id}");
    }
}

#[test]
fn is_the_one_the_person_picked_when_they_picked_one() {
    let picked = picked_hopper();
    assert_eq!(bot_for(&picked), BotSpec { shape: "hopper".into(), face: BotFace::Mouth, screen: None });
}

#[test]
fn gives_way_to_the_derived_one_on_a_shuffle_so_shuffling_moves_through_the_bots() {
    let next = shuffled_avatar(&picked_hopper());
    assert!(next.bot.is_none());
    assert_ne!(bot_for(&next).shape, bot_for(&shuffled_avatar(&next)).shape);
}

#[test]
fn is_validated_and_nothing_but_the_known_fields_reaches_disk() {
    // `isAvatarSpec` has no twin: the shape and face checks it makes are `is_valid_bot_shape` and `BotFace::from_str`
    // (what `native::parse_looks` reads a record through), and what reaches disk is `clean_avatar`.
    assert!(is_valid_bot_shape("ghost"));
    assert_eq!(BotFace::from_str("eyes"), Some(BotFace::Eyes));
    assert!(!is_valid_bot_shape("robot"));
    assert_eq!(BotFace::from_str("nose"), None);
    // The record carries a screen flag Electron's spec also keeps, and a pet; nothing else exists on the type to leak.
    let with_extras = AvatarSpec {
        headwear: 1,
        accessory: 0,
        mouth: 0,
        bot: Some(BotSpec { shape: "ghost".into(), face: BotFace::Eyes, screen: None }),
        pet: Some(PetRef { source: PetSource::Gallery, id: "codex-buddy".into(), screen: Some(false) }),
    };
    assert_eq!(
        clean_avatar(&with_extras),
        AvatarSpec {
            headwear: 1,
            accessory: 0,
            mouth: 0,
            bot: Some(BotSpec { shape: "ghost".into(), face: BotFace::Eyes, screen: None }),
            pet: Some(PetRef { source: PetSource::Gallery, id: "codex-buddy".into(), screen: Some(false) }),
        }
    );
    let plain = AvatarSpec { headwear: 1, accessory: 0, mouth: 0, bot: None, pet: None };
    assert_eq!(clean_avatar(&plain), plain);
}

// --- how a bot moves ---

#[test]
fn only_work_moves_and_a_finish_hops_once_idle_and_blocked_keep_still_at_full_motion_working_hops_the_rest_look_around() {
    let all = [
        FaceActivity::Thinking,
        FaceActivity::Working,
        FaceActivity::Delegating,
        FaceActivity::Responding,
        FaceActivity::Waiting,
        FaceActivity::Receiving,
        FaceActivity::Blocked,
        FaceActivity::Done,
        FaceActivity::Idle,
    ];
    let summary: Vec<(&str, &str)> = all
        .iter()
        .map(|activity| {
            let motion = bot_motion(*activity, BotMotionLevel::Full);
            let said = if motion.hops {
                "hop"
            } else if motion.paused {
                "still"
            } else {
                motion.state.as_str()
            };
            (activity.as_str(), said)
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("thinking", "default"),
            ("working", "working"),
            ("delegating", "working"),
            ("responding", "default"),
            ("waiting", "default"),
            ("receiving", "default"),
            ("blocked", "still"),
            ("done", "hop"),
            ("idle", "still"),
        ]
    );
}

// --- the new teammate dialog: the part that is data ---

#[test]
fn has_eleven_colours_indigo_and_coral() {
    // The list the dialog offers; the drawn radios are `look_picker::tests::has_eleven_colours_indigo_and_coral_…`.
    assert_eq!(HUES.len(), 11);
    assert_eq!(&HUES[9..], &["indigo", "coral"]);
}
