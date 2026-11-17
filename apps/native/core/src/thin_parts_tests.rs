//! `each-thin-part-is-its-own-solid.test.ts`: a shape's thin parts are drawn one solid each.

use crate::bot_spec::{BOT_AVATAR_PRESETS, locust_bot, part_pieces};

fn mech() -> &'static str {
    BOT_AVATAR_PRESETS.iter().find(|p| p.id == "mech").unwrap().parts
}

#[test]
fn are_split_into_one_piece_per_antenna_leg_or_wing() {
    assert!(part_pieces(mech()).len() > 1);
    assert_eq!(part_pieces(&locust_bot("swarm").unwrap().parts).len(), 6);
    assert_eq!(part_pieces(&locust_bot("hopper").unwrap().parts).len(), 6);
    assert_eq!(part_pieces(&locust_bot("critter").unwrap().parts).len(), 4);
    assert_eq!(part_pieces(&locust_bot("prompt").unwrap().parts).len(), 0);
}

#[test]
fn lose_nothing_in_the_split() {
    let squash = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
    for parts in [mech().to_owned(), locust_bot("swarm").unwrap().parts.clone(), locust_bot("hopper").unwrap().parts.clone()] {
        assert_eq!(squash(&part_pieces(&parts).concat()), squash(&parts));
    }
}

#[test]
fn every_piece_starts_with_a_move() {
    for key in ["hopper", "swarm", "critter", "prompt", "spark"] {
        for piece in part_pieces(&locust_bot(key).unwrap().parts) {
            assert!(piece.starts_with('M'), "{key}: {}", &piece[..piece.len().min(20)]);
        }
    }
}
