import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { agyUserLine } from "../src/process-runner.js";
import { assertSafeRuntimeCommand, createAgyEventNormalizer, createAgyPrintCommand, parseAgyModelList } from "../src/index.js";
import { ANTIGRAVITY_QUOTA_SENTENCE, antigravityQuotaLimit } from "../src/agy-events.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";

/**
 * ANTIGRAVITY CLI (0.540). Every record below is a real `agy` 1.2.14 capture,
 * 2026-10-02, Gemini 3.8 Flash (Low), with the user name scrubbed and the
 * tool list shortened (test/fixtures/agy-2026-10-02/).
 */
const fixture = (name: string): string[] =>
  readFileSync(fileURLToPath(new URL(`./fixtures/agy-2026-10-02/${name}.jsonl`, import.meta.url)), "utf8").trim().split("\n");
const completion = (exitCode = 0) => ({
  exitCode, signal: null, stderr: "", stderrTruncated: false, recordCount: 0, inputDeliveryFailed: false,
  outputLimitExceeded: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, cancelled: false,
  startedAt: "2026-10-02T00:00:00.000Z", finishedAt: "2026-10-02T00:00:10.000Z",
}) as never;
const run = (name: string, exitCode = 0, sandbox?: "read-only" | "workspace-write" | "full-access") => {
  const normalizer = createAgyEventNormalizer({ runId: "run_1", missionId: "mission_1", now: () => new Date("2026-10-02T00:00:00Z"), ...(sandbox === undefined ? {} : { sandbox }) });
  const events: NormalizedRuntimeEvent[] = [];
  fixture(name).forEach((raw, index) => events.push(...normalizer.accept({ sequence: index + 1, raw } as never)));
  events.push(...normalizer.finish(completion(exitCode)));
  return { events, thread: normalizer.runtimeThreadId };
};
const payload = <T>(event: NormalizedRuntimeEvent | undefined): T => (event as unknown as { payload: T }).payload;
const types = (events: readonly NormalizedRuntimeEvent[]) => events.map((event) => event.type);

