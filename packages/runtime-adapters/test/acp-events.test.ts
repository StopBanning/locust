import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { ACP_DECLINED, ACP_PROMPT_RESULT, ACP_SESSION, createAcpEventNormalizer } from "../src/acp-events.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";
import type { RuntimeJsonlRecord, RuntimeProcessCompletion } from "../src/process-runner.js";

/**
 * THE AGENT CLIENT PROTOCOL, READ AS LOCUST'S RECORD (0.377).
 *
 * Fed from the transcripts measured on this machine (fixtures/acp/), turned
 * into records the way acp-run.ts writes them: the session's id once a
 * session call answers; every update once the prompt has been SENT -- a
 * loaded session replays its whole history before that, and the past is not
 * this run; a refusal the moment the person's "no" is sent; each prompt's end.
 */
type ProbeMessage = { id?: number; method?: string; params?: Record<string, unknown>; result?: Record<string, unknown> };
type ProbeLine = { dir: string; message?: ProbeMessage };

function linesOf(fixture: string): ProbeLine[] {
  return readFileSync(new URL(`./fixtures/acp/${fixture}`, import.meta.url), "utf8")
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line) as ProbeLine);
}

function recordsFrom(fixture: string): RuntimeJsonlRecord[] {
  const records: RuntimeJsonlRecord[] = [];
  const push = (method: string, params: unknown): void => {
    records.push({ sequence: records.length + 1, raw: JSON.stringify({ method, params }) });
  };
  const sessionCalls = new Map<number, string | undefined>();
  const asked = new Map<number, { toolCallId: string; options: Array<{ optionId: string; kind: string }> }>();
  let prompted = false;
  for (const line of linesOf(fixture)) {
    const message = line.message;
    if (message === undefined) continue;
    if (line.dir === "out") {
      if (message.method === "session/new" || message.method === "session/load" || message.method === "session/resume") {
        sessionCalls.set(message.id!, message.params?.sessionId as string | undefined);
      }
      if (message.method === "session/prompt") prompted = true;
      const chosen = (message.result?.outcome as { optionId?: string } | undefined)?.optionId;
      const request = message.method === undefined && message.id !== undefined ? asked.get(message.id) : undefined;
      const kind = request?.options.find((option) => option.optionId === chosen)?.kind ?? "";
      if (request !== undefined && kind.startsWith("reject")) push(ACP_DECLINED, { toolCallId: request.toolCallId });
      continue;
    }
    if (line.dir !== "in") continue;
    if (message.id !== undefined && message.result !== undefined && sessionCalls.has(message.id)) {
      push(ACP_SESSION, { sessionId: message.result.sessionId ?? sessionCalls.get(message.id) });
    }
    if (message.method === "session/update" && prompted) push("session/update", message.params);
    if (message.method === "session/request_permission" && message.id !== undefined) {
      asked.set(message.id, {
        toolCallId: (message.params?.toolCall as { toolCallId: string }).toolCallId,
        options: message.params?.options as Array<{ optionId: string; kind: string }>,
      });
    }
    if (message.result?.stopReason !== undefined) push(ACP_PROMPT_RESULT, message.result);
  }
  return records;
}

const COMPLETION: RuntimeProcessCompletion = {
  exitCode: 0,
  signal: null,
  stderr: "",
  stderrTruncated: false,
  recordCount: 0,
  cancelled: false,
  forcedTerminationAttempted: false,
  terminationUnconfirmed: false,
  inputDeliveryFailed: false,
  outputLimitExceeded: false,
  oversizedRecordsDropped: 0,
  startedAt: "2026-09-26T12:00:00.000Z",
  finishedAt: "2026-09-26T12:00:05.000Z",
};

function normalize(records: readonly RuntimeJsonlRecord[], runtime: "opencode" | "copilot", completion: RuntimeProcessCompletion = COMPLETION): NormalizedRuntimeEvent[] {
  const normalizer = createAcpEventNormalizer({ runId: "run_1", missionId: "mission_1", runtime, now: () => new Date("2026-09-26T12:00:00.000Z") });
  return [...records.flatMap((record) => normalizer.accept(record)), ...normalizer.finish(completion)];
}
const payload = (event: NormalizedRuntimeEvent | undefined): Record<string, unknown> => (event?.payload ?? {}) as unknown as Record<string, unknown>;
/** What the run SAID: each message as it was finally marked whole. */
const said = (events: readonly NormalizedRuntimeEvent[]): unknown[] =>
  events.filter((event) => event.type === "message.delta" && payload(event).final === true).map((event) => payload(event).text);

