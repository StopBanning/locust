import { describe, expect, it } from "vitest";

import { MUSE_QUOTA_SENTENCE, createMuseEventNormalizer, museQuotaLimit } from "../src/muse-events.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";
import type { RuntimeProcessCompletion } from "../src/process-runner.js";

/*
 * A MUSE CODE PLAN USED UP IS SAID ONCE, AS A LIMIT (0.823).
 *
 * Muse Code 1.4.11, Muse Wonder 1.0 in Ask, 2026-10-22: 57 tool calls into a read-only audit, its own model call failed
 * "API error 429 [...]: Subscription quota exhausted. Your usage window resets at 2026-10-26T00:00:00Z.
 * (rate_limit_error)" and the run ended failed with the same words. The thread said it three times, beside three
 * kinds of "Unhandled Muse ..." line: `tool_output_ref` and `cancelled` task states, and a `run.usage` record. The
 * records below are that run's, as Muse wrote them (ids shortened, the session folder's path made neutral).
 */
const NOW = "2026-10-22T16:58:47.000Z";
const QUOTA = "API error 429 [request_id=2370cf39-f35e-496c-8d65-7ca6df260286]: Subscription quota exhausted. Your usage window resets at 2026-10-26T00:00:00Z. (rate_limit_error)";
const completion = (exitCode: number): RuntimeProcessCompletion => ({
  exitCode,
  signal: null,
  stderr: `muse: Agent delegation: auto unavailable: workspace is untrusted.\nrun ended with Failed: ${QUOTA}\n`,
  stderrTruncated: false,
  recordCount: 1225,
  cancelled: false,
  forcedTerminationAttempted: false,
  terminationUnconfirmed: false,
  inputDeliveryFailed: false,
  outputLimitExceeded: false,
  startedAt: NOW,
  finishedAt: NOW,
});

const SESSION = { kind: "session", id: "01a16488-07c9-7270-b9db-a0c54559f14d" };
const record = (payloadType: string, payload: Record<string, unknown>, recordType = "event"): string =>
  JSON.stringify({ schema_version: 1, stream: SESSION, record_type: recordType, payload_type: payloadType, payload });
const task = (taskId: string, event: Record<string, unknown>): string =>
  record(`task.lifecycle.${String(event.kind)}`, { kind: "task_lifecycle", task_id: taskId, event: { task_id: taskId, ...event } });

const FETCH = "01a1648b-58af";
const MODEL = "01a1648d-03c0";
const SEARCH = "01a1648d-1111";
const QUEUED = "01a1648d-2222";
const lines = [
  record("run.lifecycle.started", {}),
  task(FETCH, { kind: "proposed", task_kind: "tool.web_fetch" }),
  task(FETCH, { kind: "started" }),
  task(FETCH, {
    kind: "tool_output_ref",
    output_ref: { id: "web_fetch-page-1", kind: "web_fetch-page", uri: "tool-output://local/1/page.txt", media_type: "text/plain; charset=utf-8", byte_len: 15460, digest: null, path: "C:\\Users\\a\\.local\\share\\muse\\sessions\\page.txt" },
    availability: "available",
  }),
  task(FETCH, { kind: "completed" }),
  task(MODEL, { kind: "proposed", task_kind: "model.meta.response" }),
  task(SEARCH, { kind: "proposed", task_kind: "tool.search" }),
  task(SEARCH, { kind: "started" }),
  task(QUEUED, { kind: "proposed", task_kind: "tool.read_file" }),
  task(MODEL, { kind: "failed", reason: QUOTA }),
  task(SEARCH, { kind: "cancelled", reason: "main run failed" }),
  task(QUEUED, { kind: "cancelled", reason: "main run failed" }),
  task(MODEL, { kind: "cancelled", reason: "main run failed" }),
  record("run.usage", {
    kind: "run_usage",
    models: [{ model: "muse-wonder-1.0", input_tokens: 4428573, cached_tokens: 4266120, output_tokens: 8818, reasoning_tokens: 4444, calls: 43 }],
    calls: 44, completed_calls: 43, failed_calls: 1, cancelled_calls: 0, children_included: false,
    allowance: { tier: null, window: { used_percent: 92, resets_at_ms: 1792706021000 }, weekly: { used_percent: 97, resets_at_ms: 1792972800000 }, as_of_ms: 1792688325219, source: "stream" },
  }, "status"),
  record("run.terminal.failed", { kind: "run_terminal", terminal: "failed", text: "", reason: QUOTA }),
];

function run(input: readonly string[], exitCode = 1): readonly NormalizedRuntimeEvent[] {
  const muse = createMuseEventNormalizer({ runId: "run_muse", now: () => new Date(NOW) });
  const events: NormalizedRuntimeEvent[] = [];
  input.forEach((raw, index) => events.push(...muse.accept({ sequence: index + 1, raw })));
  events.push(...muse.finish(completion(exitCode)));
  return events;
}
const said = (event: NormalizedRuntimeEvent): string => String((event.payload as { readonly message?: unknown }).message);

describe("a Muse Code plan used up", () => {
  it("is one limit with its reset instant, and a failure that says what to do", () => {
    const events = run(lines);
    const limits = events.filter((event) => event.type === "route.limit_detected");
    expect(limits).toHaveLength(1);
    expect(limits[0]?.payload).toMatchObject({ kind: "quota-exhausted", message: "Muse Code's plan is used up · resets 2026-10-26T00:00:00Z" });
    const failed = events.filter((event) => event.type === "run.failed");
    expect(failed.map(said)).toEqual([MUSE_QUOTA_SENTENCE]);
    expect(MUSE_QUOTA_SENTENCE).not.toMatch(/429|request_id/);
  });

  it("leaves no Unhandled line: a saved output and the run's usage are not shown, a stopped tool fails on its row", () => {
    const events = run(lines);
    expect(events.filter((event) => event.type === "adapter.diagnostic").map(said)).toEqual([]);
    // The fetch ran and finished; its output saved to a file of Muse's is not a row of its own.
    expect(events.filter((event) => event.type === "tool.completed").map((event) => (event.payload as { readonly itemId?: string }).itemId)).toEqual([FETCH]);
    // The search Muse stopped when the run failed says so on its row; a tool still queued, and Muse's own model call,
    // say nothing.
    const stopped = events.filter((event) => event.type === "tool.failed");
    expect(stopped.map((event) => [(event.payload as { readonly itemId?: string }).itemId, (event.payload as { readonly status?: string }).status])).toEqual([[SEARCH, "Muse Code stopped it (main run failed)"]]);
  });

  it("is still said once when only the run's own end says it", () => {
    const events = run([record("run.lifecycle.started", {}), record("run.terminal.failed", { kind: "run_terminal", terminal: "failed", text: "", reason: QUOTA })]);
    expect(events.filter((event) => event.type === "route.limit_detected").map(said)).toEqual(["Muse Code's plan is used up · resets 2026-10-26T00:00:00Z"]);
    expect(events.at(-1)?.type).toBe("run.failed");
    expect(said(events.at(-1)!)).toBe(MUSE_QUOTA_SENTENCE);
  });

  it("names no reset it was not told", () => {
    expect(museQuotaLimit("API error 429: Subscription quota exhausted. (rate_limit_error)")).toBe("Muse Code's plan is used up");
  });
});
