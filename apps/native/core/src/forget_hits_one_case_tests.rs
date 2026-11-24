//! Electron's `main/forget-hits-one-memory.test.ts` (10 cases): what a `forget` points at, through `forget_target`,
//! which reading a reply's memory block calls for each forget line (Electron's store calls `forgetMatch` the same way).
use super::*;
use serde_json::json;

/// The memories a run in folder `ws_shop` can reach, by their words.
fn stored(texts: &[&str]) -> Vec<Value> {
    texts.iter().map(|t| json!({"text": t, "scope": "workspace", "workspaceId": "ws_shop"})).collect()
}
fn target(quote: &str, texts: &[&str]) -> Result<String, ForgetRefusal> {
    forget_target(&stored(texts), "ws_shop", quote).map(|m| m["text"].as_str().unwrap_or("").to_owned())
}

// case: hits the memory it quotes exactly
#[test]
fn a_forget_hits_the_memory_it_quotes_exactly() {
    assert_eq!(target("The API is on port 3000", &["The API is on port 3000", "Tests run with pnpm"]), Ok("The API is on port 3000".into()));
}

// case: still hits it when the quote is punctuated or cased differently
#[test]
fn a_forget_still_hits_when_punctuated_or_cased_differently() {
    assert_eq!(target("the api is on PORT 3000.", &["The API is on port 3000"]), Ok("The API is on port 3000".into()));
}

// case: hits it when the teammate drops a trailing clause -- the case that used to fail
#[test]
fn a_forget_hits_when_the_teammate_drops_a_trailing_clause() {
    let kept = "The API is on port 3000 in development, and 443 in production";
    assert_eq!(target("The API is on port 3000", &[kept]), Ok(kept.into()));
}

// case: hits it when the teammate quotes MORE than was stored
#[test]
fn a_forget_hits_when_the_teammate_quotes_more_than_was_stored() {
    assert_eq!(target("I think the API is on port 3000 here", &["API is on port 3000"]), Ok("API is on port 3000".into()));
}

// case: refuses, rather than guessing, when nothing answers to the quote
#[test]
fn a_forget_refuses_when_nothing_answers_to_the_quote() {
    assert_eq!(target("The database runs on Postgres", &["The API is on port 3000"]), Err(ForgetRefusal::NothingMatched));
}

// case: refuses when several memories answer to it, and removes neither
#[test]
fn a_forget_refuses_when_several_memories_answer_and_names_them() {
    assert_eq!(
        target("port 3000", &["port 3000 is the API", "port 3000 is also the docs server"]),
        Err(ForgetRefusal::Ambiguous(vec!["port 3000 is the API".into(), "port 3000 is also the docs server".into()]))
    );
}

// case: prefers an exact match even when others contain the same words
#[test]
fn a_forget_prefers_an_exact_match_over_memories_with_the_same_words() {
    assert_eq!(
        target("The API is on port 3000", &["The API is on port 3000", "The API is on port 3000 except on Tuesdays"]),
        Ok("The API is on port 3000".into())
    );
}

// case: does not treat two memories as one because they share a topic
#[test]
fn a_forget_does_not_take_a_shared_topic_for_the_same_memory() {
    assert_eq!(target("The API needs a token", &["The API is on port 3000"]), Err(ForgetRefusal::NothingMatched));
}

// case: refuses an empty or wordless quote instead of matching everything
#[test]
fn a_forget_refuses_an_empty_or_wordless_quote() {
    for quote in ["", "...", "is on a"] {
        assert_eq!(target(quote, &["The API is on port 3000"]), Err(ForgetRefusal::NothingMatched), "{quote:?}");
    }
}

// case: says nothing matched when the store is empty
#[test]
fn a_forget_says_nothing_matched_when_the_store_is_empty() {
    assert_eq!(forget_match("anything at all", &[]), Err(ForgetRefusal::NothingMatched));
    // The store itself says it holds nothing to forget (Electron's store: 'nothing-remembered'), and reading the reply
    // then says nothing about it.
    assert_eq!(target("anything at all", &[]), Err(ForgetRefusal::NothingRemembered));
}

// Same case, on forget_match itself, so its own promise does not rest on forget_target re-checking.
#[test]
fn forget_match_alone_refuses_two_memories_that_both_answer() {
    let keys = vec![key("port 3000 is the API"), key("port 3000 is also the docs server")];
    assert!(matches!(forget_match("port 3000", &keys), Err(ForgetRefusal::Ambiguous(_))), "{:?}", forget_match("port 3000", &keys));
}
