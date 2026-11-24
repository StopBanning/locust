//! a-model-of-your-own-is-priced-step-by-step.test.ts (0.810): a model of the person's own that its provider lists is
//! priced step by step from that list, the dearer tier for a step whose context passed its size.
use super::*;
use serde_json::{Value, json};

fn haiku() -> Value {
    json!({"input":0.1,"output":0.5,"cache_read":0.01,"cache_write":0.125,"tiers":[{"input":0.5,"output":2.5,"cache_read":0.05,"cache_write":0.625,"tier":{"type":"context","size":100_000}}]})
}

fn step(input: f64) -> StepTokens {
    StepTokens { input, ..Default::default() }
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} is not {b}");
}

#[test]
fn prices_input_output_reasoning_and_both_caches_each_at_its_own_rate() {
    // case: prices input, output, reasoning and both caches, each at its own rate
    let flat = json!({"input":2,"output":10,"cache_read":0.1,"cache_write":2.5});
    close(
        listed_step_cost(&flat, StepTokens { input: 1000.0, output: 100.0, reasoning: 50.0, cache_read: 2000.0, cache_write: 500.0 }),
        0.00495,
    );
    // A price that names no cache rate charges none.
    close(listed_step_cost(&json!({"input":1,"output":2}), StepTokens { cache_read: 5000.0, ..step(1000.0) }), 0.001);
}

#[test]
fn takes_the_dearer_tier_once_the_steps_context_caches_included_passes_its_size() {
    // case: takes the dearer tier once the step's context, caches included, passes its size
    close(listed_step_cost(&haiku(), StepTokens { cache_read: 20_000.0, ..step(50_000.0) }), (50_000.0 * 0.1 + 20_000.0 * 0.01) / 1e6);
    close(
        listed_step_cost(&haiku(), StepTokens { cache_read: 20_000.0, output: 100.0, ..step(90_000.0) }),
        (90_000.0 * 0.5 + 20_000.0 * 0.05 + 100.0 * 2.5) / 1e6,
    );
    // At the size exactly, still the base: a tier is for context over it.
    close(listed_step_cost(&haiku(), step(100_000.0)), 0.01);
}

#[test]
fn takes_the_largest_tier_passed_and_the_older_over_200k_price_where_a_list_gives_that() {
    // case: takes the largest tier passed, and the older over-200k price where a list gives that
    let tiered = json!({"input":1,"output":1,"tiers":[{"input":2,"output":2,"tier":{"type":"context","size":100_000}},{"input":3,"output":3,"tier":{"type":"context","size":200_000}}]});
    close(listed_step_cost(&tiered, step(250_000.0)), 0.75);
    close(listed_step_cost(&tiered, step(150_000.0)), 0.3);
    let older = json!({"input":3,"output":15,"context_over_200k":{"input":6,"output":22.5}});
    close(listed_step_cost(&older, step(210_000.0)), 1.26);
    close(listed_step_cost(&older, step(190_000.0)), 0.57);
}

fn usd_of(price: Option<Value>) -> Option<f64> {
    let mut opencode = Normalizer::new(Some("own-0a1b2c3d/claude-haiku-5-5")).with_price(price);
    let records = [
        json!({"type":"step_start","sessionID":"ses_1","part":{"type":"step-start"}}),
        // OpenCode's own figure for this step: the base only.
        json!({"type":"step_finish","sessionID":"ses_1","part":{"type":"step-finish","reason":"tool-calls","cost":0.012,"tokens":{"input":120_000,"output":0,"reasoning":0,"cache":{"read":0,"write":0}}}}),
        json!({"type":"step_start","sessionID":"ses_1","part":{"type":"step-start"}}),
        json!({"type":"step_finish","sessionID":"ses_1","part":{"type":"step-finish","reason":"stop","cost":0.0001,"tokens":{"input":1000,"output":100,"reasoning":0,"cache":{"read":0,"write":0}}}}),
    ];
    for r in &records {
        opencode.accept(r);
    }
    let done = opencode.finish(false, Some(0), "", false);
    done.last()?.payload["usage"]["usd"].as_f64()
}

#[test]
fn adds_up_each_step_at_its_listed_price_the_step_over_the_tier_at_the_tiers() {
    // case: adds up each step at its listed price, the step over the tier at the tier's
    close(usd_of(Some(haiku())).unwrap(), 0.06 + 0.00015);
    // The price reaches the reader from the run's own config, as Your own models declared it.
    let config = json!({"provider":{"own-0a1b2c3d":{"models":{"claude-haiku-5-5":{"name":"claude-haiku-5-5","cost":haiku()}}}}});
    assert_eq!(listed_price(&config, "own-0a1b2c3d/claude-haiku-5-5"), Some(haiku()));
    assert_eq!(listed_price(&config, "opencode/beam-free"), None);
}

#[test]
fn reads_opencodes_own_figure_where_no_price_is_given_as_before() {
    // case: reads OpenCode's own figure where no price is given, as before
    close(usd_of(None).unwrap(), 0.0121);
}
