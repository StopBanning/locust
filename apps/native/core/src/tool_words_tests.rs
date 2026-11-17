//! Electron's `a-file-row-says-a-word.test.ts` (ActivityCard.tsx `fileToolWord`) and
//! `a-folded-row-says-it-in-words.test.ts` (missionView.ts `foldedToolsLead`). Native's folded row takes its verb from
//! `tool_kind`, so each fold case goes through it as the window's fold does.

use crate::activity_entries::file_tool_word;
use crate::tool_fold::{folded_tools_lead, tool_kind};

#[test]
fn says_codexs_change_as_a_word() {
    assert_eq!(file_tool_word("file_change"), "changed");
}

#[test]
fn says_a_read_a_search_a_listing_and_a_fetch_in_claude_codes_words_whoever_ran_it() {
    for read in ["Read", "read", "view_file", "read_file"] {
        assert_eq!(file_tool_word(read), "Read", "{read}");
    }
    assert_eq!(file_tool_word("grep_search"), "Grep");
    assert_eq!(file_tool_word("list_dir"), "Glob");
    assert_eq!(file_tool_word("find_by_name"), "Glob");
    assert_eq!(file_tool_word("search_web"), "WebSearch");
    assert_eq!(file_tool_word("read_url_content"), "WebFetch");
}

#[test]
fn says_antigravitys_write_and_edit_in_claude_codes_words() {
    assert_eq!(file_tool_word("write_to_file"), "Write");
    assert_eq!(file_tool_word("replace_file_content"), "Edit");
    assert_eq!(file_tool_word("multi_replace_file_content"), "Edit");
    assert_eq!(file_tool_word("view_file_outline"), "Read");
}

#[test]
fn keeps_a_word_with_no_claude_code_counterpart_as_it_is() {
    assert_eq!(file_tool_word("edit"), "edit");
    assert_eq!(file_tool_word("Write"), "Write");
}

fn lead(count: usize, tool: Option<&str>) -> String {
    let kind = tool.map(tool_kind);
    folded_tools_lead(count, kind.as_deref())
}

#[test]
fn a_folded_group_reads_as_activitys_rows_do_for_each_runtimes_read_tool() {
    assert_eq!(lead(3, Some("read")), "Read 3 files");
    assert_eq!(lead(2, Some("view_file")), "Read 2 files");
    assert_eq!(lead(2, Some("Read")), "Read 2 files");
}

#[test]
fn a_folded_groups_searches_are_counted_as_searches() {
    assert_eq!(lead(4, Some("grep")), "Searched 4 times");
}

#[test]
fn a_folded_tool_with_no_word_of_its_own_is_counted_never_named_in_code() {
    assert_eq!(lead(2, Some("zork_frobnicate")), "2 tool calls");
    assert_eq!(lead(2, None), "2 tool calls");
}