describe("Antigravity CLI's stream", () => {
  it("reads a file: one tool row, the answer whole, the conversation to carry on, and usage", () => {
    const { events, thread } = run("reads-a-file");
    expect(thread).toBe("c0da4624-b6cd-4e9e-9979-87b67b7abe60");
    const tools = events.filter((event) => event.type.startsWith("tool."));
    expect(types(tools)).toEqual(["tool.started", "tool.completed"]);
    expect(payload<{ name: string; command: string }>(tools[0])).toMatchObject({ name: "view_file", command: "C:/Users/person/Documents/locust-scratch/agy-probe/note.txt" });
    const final = events.filter((event) => event.type === "message.delta").at(-1);
    expect(payload<{ text: string; final: boolean; operation: string }>(final)).toMatchObject({ text: "hello from a file\nDONE\n", final: true, operation: "replace" });
    const done = events.at(-1)!;
    expect(done.type).toBe("run.completed");
    expect(payload<{ usage: unknown }>(done).usage).toEqual({ inputTokens: 20836, outputTokens: 173 });
  });

  it("the answer arrived whole, then the service was unavailable: finished, with a line saying so (0.657)", () => {
    const { events } = run("answered-then-unavailable", 1);
    const final = events.filter((event) => event.type === "message.delta").at(-1);
    expect(payload<{ text: string; final: boolean }>(final)).toMatchObject({ text: "hello from a file\nDONE\n", final: true });
    const said = events.find((event) => event.type === "adapter.diagnostic");
    expect(payload<{ level: string; code: string; message: string }>(said)).toMatchObject({ level: "warning", code: "antigravity.unavailable_after_answer" });
    expect(payload<{ message: string }>(said).message).toMatch(/send "continue"/);
    expect(events.at(-1)!.type).toBe("run.completed");
    expect(events.some((event) => event.type === "run.failed")).toBe(false);
  });

  it("a sentence said before a step is not said again on top of the answer (0.668)", () => {
    const normalizer = createAgyEventNormalizer({ runId: "run_1", missionId: "mission_1", now: () => new Date("2026-10-02T00:00:00Z") });
    const step = (index: number, fields: Record<string, unknown>) => JSON.stringify({ event: "step_update", step_update: { conversation_id: "c1", step_index: index, ...fields } });
    const lines = [
      '{"event":"init","conversation_id":"c1"}',
      step(1, { step_type: "agent_response", state: "DONE", text_delta: "I am about to read note.txt." }),
      step(2, { step_type: "tool", state: "DONE", tool_name: "view_file", tool_info: { parameters: { AbsolutePath: "C:/w/note.txt" } } }),
      step(3, { step_type: "agent_response", state: "DONE", text_delta: "It says hello. FINISHED" }),
      '{"event":"result","result":{"conversation_id":"c1","status":"SUCCESS","response":"I am about to read note.txt.\\nIt says hello. FINISHED"}}'
    ];
    const events = [...lines.flatMap((raw, index) => normalizer.accept({ sequence: index + 1, raw } as never)), ...normalizer.finish(completion(0))];
    const final = events.filter((event) => event.type === "message.delta").at(-1);
    expect(payload<{ text: string; final: boolean; itemId: string }>(final)).toMatchObject({ text: "It says hello. FINISHED", final: true, itemId: "msg_3" });
  });

  /*
   * Gemini 4 Argon's voxel station, 2026-10-16 (agy, Antigravity CLI): the answer's own step DONE, the whole answer
   * in the result, then status ERROR, "The stream was interrupted. Please continue the task you were working on.",
   * exit 0 -- and Locust drew "failed" with "Try again" over a finished 91 KB page.
   */
  const interrupted = (after: readonly string[]) => {
    const normalizer = createAgyEventNormalizer({ runId: "run_1", missionId: "mission_1", now: () => new Date("2026-10-02T00:00:00Z") });
    const step = (index: number, fields: Record<string, unknown>) => JSON.stringify({ event: "step_update", step_update: { conversation_id: "c1", step_index: index, ...fields } });
    const lines = [
      '{"event":"init","conversation_id":"c1"}',
      step(28, { step_type: "tool", state: "DONE", tool_name: "write_to_file", tool_info: { parameters: { TargetFile: "C:/w/index.html" } } }),
      step(30, { step_type: "agent_response", state: "ACTIVE", text_delta: "I have built the station in index.html." }),
      step(30, { step_type: "agent_response", state: "DONE", text_delta: " Open it in Chrome." }),
      ...after,
      '{"event":"result","result":{"conversation_id":"c1","status":"ERROR","response":"I have built the station in index.html. Open it in Chrome.","error":"The stream was interrupted. Please continue the task you were working on.","duration_seconds":1109.15}}'
    ];
    return [...lines.flatMap((raw, index) => normalizer.accept({ sequence: index + 1, raw } as never)), ...normalizer.finish(completion(0))];
  };

  it("the answer closed, then the stream was \"interrupted\": finished, with a line saying so (0.779)", () => {
    const events = interrupted([]);
    const said = events.find((event) => event.type === "adapter.diagnostic" && payload<{ code: string }>(event).code === "antigravity.interrupted_after_answer");
    expect(payload<{ level: string; message: string }>(said)).toMatchObject({ level: "warning" });
    expect(payload<{ message: string }>(said).message).toMatch(/kept.*send "continue"/);
    expect(events.at(-1)!.type).toBe("run.completed");
    expect(events.some((event) => event.type === "run.failed")).toBe(false);
  });

  it("a stream interrupted while a tool was still going is still a failure", () => {
    const events = interrupted([JSON.stringify({ event: "step_update", step_update: { conversation_id: "c1", step_index: 31, step_type: "tool", state: "ACTIVE", tool_name: "run_command", tool_info: { parameters: { CommandLine: "npx serve" } } } })]);
    expect(events.at(-1)!.type).toBe("run.failed");
    expect(events.some((event) => event.type === "adapter.diagnostic" && payload<{ code: string }>(event).code === "antigravity.interrupted_after_answer")).toBe(false);
  });

  // 0.823, from the 2026-10-22 handoff drive (Gemini 4 Argon in Ask): the card pasted AGY_ERROR's raw JSON after the
  // sentence, and no "Usage limit reached" card said when it would reset.
  it("a quota used up is the limit card, with the reset as the instant it names", () => {
    const normalizer = createAgyEventNormalizer({ runId: "run_1", missionId: "mission_1", now: () => new Date("2026-10-22T17:03:44Z") });
    const raw = '{"event":"result","result":{"conversation_id":"c1","status":"ERROR","response":"","error":"Individual quota reached. Please upgrade your subscription to increase your limits. Resets in 33h54m24s."}}';
    const events = [...normalizer.accept({ sequence: 1, raw } as never), ...normalizer.finish(completion(1))];
    const limit = events.find((event) => event.type === "route.limit_detected");
    expect(payload<{ kind: string; message: string }>(limit)).toMatchObject({ kind: "quota-exhausted", message: "Antigravity's quota is used up · resets 2026-10-24T02:58:08Z" });
    expect(events.at(-1)!.type).toBe("run.failed");
    // Under the card, one sentence that says what to do: not the card's fact again with an upgrade pitch (the packaged
    // 0.823 drive showed "Please upgrade your subscription to increase your limits" there).
    expect(payload<{ message: string }>(events.at(-1)).message).toBe(ANTIGRAVITY_QUOTA_SENTENCE);
    expect(ANTIGRAVITY_QUOTA_SENTENCE).not.toMatch(/upgrade|subscription/i);
    expect(antigravityQuotaLimit("RESOURCE_EXHAUSTED", new Date(0))).toBe("Antigravity's quota is used up");
    expect(antigravityQuotaLimit("API error (attempt 1): UNAVAILABLE (code 503): The service is currently unavailable.", new Date(0))).toBeUndefined();
  });

  it("a 503 with no answer is still a failure", () => {
    const normalizer = createAgyEventNormalizer({ runId: "run_1", missionId: "mission_1", now: () => new Date("2026-10-02T00:00:00Z") });
    const raw = '{"event":"result","result":{"conversation_id":"c1","status":"ERROR","response":"","error":"API error (attempt 1): UNAVAILABLE (code 503): The service is currently unavailable."}}';
    const events = [...normalizer.accept({ sequence: 1, raw } as never), ...normalizer.finish(completion(1))];
    expect(events.at(-1)!.type).toBe("run.failed");
    expect(events.some((event) => event.type === "adapter.diagnostic" && payload<{ code: string }>(event).code === "antigravity.unavailable_after_answer")).toBe(false);
  });

  it("a write in the default mode is refused, its row says so, and the person is told how to allow it", () => {
    const { events } = run("write-denied");
    expect(types(events.filter((event) => event.type.startsWith("tool.")))).toEqual(["tool.started", "tool.failed"]);
    const said = events.find((event) => event.type === "adapter.diagnostic");
    expect(payload<{ message: string; level: string }>(said)).toMatchObject({ level: "warning", message: "It stopped there. Antigravity was not allowed to change files in this mode. Choose Edit or Auto to let it." });
    expect(events.at(-1)!.type).toBe("run.completed");
  });

  it("in Edit, the file is written and the command refused, said as such", () => {
    const { events } = run("edit-then-command-denied", 0, "workspace-write");
    const tools = events.filter((event) => event.type.startsWith("tool."));
    expect(tools.map((event) => `${event.type}:${payload<{ name: string }>(event).name}`)).toEqual([
      "tool.started:write_to_file", "tool.completed:write_to_file", "tool.started:run_command", "tool.failed:run_command",
    ]);
    expect(payload<{ message: string }>(events.find((event) => event.type === "adapter.diagnostic")).message).toMatch(/^It stopped there. Antigravity was not allowed to run a command in this mode/);
    // The command never ran: refused, with its reason, not failed (0.541).
    expect(payload<{ status: string; output: string }>(tools.at(-1))).toMatchObject({ status: "refused", output: "Not allowed in this mode." });
  });

  it("a refusal agy reports as DONE is refused once the result names it (0.550, Sol on 0.546)", () => {
    // Sol's own Ask run, from the beta ledger: the command ends DONE with no output
    // or error, and only the result's denied_actions says it never ran.
    const { events } = run("ask-command-denied-done");
    const tools = events.filter((event) => event.type.startsWith("tool."));
    expect(types(tools)).toEqual(["tool.started", "tool.failed"]);
    expect(payload<{ status: string; command: string }>(tools[1])).toMatchObject({ status: "refused", command: "git status" });
    // The step settles before the note that says why.
    expect(events.findIndex((event) => event.type === "tool.failed")).toBeLessThan(events.findIndex((event) => event.type === "adapter.diagnostic"));
  });

  it("a write that ran with nothing to show is still done when the result refused something else", () => {
    const { events } = run("edit-then-command-denied");
    const write = events.find((event) => event.type.startsWith("tool.") && event.type !== "tool.started" && payload<{ name: string }>(event).name === "write_to_file");
    expect(write?.type).toBe("tool.completed");
  });

  it("a refusal agy explains before it stops is still refused (0.551, Boss on 0.550)", () => {
    // Sol's record with a sentence put between the refused call and the result.
    const { events } = run("ask-command-denied-then-said");
    const settled = events.filter((event) => event.type === "tool.completed" || event.type === "tool.failed");
    expect(types(settled)).toEqual(["tool.failed"]);
    expect(payload<{ status: string }>(settled[0])).toMatchObject({ status: "refused" });
  });

  it("plan mode: its own plan file is written, the change it went on to make is refused", () => {
    // Measured 2026-10-02, `--mode plan`: agy writes plan.md into its brain folder, then tries the change.
    const { events } = run("plan-mode-writes-its-plan", 0, "read-only");
    const settled = events.filter((event) => event.type === "tool.completed" || event.type === "tool.failed");
    const by = (end: string) => settled.find((event) => payload<{ command?: string }>(event).command?.endsWith(end));
    expect(by("plan.md")?.type).toBe("tool.completed");
    expect(by("greet.js")?.type).toBe("tool.failed");
    expect(payload<{ status: string }>(by("greet.js"))).toMatchObject({ status: "refused" });
  });

  it("in Edit an edit is never held: it is done as it ends", () => {
    const { events } = run("edit-then-command-denied", 0, "workspace-write");
    const write = events.findIndex((event) => event.type === "tool.completed" && payload<{ name: string }>(event).name === "write_to_file");
    const command = events.findIndex((event) => event.type === "tool.started" && payload<{ name: string }>(event).name === "run_command");
    expect(write).toBeGreaterThanOrEqual(0);
    expect(write).toBeLessThan(command);
  });

  it("an allowed command carries its command line and its output", () => {
    const { events } = run("command-allowed");
    const done = events.find((event) => event.type === "tool.completed");
    expect(payload<{ command: string; output: string }>(done)).toMatchObject({ command: "node -e \"console.log(40+2)\"", output: "42\n" });
  });

  it("an unknown model fails the run with agy's own reason, first line only", () => {
    const { events } = run("unknown-model", 1);
    const failed = events.at(-1)!;
    expect(failed.type).toBe("run.failed");
    expect(payload<{ message: string }>(failed).message).toBe("Antigravity could not run it: invalid model selection (--model \"no-such-model\" --effort \"\"): model no-such-model is not recognized as a known model or custom model in settings");
  });

  it("never records reasoning text: agy only counts it", () => {
    const { events } = run("reads-a-file");
    expect(JSON.stringify(events.map((event) => event.payload))).not.toMatch(/"thinking"|reasoning text/);
  });
});

