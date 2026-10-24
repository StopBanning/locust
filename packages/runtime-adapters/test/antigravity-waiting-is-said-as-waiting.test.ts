import { describe, expect, it } from "vitest";

import { agyHelpersIn, ANTIGRAVITY_STALLED_SENTENCE, createAgyEventNormalizer } from "../src/agy-events.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";

/**
 * ANTIGRAVITY'S WAITING IS SAID AS WAITING (0.833). Colin's Gemini 4 Argon turn, 2026-10-24, in the shape agy 1.4.7
 * reported it: a `manage_subagents` list with one helper "waiting_for_dependents", no step since 07:18; then six
 * wake-ups of five to six minutes, each re-reading the whole conversation (about 230,000 tokens, none reused); then a
 * call with no answer for five minutes, and agy's ERROR. The window showed six "Thought for 5m" rows and agy's raw
 * error. Now the rows say what it was waiting on, the third whole re-read is said with what it cost, and the end is
 * said in words.
 */
const CONVERSATION = "ce799772-ae3c-426e-843b-a0d33fd982e4";
const step = (index: number, body: Record<string, unknown>): string => JSON.stringify({ event: "step_update", step_update: { conversation_id: CONVERSATION, step_index: index, state: "DONE", ...body } });
const LIST = "You have 1 active subagent(s):\n" + JSON.stringify([{ role: "Soak & Sweep Runner", type: "self", conversationId: "8115c8f0", state: "waiting_for_dependents", lastStepTime: "2026-10-24T07:18:51Z" }]);
const wake = (index: number, seconds: number, thinking: number): string =>
  step(index, { step_type: "agent_response", duration_seconds: seconds, usage: { input_tokens: 229_594 + index, output_tokens: thinking, thinking_tokens: thinking, cache_read_tokens: 0, total_tokens: 229_594 + thinking } });
const completion = { exitCode: 0, signal: null, stderr: "", stderrTruncated: false, recordCount: 0, inputDeliveryFailed: false, outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, cancelled: false, startedAt: "2026-10-24T06:43:00.000Z", finishedAt: "2026-10-24T08:52:28.000Z" } as never;

const run = (lines: readonly string[]): NormalizedRuntimeEvent[] => {
  const normalizer = createAgyEventNormalizer({ runId: "run_1", missionId: "mission_1", sandbox: "full-access" });
  return [...lines.flatMap((raw, index) => normalizer.accept({ sequence: index + 1, raw } as never)), ...normalizer.finish(completion)];
};
const thoughts = (events: readonly NormalizedRuntimeEvent[]) => events.filter((event) => event.type === "step.completed" && (event.payload as { stepKind?: string }).stepKind === "reasoning").map((event) => event.payload as { message?: string; durationMs?: number });
const warnings = (events: readonly NormalizedRuntimeEvent[]) => events.filter((event) => event.type === "adapter.diagnostic" && (event.payload as { code?: string }).code === "antigravity.rereading").map((event) => (event.payload as { message: string }).message);

