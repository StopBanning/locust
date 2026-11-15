//! Three time-marker cases from missionView, using the same UTC instants.
use crate::time_marker::{thread_markers,TurnTiming};
fn turn(minute:u32,index:usize)->TurnTiming {
    let at=format!("2026-08-31T16:{minute:02}:00.000Z");
    TurnTiming {before_item:index,first_at:Some(at.clone()),last_at:Some(at)}
}
#[test]
fn says_nothing_when_turns_follow_each_other_closely(){assert!(thread_markers(&[turn(0,0),turn(1,1)]).is_empty());}
#[test]
fn marks_a_gap_with_the_elapsed_mission_time_and_how_long_the_wait_was(){
    let markers=thread_markers(&[turn(0,0),turn(40,1)]);
    assert_eq!(markers.len(),1);
    assert_eq!(markers[0].before_item,1);
    assert_eq!(markers[0].minutes_in,40);
    assert_eq!(markers[0].note.as_deref(),Some("waited 40 min"));
}
#[test]
fn measures_elapsed_time_from_the_mission_first_event_not_the_previous_turn(){
    assert_eq!(thread_markers(&[turn(0,0),turn(10,1),turn(30,2)]).iter().map(|m|m.minutes_in).collect::<Vec<_>>(),vec![10,30]);
}
