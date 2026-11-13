//! Electron's "testing an endpoint" cases (main/your-own-model.test.ts), against an endpoint the test answers for.
use super::*;
use std::sync::Mutex;

type Answer = Box<dyn Fn(&str, &[(&str, String)], Option<&str>, Duration) -> Outcome + Send + Sync>;

struct Fake {
    answer: Answer,
    asked: Mutex<Vec<(String, Option<String>, Option<String>, Duration)>>,
}
impl Fetch for Fake {
    fn request(&self, _method: &str, url: &str, headers: &[(&str, String)], body: Option<&str>, timeout: Duration) -> Outcome {
        let auth = headers.iter().find(|(n, _)| *n == "authorization").map(|(_, v)| v.clone());
        self.asked.lock().unwrap().push((url.to_owned(), auth, body.map(str::to_owned), timeout));
        (self.answer)(url, headers, body, timeout)
    }
}

fn fake(answer: impl Fn(&str, &[(&str, String)], Option<&str>, Duration) -> Outcome + Send + Sync + 'static) -> Fake {
    Fake { answer: Box::new(answer), asked: Mutex::new(vec![]) }
}

fn ok(status: u16, body: Value) -> Outcome {
    Outcome::Answered { status, status_text: String::new(), body: body.to_string() }
}

/// The list, and how it answers a request that carries a tool.
fn endpoint(list: Outcome, chat: Outcome) -> Fake {
    fake(move |url, _, _, _| if url.ends_with("/models") { list.clone() } else { chat.clone() })
}

#[test]
fn says_it_answered_and_serves_the_model_and_when_the_one_word_request_fails_that_it_could_not_tell() {
    let f = endpoint(ok(200, json!({"data": [{"id": "acme-70b"}]})), ok(500, json!({})));
    assert_eq!(
        test_own_endpoint("https://x/v1", "acme-70b", None, &f),
        Tested { ok: false, said: "It answered, and serves acme-70b. A one-word request got 500 back, so Test could not tell whether it can use tools.".into(), tools: None }
    );
}

#[test]
fn a_slow_model_gets_a_minute_and_one_that_never_answers_or_cannot_be_reached_is_said() {
    let listed = || ok(200, json!({"data": [{"id": "m"}]}));
    let f = endpoint(listed(), ok(200, json!({})));
    assert_eq!(test_own_endpoint("http://127.0.0.1:8089/v1", "m", None, &f).said, "It answered, and serves m. It can use tools.");
    let bounds: Vec<Duration> = f.asked.lock().unwrap().iter().map(|a| a.3).collect();
    assert_eq!(bounds, vec![Duration::from_secs(8), Duration::from_secs(60)]);
    let never = endpoint(listed(), Outcome::TimedOut);
    assert_eq!(
        test_with("http://127.0.0.1:8089/v1", "m", None, &never, Duration::from_millis(50), Duration::from_secs(1)),
        Tested { ok: false, said: "It answered, and serves m. It did not answer a one-word request in 1 second, so Test could not tell whether it can use tools.".into(), tools: None }
    );
    let gone = endpoint(listed(), Outcome::Failed("fetch failed".into()));
    assert_eq!(test_own_endpoint("http://127.0.0.1:8089/v1", "m", None, &gone).said, "It answered, and serves m. A one-word request to it failed (fetch failed), so Test could not tell whether it can use tools.");
}

#[test]
fn says_whether_it_takes_tools_and_finds_a_model_that_only_chats() {
    let listed = || ok(200, json!({"data": [{"id": "acme-70b"}]}));
    assert_eq!(test_own_endpoint("https://x/v1", "acme-70b", None, &endpoint(listed(), ok(200, json!({"choices": []})))), Tested { ok: true, said: "It answered, and serves acme-70b. It can use tools.".into(), tools: Some(true) });
    // Ollama's own words for a model without tool support.
    let refused = ok(400, json!({"error": {"message": "registry.ollama.ai/library/plain does not support tools"}}));
    assert_eq!(test_own_endpoint("https://x/v1", "acme-70b", None, &endpoint(listed(), refused)), Tested { ok: true, said: "It answered, and serves acme-70b. It cannot use tools, so it is set to chat only.".into(), tools: Some(false) });
}

