//! `a-wordless-thought-does-not-start-a-line.test.ts` (both cases).
use crate::ledger::{Item, Status, ToolDetail};
use crate::steps_fold::{fold_steps, group_headline};

fn thought(id: &str, words: &str) -> Item {
    Item::Thought { id: id.into(), text: words.into(), duration_ms: Some(3800) }
}
fn call(command: &str) -> Item {
    Item::Tool { name: "run_command".into(), status: Status::Done, summary: command.into(), detail: Box::new(ToolDetail::default()) }
}

// case: is one line when the thoughts say nothing
#[test]
fn is_one_line_when_the_thoughts_say_nothing() {
    let items = [thought("t1", ""), call("Get-ChildItem"), thought("t2", ""), call("Get-Content a.md"), thought("t3", ""), call("git status")];
    let lines = fold_steps(&items, false);
    assert_eq!(lines.len(), 1);
    // The thinking is still shown, in the line it folded into.
    let thoughts = lines[0].items.iter().filter(|&&i| matches!(items[i], Item::Thought { .. })).count();
    assert_eq!(thoughts, 3);
}

// case: still starts a line at a thought that says something
#[test]
fn still_starts_a_line_at_a_thought_that_says_something() {
    let items = [thought("t1", "Listing the folder"), call("Get-ChildItem"), thought("t2", "Reading the plan"), call("Get-Content a.md")];
    assert_eq!(fold_steps(&items, false).len(), 2);
}

// A message breaks the turn; a call still going is the live line's.
#[test]
fn a_message_breaks_the_line_and_a_running_call_is_left_out() {
    let mut running = call("sleep 60");
    if let Item::Tool { status, .. } = &mut running {
        *status = Status::Running;
    }
    let items = [call("a"), Item::Assistant { item_id: "m".into(), text: "ok".into(), r#final: false }, call("b"), running];
    let lines = fold_steps(&items, true);
    assert_eq!(lines.iter().map(|l| l.items.clone()).collect::<Vec<_>>(), vec![vec![0], vec![2]]);
}

// `a-long-silent-stretch-reads-as-its-moves.test.ts`, first two cases (the third is in thread_words_tests)
#[test]
fn a_new_line_each_time_the_model_thinks_led_by_what_it_thought_about() {
    let items = [
        thought("r1", "**Inspecting the config**\n\nLooking for where the port is set."),
        call("cat config.json"),
        call("rg port src"),
        thought("r2", "**Running the tests**"),
        call("npm test"),
        thought("r3", ""),
        call("npm run build"),
        Item::Assistant { item_id: "m1".into(), text: "Done.".into(), r#final: true },
    ];
    let lines: Vec<_> = fold_steps(&items, false).iter().filter_map(|g| group_headline(&items, g)).collect();
    // r3 says nothing and folds into the line it sits in: it adds no line.
    assert_eq!(lines, vec!["Inspecting the config", "Running the tests"]);
}

#[test]
fn a_turn_with_no_thinking_is_one_line() {
    assert_eq!(fold_steps(&[call("ls"), call("pwd")], false).len(), 1);
}