describe("an ACP turn, from OpenCode's own stream", () => {
  const events = normalize(recordsFrom("opencode-1.18.27-bash-rejected.jsonl"), "opencode");

  it("starts, thinks, asks to run a command, is refused, and ends -- as the product's events", () => {
    expect(events.map((event) => event.type)).toEqual(["run.started", "step.started", "step.completed", "tool.started", "tool.failed", "run.completed"]);
    expect(events.every((event) => event.sourceAdapter === "opencode")).toBe(true);
    expect(events.at(-1)?.runtimeThreadId).toBe("ses_f1fddcf4bffeVoJmoJ2fz70R86");
  });

  it("records the refused call as declined, with its command -- not as a failure", () => {
    const refused = events.find((event) => event.type === "tool.failed");
    expect(payload(refused)).toMatchObject({ toolKind: "command_execution", name: "bash", command: "echo ACP-PROBE", status: "declined" });
    // The first report named only the folder: the row waits for the command.
    expect(payload(events.find((event) => event.type === "tool.started")).command).toBeUndefined();
  });

  it("keeps what was thought on the thinking step, as every runtime's is -- and nothing of the machine's own commands", () => {
    expect(JSON.stringify(events)).not.toContain("example-command");
    const thought = linesOf("opencode-1.18.27-bash-rejected.jsonl")
      .map((line) => line.message?.params?.update as { sessionUpdate?: string; content?: { text?: string } } | undefined)
      .filter((update) => update?.sessionUpdate === "agent_thought_chunk")
      .map((update) => update?.content?.text ?? "")
      .join("")
      .trim();
    expect(thought.length).toBeGreaterThan(20);
    expect(payload(events.find((event) => event.type === "step.completed"))).toMatchObject({ stepKind: "reasoning", message: thought });
    // On the step, and only there: no record's evidence carries it too.
    const inEvidence = events.filter((event) => JSON.stringify(payload(event).evidence ?? {}).includes(thought.slice(0, 20)));
    expect(inEvidence).toEqual([]);
  });

  it("ends with the turn's usage", () => {
    expect(payload(events.at(-1)).usage).toMatchObject({ inputTokens: 11569, outputTokens: 28 });
  });
});

describe("an ACP turn, from Copilot's own stream", () => {
  const events = normalize(recordsFrom("copilot-1.0.88-bash-rejected.jsonl"), "copilot");

  it("names the call by what the agent said it was for, and the command it asked to run", () => {
    const started = events.find((event) => event.type === "tool.started");
    expect(payload(started)).toMatchObject({ name: "bash", command: "echo ACP-PROBE", title: "Run requested probe" });
    expect(payload(events.find((event) => event.type === "tool.failed")).status).toBe("declined");
    expect(events.at(-1)?.type).toBe("run.completed");
    expect(events.every((event) => event.sourceAdapter === "copilot")).toBe(true);
  });
});

describe("a Copilot session loaded over ACP, a command approved, and the answer (measured 2026-09-26)", () => {
  const events = normalize(recordsFrom("copilot-1.0.88-load-approved.jsonl"), "copilot");

  it("keeps none of the conversation the load replayed -- only the turn it was sent", () => {
    // The replay carried the first turn's "Remember the word PLUM-3" and its "OK".
    expect(said(events)).toEqual(["ACP-OK-7 PLUM-3"]);
    expect(JSON.stringify(events)).not.toContain("Remember the word");
  });

  it("runs the approved command, and keeps what it printed", () => {
    expect(events.map((event) => event.type).filter((type) => type !== "message.delta")).toEqual(["run.started", "tool.started", "tool.completed", "run.completed"]);
    expect(payload(events.find((event) => event.type === "tool.completed"))).toMatchObject({
      toolKind: "command_execution",
      name: "bash",
      command: "echo ACP-OK-7",
      title: "Print the required value from the shell",
      status: "completed",
      output: expect.stringContaining("ACP-OK-7") as unknown,
    });
  });

  it("names the session it loaded, and what the turn cost", () => {
    expect(events.at(-1)).toMatchObject({ type: "run.completed", runtimeThreadId: "3abe5d9d-b422-4bf5-8c95-97d605d3f819" });
    expect(payload(events.at(-1)).usage).toMatchObject({ inputTokens: 22408, outputTokens: 57 });
  });
});