describe("Antigravity waiting on a helper", () => {
  const turn = [
    JSON.stringify({ event: "init", conversation_id: CONVERSATION, init: { model: "gemini-4-argon" } }),
    step(622, { step_type: "agent_response", duration_seconds: 7.3, usage: { input_tokens: 2_075, output_tokens: 55, thinking_tokens: 32, cache_read_tokens: 228_696 } }),
    step(623, { step_type: "tool", tool_name: "manage_subagents", duration_seconds: 0.1, tool_info: { name: "manage_subagents", parameters: { Action: "list" }, output: LIST } }),
    wake(624, 309.3, 98), wake(627, 357.3, 2_147), wake(629, 338.6, 1_252), wake(631, 320.2, 902), wake(633, 340.0, 3_006), wake(635, 352.1, 4_493),
    JSON.stringify({ event: "result", result: { conversation_id: CONVERSATION, status: "ERROR", response: "Waiting for task-570 to complete.", error: "agent executor error: generating and executing: request failed: stream idle timeout (5m0s): context deadline exceeded" } })
  ];

  it("reads the helpers a list names", () => {
    expect(agyHelpersIn(LIST)).toEqual([{ role: "Soak & Sweep Runner", state: "waiting_for_dependents", lastStepAt: "2026-10-24T07:18:51Z" }]);
    expect(agyHelpersIn("You have no active subagents.")).toEqual([]);
    expect(agyHelpersIn("not a list [ at all")).toEqual([]);
  });

  it("says each long wait after the list as a wait on that helper, at the length agy timed", () => {
    const all = thoughts(run(turn));
    // The short thought before the list is only a thought.
    expect(all).toHaveLength(7);
    expect(all[0]!.message).toBeUndefined();
    const rows = all.slice(1);
    expect(rows.map((row) => Math.round((row.durationMs ?? 0) / 1000))).toEqual([309, 357, 339, 320, 340, 352]);
    for (const row of rows) {
      expect(row.message?.startsWith("**Waiting on its helper Soak & Sweep Runner**")).toBe(true);
      expect(row.message).toContain("waiting for dependents, with no step since");
    }
  });

  it("says, once, at the third, that the whole conversation is being read again and what it has cost", () => {
    const events = run(turn);
    const said = warnings(events);
    expect(said).toHaveLength(1);
    expect(said[0]).toContain("read this whole conversation again 3 times this turn, about 230,000 tokens each, with none of it reused: 691,000 tokens of your quota so far");
    expect(said[0]).toContain("press Stop");
    // After the third wake-up's thought, not before (the short thought before the list is a fourth).
    const at = events.findIndex((event) => event.type === "adapter.diagnostic" && (event.payload as { code?: string }).code === "antigravity.rereading");
    expect(events.slice(0, at).filter((event) => event.type === "step.completed" && (event.payload as { stepKind?: string }).stepKind === "reasoning")).toHaveLength(4);
  });

  it("says the timeout that ended it in words", () => {
    const failed = run(turn).find((event) => event.type === "run.failed");
    expect((failed?.payload as { message?: string }).message).toBe(ANTIGRAVITY_STALLED_SENTENCE);
  });

  it("keeps the last list through a call that is not one, and lets it go once no helper is left", () => {
    const manage = (index: number, output: string) => step(index, { step_type: "tool", tool_name: "manage_subagents", tool_info: { name: "manage_subagents", parameters: { Action: "send" }, output } });
    const rows = thoughts(run([
      JSON.stringify({ event: "init", conversation_id: CONVERSATION, init: {} }),
      manage(1, LIST),
      manage(2, "Sent your message to Soak & Sweep Runner."),
      wake(3, 300, 50),
      manage(4, "You have no active subagents."),
      wake(5, 300, 50),
      JSON.stringify({ event: "result", result: { conversation_id: CONVERSATION, status: "SUCCESS", response: "Done." } })
    ]));
    expect(rows[0]!.message?.startsWith("**Waiting on its helper Soak & Sweep Runner**")).toBe(true);
    expect(rows[1]!.message).toBeUndefined();
  });

  it("says nothing of the kind for a helper that finished, or for steps that reused the conversation", () => {
    const finished = LIST.replace("waiting_for_dependents", "completed");
    const cached = (index: number) => step(index, { step_type: "agent_response", duration_seconds: 40, usage: { input_tokens: 229_594, output_tokens: 500, thinking_tokens: 500, cache_read_tokens: 228_000 } });
    const events = run([
      JSON.stringify({ event: "init", conversation_id: CONVERSATION, init: {} }),
      step(1, { step_type: "tool", tool_name: "manage_subagents", tool_info: { name: "manage_subagents", parameters: { Action: "list" }, output: finished } }),
      cached(2), cached(3), cached(4), cached(5),
      JSON.stringify({ event: "result", result: { conversation_id: CONVERSATION, status: "SUCCESS", response: "Done." } })
    ]);
    expect(thoughts(events).every((row) => row.message === undefined)).toBe(true);
    expect(warnings(events)).toEqual([]);
  });
});
