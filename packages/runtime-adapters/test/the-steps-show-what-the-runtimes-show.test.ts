import { describe, expect, it } from "vitest";

import { claudeShowsOutput, createClaudeEventNormalizer } from "../src/claude-events.js";
import { createAppServerEventNormalizer } from "../src/app-server-events.js";
import type { AppServerNotification } from "../src/app-server.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";

/**
 * THE STEPS SHOW WHAT THE RUNTIMES SHOW (0.489). Colin, 2026-09-30: "most
 * users will not be happy having things hidden or not visible". Measured in
 * docs/DISPLAY-COVERAGE-2026-09-30.md: Claude's and Codex's command output was
 * never carried (gap 5), thinking left no mark (gap 6), and Codex's live line
 * read "Thinking" through the answer and "writing" for the person's own
 * message (gap 11).
 */
const NOW = "2026-09-30T10:00:00.000Z";
let line = 0;
const record = (value: unknown) => ({ sequence: (line += 1), raw: JSON.stringify(value) });
const payload = <T>(event: NormalizedRuntimeEvent | undefined): T => (event as unknown as { payload: T }).payload;

const claudeCall = (name: string, content: unknown) => {
  const claude = createClaudeEventNormalizer({ runId: "run_1", now: () => new Date(NOW) });
  return [
    ...claude.accept(record({ type: "stream_event", event: { type: "content_block_start", index: 0, content_block: { type: "tool_use", id: "t1", name } } })),
    ...claude.accept(record({ type: "assistant", message: { content: [{ type: "tool_use", id: "t1", name, input: { command: "npm test" } }] } })),
    ...claude.accept(record({ type: "user", message: { content: [{ type: "tool_result", tool_use_id: "t1", content }] } })),
  ];
};

describe("Claude: what a step printed", () => {
  it("carries a command's output, as Claude Code shows it under the command", () => {
    const done = claudeCall("Bash", "Tests  6 passed (6)").find((event) => event.type === "tool.completed");
    expect(payload<{ output?: string }>(done).output).toBe("Tests  6 passed (6)");
  });

  it("reads output given as text blocks", () => {
    const done = claudeCall("Grep", [{ type: "text", text: "src/a.ts" }, { type: "text", text: "src/b.ts" }]).find((event) => event.type === "tool.completed");
    expect(payload<{ output?: string }>(done).output).toBe("src/a.ts\nsrc/b.ts");
  });

  it("does not carry a file's text for a Read, which Claude Code never shows", () => {
    const done = claudeCall("Read", "1\tsecret line").find((event) => event.type === "tool.completed");
    expect(payload<{ output?: string }>(done).output).toBeUndefined();
    expect(claudeShowsOutput("Edit")).toBe(false);
    expect(claudeShowsOutput("mcp__github__list_issues")).toBe(true);
  });

  it("scrubs a key in the output", () => {
    const done = claudeCall("Bash", "token=sk-abcdefghijklmnopqrstuv").find((event) => event.type === "tool.completed");
    expect(payload<{ output?: string }>(done).output).not.toContain("sk-abcdefghijklmnopqrstuv");
  });
});

describe("Claude: thinking, timed and kept", () => {
  it("opens and closes a reasoning step around a thinking block, with its shown text", () => {
    const claude = createClaudeEventNormalizer({ runId: "run_1", now: () => new Date(NOW) });
    const events = [
      ...claude.accept(record({ type: "stream_event", event: { type: "message_start", message: { id: "msg_1", usage: {} } } })),
      ...claude.accept(record({ type: "stream_event", event: { type: "content_block_start", index: 0, content_block: { type: "thinking", thinking: "" } } })),
      ...claude.accept(record({ type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "thinking_delta", thinking: "shown words" } } })),
      ...claude.accept(record({ type: "stream_event", event: { type: "content_block_stop", index: 0 } })),
    ];
    const steps = events.filter((event) => (event.type === "step.started" || event.type === "step.completed") && payload<{ stepKind: string }>(event).stepKind === "reasoning");
    expect(steps.map((event) => [event.type, payload<{ stepKind: string }>(event).stepKind])).toEqual([
      ["step.started", "reasoning"],
      ["step.completed", "reasoning"],
    ]);
    expect(steps.at(-1)?.payload).toMatchObject({ message: "shown words" });
  });
});

describe("Codex's app server", () => {
  const note = (method: string, params: unknown): AppServerNotification => ({ method, params: params as never });
  const app = () => createAppServerEventNormalizer({ runId: "run_1", missionId: "mission_1", runtime: "codex", now: () => new Date(NOW) });

  it("carries a command's output", () => {
    const codex = app();
    codex.accept(note("item/started", { threadId: "t", turnId: "u", item: { type: "commandExecution", id: "c1", command: "npm test" } }));
    const events = codex.accept(note("item/completed", { threadId: "t", turnId: "u", item: { type: "commandExecution", id: "c1", command: "npm test", status: "completed", exitCode: 0, aggregatedOutput: "6 passed" } }));
    expect(payload<{ output?: string }>(events.find((event) => event.type === "tool.completed")).output).toBe("6 passed");
  });

  it("closes a reasoning item, so 'Thinking' leaves the live line", () => {
    const codex = app();
    const opened = codex.accept(note("item/started", { threadId: "t", turnId: "u", item: { type: "reasoning", id: "r1" } }));
    const closed = codex.accept(note("item/completed", { threadId: "t", turnId: "u", item: { type: "reasoning", id: "r1" } }));
    expect(opened.map((event) => event.type)).toEqual(["step.started"]);
    expect(closed.map((event) => event.type)).toEqual(["step.completed"]);
  });

  it("does not read the person's own message as the teammate writing", () => {
    const codex = app();
    expect(codex.accept(note("item/started", { threadId: "t", turnId: "u", item: { type: "userMessage", id: "u1", content: [] } }))).toEqual([]);
  });
});