#[test]
fn asks_about_tools_with_one_request_capped_at_a_single_token() {
    let f = endpoint(ok(200, json!({"data": [{"id": "m"}]})), ok(200, json!({})));
    test_own_endpoint("https://x/v1", "m", None, &f);
    let bodies: Vec<String> = f.asked.lock().unwrap().iter().filter_map(|a| a.2.clone()).collect();
    assert_eq!(bodies.len(), 1);
    let body: Value = serde_json::from_str(&bodies[0]).unwrap();
    assert_eq!((body["model"].as_str(), body["max_tokens"].as_u64(), body["tools"][0]["type"].as_str()), (Some("m"), Some(1), Some("function")));
}

#[test]
fn catches_a_model_name_the_endpoint_does_not_know_naming_what_it_does() {
    let f = endpoint(ok(200, json!({"data": [{"id": "acme-70b"}, {"id": "acme-8b"}]})), ok(500, json!({})));
    assert_eq!(test_own_endpoint("https://x/v1", "acme-7b", None, &f), Tested { ok: false, said: "It answered, but does not list acme-7b. It lists acme-70b, acme-8b.".into(), tools: None });
}

#[test]
fn says_a_refused_key_is_a_refused_key() {
    let f = endpoint(ok(401, json!({})), ok(500, json!({})));
    assert_eq!(test_own_endpoint("https://x/v1", "m", Some("bad"), &f).said, "It answered, and refused the key.");
}

#[test]
fn sends_the_key_as_a_bearer_token_to_that_address_only() {
    let f = endpoint(ok(200, json!({"data": []})), ok(200, json!({})));
    test_own_endpoint("https://llm.acme.example/v1", "m", Some("made-up-1"), &f);
    let asked: Vec<(String, Option<String>)> = f.asked.lock().unwrap().iter().map(|a| (a.0.clone(), a.1.clone())).collect();
    assert_eq!(
        asked,
        vec![
            ("https://llm.acme.example/v1/models".to_owned(), Some("Bearer made-up-1".to_owned())),
            ("https://llm.acme.example/v1/chat/completions".to_owned(), Some("Bearer made-up-1".to_owned())),
        ]
    );
}

#[test]
fn lists_the_models_of_anthropics_compatible_endpoint_which_wants_a_version_header_there() {
    let f = fake(|url, headers, _, _| {
        if url.ends_with("/models") && !headers.iter().any(|(n, _)| *n == "anthropic-version") {
            return Outcome::Answered { status: 400, status_text: "Bad Request".into(), body: "anthropic-version: header is required".into() };
        }
        ok(200, if url.ends_with("/models") { json!({"data": [{"id": "claude-haiku-5-5"}]}) } else { json!({"choices": []}) })
    });
    assert_eq!(test_own_endpoint("https://api.anthropic.com/v1", "claude-haiku-5-5", Some("made-up-2"), &f), Tested { ok: true, said: "It answered, and serves claude-haiku-5-5. It can use tools.".into(), tools: Some(true) });
}

#[test]
fn says_it_could_not_be_reached_rather_than_failing() {
    let f = endpoint(Outcome::Failed("Could not resolve host: x".into()), ok(500, json!({})));
    assert_eq!(test_own_endpoint("https://x/v1", "m", None, &f).said, "It could not be reached: Could not resolve host: x.");
    let slow = endpoint(Outcome::TimedOut, ok(500, json!({})));
    assert_eq!(test_own_endpoint("https://x/v1", "m", None, &slow).said, "No answer in 8 seconds.");
    let unlisted = endpoint(ok(200, json!({"object": "list"})), ok(200, json!({})));
    assert_eq!(test_own_endpoint("https://x/v1", "m", None, &unlisted).said, "It answered. It did not list its models, so the model name could not be checked. It can use tools.");
}

#[test]
fn a_request_is_never_made_from_a_test() {
    assert_eq!(Curl.request("GET", "https://example.com", &[], None, Duration::from_secs(1)), Outcome::Failed("no network in a test".into()));
}