describe("what the agent says", () => {
  const record = (sequence: number, method: string, params: unknown): RuntimeJsonlRecord => ({ sequence, raw: JSON.stringify({ method, params }) });
  const chunk = (sequence: number, text: string, messageId?: string): RuntimeJsonlRecord =>
    record(sequence, "session/update", { sessionId: "s", update: { sessionUpdate: "agent_message_chunk", ...(messageId === undefined ? {} : { messageId }), content: { type: "text", text } } });
  const shell = (sequence: number, status: "pending" | "completed"): RuntimeJsonlRecord =>
    record(sequence, "session/update", { update: { sessionUpdate: status === "pending" ? "tool_call" : "tool_call_update", toolCallId: "t1", kind: "execute", status, rawInput: { command: "npm test" } } });
  const ended = (sequence: number): RuntimeJsonlRecord => record(sequence, ACP_PROMPT_RESULT, { stopReason: "end_turn" });

  it("sent whole rather than token by token, is bounded like every other message -- and keeps both ends (review 2026-10-15, C1)", () => {
    // An agent that sends its answer in one chunk is what "bring any agent" is for; a
    // 17,000 character chunk used to be refused by the ledger and stop the run.
    const long = `START ${"w".repeat(200_000)} END`;
    const events = normalize([record(1, ACP_SESSION, { sessionId: "s" }), chunk(2, long, "m1"), chunk(3, " more", "m1"), ended(4)], "copilot");
    const deltas = events.filter((event) => event.type === "message.delta").map(payload);
    expect(deltas.map((delta) => (delta.text as string).length).every((length) => length <= 131_072)).toBe(true);
    const whole = said(events)[0] as string;
    expect(whole.startsWith("START ")).toBe(true);
    expect(whole.endsWith("END more")).toBe(true);
    // The control: an ordinary chunk passes through as it came.
    expect(deltas[1]).toMatchObject({ operation: "append", text: " more" });
  });

  it("streams as it is written, and is marked whole and final when the turn ends", () => {
    const events = normalize([record(1, ACP_SESSION, { sessionId: "s" }), chunk(2, "Done: ", "m1"), chunk(3, "port is 3001.", "m1"), ended(4)], "copilot");
    const deltas = events.filter((event) => event.type === "message.delta").map(payload);
    expect(deltas).toEqual([
      expect.objectContaining({ itemId: "m1", operation: "append", text: "Done: ", final: false }),
      expect.objectContaining({ itemId: "m1", operation: "append", text: "port is 3001.", final: false }),
      expect.objectContaining({ itemId: "m1", operation: "replace", text: "Done: port is 3001.", final: true }),
    ]);
  });

  it("what is said before a tool call and after it are two messages, each whole -- Copilot names neither", () => {
    const events = normalize([chunk(1, "I'll run the tests."), shell(2, "pending"), shell(3, "completed"), chunk(4, "All 12 pass."), ended(5)], "copilot");
    expect(said(events)).toEqual(["I'll run the tests.", "All 12 pass."]);
    const finals = events.filter((event) => event.type === "message.delta" && payload(event).final === true).map((event) => payload(event).itemId);
    expect(new Set(finals).size).toBe(2);
    // In the order it happened: the first message is whole before the call starts.
    expect(events.map((event) => event.type).indexOf("tool.started")).toBeGreaterThan(events.findIndex((event) => payload(event).final === true));
  });

  it("an agent that keeps one message id across a tool call gets one item per stretch, so neither replaces the other", () => {
    const events = normalize([chunk(1, "Checking.", "m1"), shell(2, "pending"), shell(3, "completed"), chunk(4, "Fine.", "m1"), ended(5)], "opencode");
    const finals = events.filter((event) => event.type === "message.delta" && payload(event).final === true).map(payload);
    expect(finals).toEqual([expect.objectContaining({ itemId: "m1", text: "Checking." }), expect.objectContaining({ itemId: "m1:2", text: "Fine." })]);
  });

  it("the person's own words, replayed, are never taken for the agent's", () => {
    const events = normalize([record(1, "session/update", { update: { sessionUpdate: "user_message_chunk", content: { type: "text", text: "Remember PLUM-3." } } }), chunk(2, "OK."), ended(3)], "copilot");
    expect(said(events)).toEqual(["OK."]);
    expect(events.filter((event) => event.type === "adapter.diagnostic")).toEqual([]);
  });

  it("an edit carries its change as a patch, worked out from before and after", () => {
    const events = normalize(
      [
        record(1, "session/update", { update: { sessionUpdate: "tool_call", toolCallId: "t1", kind: "edit", title: "Edit app.js", status: "pending", locations: [{ path: "app.js" }] } }),
        record(2, "session/update", {
          update: {
            sessionUpdate: "tool_call_update",
            toolCallId: "t1",
            status: "completed",
            content: [{ type: "diff", path: "app.js", oldText: "const port = 3000\nlisten(port)\n", newText: "const port = 3001\nlisten(port)\n" }],
          },
        }),
        ended(3),
      ],
      "copilot",
    );
    const done = payload(events.find((event) => event.type === "tool.completed"));
    expect(done).toMatchObject({ toolKind: "file_change", name: "edit", command: "app.js", status: "completed" });
    expect(done.patch).toMatchObject({ added: 1, removed: 1, truncated: false });
  });

  it("a plan goes to the plan card as the agent wrote it", () => {
    const events = normalize([record(1, "session/update", { update: { sessionUpdate: "plan", entries: [{ content: "Read app.js", priority: "high", status: "completed" }, { content: "Change the port", priority: "high", status: "in_progress" }] } })], "copilot");
    expect(payload(events.find((event) => event.type === "plan.updated")).plan).toEqual([
      { content: "Read app.js", status: "completed" },
      { content: "Change the port", status: "in_progress" },
    ]);
  });
});

