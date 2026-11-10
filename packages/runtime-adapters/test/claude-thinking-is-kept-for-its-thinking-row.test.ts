import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { createClaudeEventNormalizer } from "../src/claude-events.js";
import { boundedMessageText } from "../src/codex-events.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";

const record = (value: unknown) => ({ sequence: 1, raw: JSON.stringify(value) });
const stream = (event: unknown, parent_tool_use_id: string | null = null) => ({ type: "stream_event", event, parent_tool_use_id });
const start = (index = 0, type = "thinking", thinking = "") => stream({ type: "content_block_start", index, content_block: { type, thinking, data: "OPAQUE REDACTED DATA", signature: "OPAQUE SIGNATURE" } });
const delta = (thinking: string, index = 0) => stream({ type: "content_block_delta", index, delta: { type: "thinking_delta", thinking } });
const stop = (index = 0) => stream({ type: "content_block_stop", index });
const whole = (thinking: string, id = "msg_1") => ({ type: "assistant", message: { id, content: [{ type: "thinking", thinking, signature: "OPAQUE SIGNATURE" }] } });
const run = (values: unknown[]): NormalizedRuntimeEvent[] => {
  const n = createClaudeEventNormalizer({ runId: "r", now: () => new Date("2026-11-10T12:00:00Z") });
  return values.flatMap(value => n.accept(record(value)));
};
const thoughts = (events: readonly NormalizedRuntimeEvent[]) => events.filter(e => e.type === "step.completed" && e.payload.stepKind === "reasoning");
const messages = (events: readonly NormalizedRuntimeEvent[]) => thoughts(events).map(e => e.type === "step.completed" ? e.payload.message : undefined);

describe("Claude thinking is kept for its thinking row", () => {
  it("keeps shown streamed thinking on the closing step, including an initial fragment", () => {
    const events = run([start(0, "thinking", "First "), delta("inspect the "), delta("configuration."), stop()]);
    expect(messages(events)).toEqual(["First inspect the configuration."]);
    expect(events.filter(e => e.type === "step.started" && e.payload.stepKind === "reasoning")).toHaveLength(1);
    expect(events.filter(e => e.type === "message.delta")).toEqual([]);
    expect(events[0]?.payload).not.toHaveProperty("message");
  });

  it("keeps a whole shown thinking block when partial output was not sent", () => {
    expect(messages(run([whole("Inspect the configuration.")]))).toEqual(["Inspect the configuration."]);
  });

  it("uses the whole block before its stop, without repeating a streamed thought or its later echo", () => {
    const events = run([stream({ type: "message_start", message: { id: "msg_1" } }), start(2), delta("Partial.", 2), whole("The whole shown thought."), stop(2), whole("The whole shown thought.")]);
    expect(messages(events)).toEqual(["The whole shown thought."]);
    expect(events.filter(e => e.type === "step.started" && e.payload.stepKind === "reasoning")).toHaveLength(1);
  });

  it("scrubs secrets from streamed and whole thinking, including a key split between deltas", () => {
    const token = "sk-abcdefghijklmnopqrstuv";
    for (const values of [[start(), delta("Check sk-abc"), delta("defghijklmnopqrstuv safely."), stop()], [whole(`Check ${token} safely.`)]]) {
      const events = run(values);
      expect(messages(events)[0]).toContain("[redacted]");
      expect(messages(events)[0]).toContain("safely.");
      expect(JSON.stringify(events)).not.toContain(token);
      expect(JSON.stringify(events)).not.toContain("OPAQUE SIGNATURE");
    }
  });

  it("bounds the thinking message exactly as Cursor does while keeping both ends", () => {
    const text = "Start. " + "x".repeat(30_000) + " The end.";
    expect(messages(run([start(), delta(text), stop()]))).toEqual([boundedMessageText(text)]);
    expect(messages(run([whole(text)]))).toEqual([boundedMessageText(text)]);
    expect(messages(run([start(), delta(text), stop()]))[0]?.length).toBe(16_384);
  });

  it("times redacted thinking without retaining its text, opaque data or signature", () => {
    const events = run([start(0, "redacted_thinking"), delta("WORDS NEVER SHOWN"), stop(), { type: "assistant", message: { id: "redacted", content: [{ type: "redacted_thinking", data: "OPAQUE REDACTED DATA" }] } }]);
    expect(messages(events)).toEqual([undefined, undefined]);
    expect(JSON.stringify(events)).not.toContain("WORDS NEVER SHOWN");
    expect(JSON.stringify(events)).not.toContain("OPAQUE REDACTED DATA");
    expect(JSON.stringify(events)).not.toContain("OPAQUE SIGNATURE");
  });

  it("keeps helper thinking out of the teammate row and leaves the teammate’s open thought intact", () => {
    const helper = "helper_call";
    const events = run([start(), delta("The teammate’s thought."),
      { ...start(), parent_tool_use_id: helper }, { ...delta("HELPER THOUGHT"), parent_tool_use_id: helper },
      { ...stop(), parent_tool_use_id: helper }, { ...whole("HELPER THOUGHT"), parent_tool_use_id: helper }, stop()]);
    expect(messages(events)).toEqual(["The teammate’s thought."]);
    expect(JSON.stringify(events)).not.toContain("HELPER THOUGHT");
  });

  it("replays captured empty thinking as timed steps without inventing words", () => {
    const values = readFileSync(new URL("fixtures/claude/edit-and-write.jsonl", import.meta.url), "utf8").trim().split("\n").map(line => JSON.parse(line));
    const events = run(values);
    expect(messages(events)).toEqual([undefined, undefined, undefined]);
    expect(events.some(e => e.type === "tool.completed")).toBe(true);
  });
});
