//! Electron's `packages/runtime-adapters/test/a-codex-schema-change-cannot-silently-drop-a-read-field.test.ts` and
//! its `codex-schema-read-fields.ts`: the generated app-server schema fields this normalizer (events.rs) reads, kept
//! with their type and variant so a field elsewhere cannot mask its loss, and the check that says which are gone.
//! The check runs against Codex's own `generate-json-schema` only when LOCUST_CODEX_SCHEMA=1 (ignored otherwise).
use serde_json::{Value, json};

type ReadField = (&'static str, &'static str, &'static [&'static str]);

/// Canonical generated-schema fields read by events.rs (app-server-events.ts READ_FIELDS).
const READ_FIELDS: &[ReadField] = &[
    ("ThreadStartedNotification", "", &["thread"]),
    ("Thread", "", &["id"]),
    ("TurnStartedNotification", "", &["threadId", "turn"]),
    ("TurnCompletedNotification", "", &["threadId"]),
    ("ItemStartedNotification", "", &["threadId", "item"]),
    ("ItemCompletedNotification", "", &["threadId", "item"]),
    ("AgentMessageDeltaNotification", "", &["threadId", "itemId", "delta"]),
    ("TurnPlanUpdatedNotification", "", &["threadId", "turnId", "plan"]),
    ("ThreadTokenUsageUpdatedNotification", "", &["threadId", "tokenUsage"]),
    ("ThreadTokenUsage", "", &["total", "last"]),
    ("TokenUsageBreakdown", "", &["inputTokens", "outputTokens", "cachedInputTokens", "cacheWriteInputTokens"]),
    ("AccountRateLimitsUpdatedNotification", "", &["rateLimits"]),
    ("RateLimitWindow", "", &["usedPercent", "windowDurationMins", "resetsAt"]),
    ("ErrorNotification", "", &["threadId", "error", "willRetry"]),
    ("TurnError", "", &["message", "codexErrorInfo"]),
    ("WarningNotification", "", &["message"]),
    ("GuardianWarningNotification", "", &["message"]),
    ("ConfigWarningNotification", "", &["summary", "details"]),
    ("ThreadItem", "agentMessage", &["type", "id", "text"]),
    ("ThreadItem", "reasoning", &["type", "id", "summary"]),
    ("ThreadItem", "commandExecution", &["type", "id", "command", "status", "exitCode", "aggregatedOutput"]),
    ("ThreadItem", "fileChange", &["type", "id", "changes", "status"]),
    ("ThreadItem", "mcpToolCall", &["type", "id", "server", "tool", "status"]),
    ("ThreadItem", "dynamicToolCall", &["type", "id", "tool", "status"]),
    ("ThreadItem", "collabAgentToolCall", &["type", "id", "tool", "status"]),
    ("ThreadItem", "webSearch", &["type", "id", "query"]),
    ("ThreadItem", "imageGeneration", &["type", "id", "status"]),
    ("ThreadItem", "contextCompaction", &["type", "id"]),
    ("ThreadItem", "userMessage", &["type", "id"]),
];

/// The codexErrorInfo variants events.rs classifies (codex_failure::app_server_failure_kind).
const ERROR_VARIANTS: &[&str] = &["usageLimitExceeded", "rateLimitExceeded", "unauthorized", "cyberPolicy", "misalignmentPolicyViolation"];

fn resolve<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    match schema["$ref"].as_str().and_then(|r| r.strip_prefix("#/definitions/")) {
        Some(name) => root["definitions"].get(name).unwrap_or(&Value::Null),
        None => schema,
    }
}

fn fields(schema: &Value, root: &Value) -> serde_json::Map<String, Value> {
    let held = resolve(schema, root);
    let mut out = serde_json::Map::new();
    for part in held["allOf"].as_array().into_iter().flatten() {
        out.extend(fields(part, root));
    }
    if let Some(properties) = held["properties"].as_object() {
        out.extend(properties.clone());
    }
    out
}

fn enum_values(schema: &Value, root: &Value) -> Vec<Value> {
    let held = resolve(schema, root);
    let mut out: Vec<Value> = held["enum"].as_array().cloned().unwrap_or_default();
    for key in ["oneOf", "anyOf"] {
        for part in held[key].as_array().into_iter().flatten() {
            out.extend(enum_values(part, root));
        }
    }
    out
}

fn missing_read_fields(root: &Value, reads: &[ReadField]) -> Vec<String> {
    let mut missing = vec![];
    for (definition, variant, names) in reads {
        let Some(schema) = root["definitions"].get(*definition) else {
            missing.push(format!("{definition}: missing definition"));
            continue;
        };
        let target = if variant.is_empty() {
            Some(schema)
        } else {
            ["oneOf", "anyOf"].iter().flat_map(|k| schema[*k].as_array().into_iter().flatten()).find(|part| {
                let kinds = fields(part, root).get("type").map(|t| enum_values(t, root)).unwrap_or_default();
                kinds.contains(&json!(variant))
            })
        };
        let Some(target) = target else {
            missing.push(format!("{definition}.{variant}: missing variant"));
            continue;
        };
        let properties = fields(target, root);
        for name in *names {
            if !properties.contains_key(*name) {
                let at = if variant.is_empty() { String::new() } else { format!(".{variant}") };
                missing.push(format!("{definition}{at}.{name}"));
            }
        }
    }
    missing
}