describe("how a turn ends", () => {
  const ended = (records: readonly RuntimeJsonlRecord[], completion: Partial<RuntimeProcessCompletion> = {}) =>
    normalize(records, "copilot", { ...COMPLETION, ...completion }).at(-1);
  const result = (stopReason: string, sequence = 1, usage?: Record<string, number>): RuntimeJsonlRecord => ({
    sequence,
    raw: JSON.stringify({ method: ACP_PROMPT_RESULT, params: { stopReason, ...(usage === undefined ? {} : { usage }) } }),
  });

  it("cancelled, by the person or by the agent's own answer", () => {
    expect(ended([result("cancelled")])?.type).toBe("run.cancelled");
    expect(ended([], { cancelled: true })?.type).toBe("run.cancelled");
  });

  it("failed when the agent ended before its turn did, or refused it -- and a CLI that is signed out says so", () => {
    expect(payload(ended([], { stderr: "copilot: not signed in" }))).toMatchObject({ kind: "authentication-failed", message: "copilot: not signed in", runtimeTerminal: "missing" });
    expect(payload(ended([], { stderr: "Authentication required" }))).toMatchObject({ kind: "authentication-failed" });
    expect(payload(ended([], { stderr: "The agent exited before its turn finished." }))).toMatchObject({ kind: "process-failed" });
    expect(payload(ended([result("refusal")]))).toMatchObject({ kind: "safety-blocked" });
  });

  it("cut off at a limit says so, beside the completion (2026-10-10 sweep)", () => {
    const events = normalize([result("max_tokens")], "copilot", COMPLETION);
    const warned = events.find((event) => event.type === "adapter.diagnostic");
    expect(payload(warned)).toMatchObject({ level: "warning", code: "acp.stopped_at_limit" });
    expect(events.at(-1)?.type).toBe("run.completed");
    expect(normalize([result("end_turn")], "copilot", COMPLETION).some((event) => event.type === "adapter.diagnostic")).toBe(false);
  });

  it("a run lost during a later prompt did not finish, though an earlier one did", () => {
    expect(payload(ended([result("end_turn")], { exitCode: null, stderr: "The agent exited before its turn finished." }))).toMatchObject({ kind: "process-failed" });
  });

  it("a second prompt -- a denial's reason, sent on -- is paid for as well as the first", () => {
    const last = ended([result("end_turn", 1, { inputTokens: 100, outputTokens: 10 }), result("end_turn", 2, { inputTokens: 150, outputTokens: 20 })]);
    expect(last?.type).toBe("run.completed");
    expect(payload(last).usage).toEqual({ inputTokens: 250, outputTokens: 30 });
  });
});