describe("the agy command", () => {
  const launch = { executablePath: "C:\\Users\\a\\AppData\\Local\\agy\\bin\\agy.exe", prefixArgs: [] } as never;
  it("is print mode with the prompt on stdin, and each mode is the measured flag", () => {
    const ask = createAgyPrintCommand(launch, { workspacePath: "C:/work", sandbox: "read-only", model: "gemini-3.8-flash-low" });
    expect(ask.args).toEqual(["--output-format", "stream-json", "--input-format", "stream-json", "--model", "gemini-3.8-flash-low", "-p="]);
    expect(ask.stdin).toBe("agy-json");
    expect(createAgyPrintCommand(launch, { workspacePath: "C:/work", sandbox: "workspace-write" }).args).toContain("accept-edits");
    expect(createAgyPrintCommand(launch, { workspacePath: "C:/work", sandbox: "full-access" }).args).toContain("--dangerously-skip-permissions");
    expect(createAgyPrintCommand(launch, { workspacePath: "C:/work", sandbox: "workspace-write" }).args).not.toContain("--dangerously-skip-permissions");
    expect(createAgyPrintCommand(launch, { workspacePath: "C:/work", resumeThreadId: "c0da" }).args).toEqual(expect.arrayContaining(["--conversation", "c0da"]));
  });

  it("allows everything only under full access, and the flag stays banned for every other runtime", () => {
    const spec = createAgyPrintCommand(launch, { workspacePath: "C:/work", sandbox: "full-access" });
    expect(() => assertSafeRuntimeCommand({ ...spec, sandbox: "workspace-write" })).toThrow(/Forbidden runtime argument/);
    expect(() => assertSafeRuntimeCommand({ ...spec, runtime: "claude" }, "full-access")).toThrow(/Forbidden runtime argument/);
  });

  it("sends the prompt in the one input shape agy accepted", () => {
    expect(JSON.parse(agyUserLine("Fix it"))).toEqual({ event: "user", message: { role: "user", content: [{ type: "text", text: "Fix it" }] } });
  });

  it("reads agy models as printed", () => {
    const listed = parseAgyModelList("Fetching available models...\ngemini-3.8-flash-high\tGemini 3.8 Flash (High)\ngemini-3.1-pro-low\tGemini 3.1 Pro (Low)\nclaude-opus-4-6-thinking\tClaude Opus 4.6 (Thinking)\n");
    expect(listed?.models).toEqual([
      { id: "gemini-3.8-flash", displayName: "Gemini 3.8 Flash", efforts: ["high"] },
      { id: "gemini-3.1-pro", displayName: "Gemini 3.1 Pro", efforts: ["low"] },
      { id: "claude-opus-4-6-thinking", displayName: "Claude Opus 4.6 (Thinking)" },
    ]);
    // Each effort a row of its own in agy: one model here, efforts in order.
    const flash = parseAgyModelList("gemini-3.8-flash-high\tGemini 3.8 Flash (High)\ngemini-3.8-flash-low\tGemini 3.8 Flash (Low)\ngemini-3.8-flash-medium\tGemini 3.8 Flash (Medium)\n");
    expect(flash?.models).toEqual([{ id: "gemini-3.8-flash", displayName: "Gemini 3.8 Flash", efforts: ["low", "medium", "high"] }]);
  });

  it("thinking reads as its time, only where thinking tokens were spent (0.542)", () => {
    // reads-a-file: step 1 spent 62 thinking tokens over 4.39s before its tool.
    const thought = run("reads-a-file").events.filter((event) => event.type === "step.completed" && payload<{ stepKind: string }>(event).stepKind === "reasoning");
    expect(thought.map((event) => payload<{ durationMs: number; message?: string }>(event))).toEqual([expect.objectContaining({ durationMs: 4385 })]);
    expect(payload<{ message?: string }>(thought[0]).message).toBeUndefined();
    // command-allowed spent none: no line claims it thought.
    expect(run("command-allowed").events.some((event) => event.type.startsWith("step.") && payload<{ stepKind: string }>(event).stepKind === "reasoning")).toBe(false);
  });

  it("a conversation the CLI does not have ends the session and says why (0.541, a conversation the app began)", () => {
    // Real: `agy --conversation <an app conversation id>`, 2026-10-02.
    const { events } = run("conversation-not-found", 1);
    const failed = events.at(-1)!;
    expect(failed.type).toBe("run.failed");
    expect(payload<{ sessionEnded: boolean; message: string }>(failed)).toMatchObject({
      sessionEnded: true,
      message: "Antigravity CLI does not have this conversation: it began in the Antigravity app. Send it again and it starts fresh, with the conversation so far.",
    });
  });
});