fn missing_error_variants(root: &Value) -> Vec<String> {
    let variants = enum_values(root["definitions"].get("CodexErrorInfo").unwrap_or(&json!({})), root);
    ERROR_VARIANTS.iter().filter(|v| !variants.contains(&json!(v))).map(|v| format!("CodexErrorInfo.{v}")).collect()
}

#[test]
fn a_disappeared_field_cannot_be_masked_by_the_same_spelling_on_a_different_item_type() {
    // case: A disappeared field cannot be masked by the same spelling on a different item type.
    let schema = json!({"definitions":{"ThreadItem":{"oneOf":[
        {"properties":{"type":{"enum":["fileChange"]},"id":{},"status":{}}},
        {"properties":{"type":{"enum":["commandExecution"]},"id":{},"command":{}}}
    ]}}});
    let reads: &[ReadField] = &[("ThreadItem", "commandExecution", &["id", "command"])];
    assert!(missing_read_fields(&schema, reads).is_empty());
    let broken = json!({"definitions":{"ThreadItem":{"oneOf":[
        {"properties":{"type":{"enum":["fileChange"]},"id":{},"command":{}}},
        {"properties":{"type":{"enum":["commandExecution"]},"id":{}}}
    ]}}});
    assert_eq!(missing_read_fields(&broken, reads), ["ThreadItem.commandExecution.command"]);
}

#[test]
fn a_disappeared_definition_or_tagged_variant_fails_distinctly_from_an_absent_property() {
    // case: A disappeared definition or tagged variant fails distinctly from an absent property.
    assert_eq!(missing_read_fields(&json!({}), &[("RateLimitWindow", "", &["usedPercent"])]), ["RateLimitWindow: missing definition"]);
    assert_eq!(
        missing_read_fields(&json!({"definitions":{"ThreadItem":{"oneOf":[]}}}), &[("ThreadItem", "fileChange", &["changes"])]),
        ["ThreadItem.fileChange: missing variant"]
    );
}

#[test]
fn references_and_allof_fields_are_followed_and_absent_fields_still_fail() {
    // case: References and allOf fields are followed, and absent fields still fail.
    let schema = json!({"definitions":{"Base":{"properties":{"delta":{}}},"Message":{"allOf":[{"$ref":"#/definitions/Base"},{"properties":{"itemId":{}}}]}}});
    assert!(missing_read_fields(&schema, &[("Message", "", &["delta", "itemId"])]).is_empty());
    assert_eq!(missing_read_fields(&schema, &[("Message", "", &["threadId"])]), ["Message.threadId"]);
}

#[test]
fn removing_a_classified_error_variant_is_schema_drift_too() {
    // case: Removing a classified error variant is schema drift too.
    let values = json!(["usageLimitExceeded", "rateLimitExceeded", "unauthorized", "cyberPolicy", "misalignmentPolicyViolation"]);
    assert!(missing_error_variants(&json!({"definitions":{"CodexErrorInfo":{"oneOf":[{"enum":values}]}}})).is_empty());
    let fewer = json!(["rateLimitExceeded", "unauthorized", "cyberPolicy", "misalignmentPolicyViolation"]);
    assert_eq!(missing_error_variants(&json!({"definitions":{"CodexErrorInfo":{"enum":fewer}}})), ["CodexErrorInfo.usageLimitExceeded"]);
}

/// Every variant events.rs classifies is one this inventory checks, and the reverse: the two lists are one fact.
#[test]
fn the_classified_variants_are_the_ones_the_normalizer_names() {
    for variant in ERROR_VARIANTS {
        assert_ne!(crate::codex_failure::app_server_failure_kind(variant, "x"), "unknown", "{variant}");
    }
}

#[test]
#[ignore = "runs the installed Codex: LOCUST_CODEX_SCHEMA=1"]
fn the_installed_codex_schema_still_contains_every_field_the_normalizer_reads() {
    assert_eq!(std::env::var("LOCUST_CODEX_SCHEMA").ok().as_deref(), Some("1"), "set LOCUST_CODEX_SCHEMA=1");
    let codex = super::super::discovery::find_named("codex").expect("install Codex; this check installs nothing");
    let out = std::env::temp_dir().join(format!("locust-codex-schema-{}", crate::invariants::unique_nanos()));
    let status = std::process::Command::new(codex).args(["app-server", "generate-json-schema", "--out"]).arg(&out).status().unwrap();
    assert!(status.success());
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(out.join("codex_app_server_protocol.v2.schemas.json")).unwrap()).unwrap();
    let mut missing = missing_read_fields(&schema, READ_FIELDS);
    missing.extend(missing_error_variants(&schema));
    let _ = std::fs::remove_dir_all(&out);
    assert!(missing.is_empty(), "a generated wire field events.rs reads disappeared: {missing:?}");
}
