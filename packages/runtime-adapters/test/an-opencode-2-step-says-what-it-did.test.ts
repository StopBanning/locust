import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { ACP_PROMPT_RESULT, createAcpEventNormalizer } from "../src/acp-events.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";
import type { RuntimeJsonlRecord, RuntimeProcessCompletion } from "../src/process-runner.js";

/**
 * OpenCode 2's steps, measured 2026-10-21 (`_tools/probe-opencode-2-tool-rows.mjs`).
 *
 * A read can arrive with kind `other` and a title that is the tool word.
 * A shell's command arrives on a later update; the completion after it often
 * carries only the folder in `locations`. That folder is not the command.
 * Copilot's rows, which already name the command in the first report, stay
 * as they were.
 */

const FOLDER = "C:\\work\\scratch\\oc2-tool-rows-probe";
const FILE = `${FOLDER}\\note.txt`;

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
  startedAt: "2026-10-21T04:00:00.000Z",
  finishedAt: "2026-10-21T04:00:05.000Z",
};

function record(sequence: number, update: Record<string, unknown>): RuntimeJsonlRecord {
  return { sequence, raw: JSON.stringify({ method: "session/update", params: { update } }) };
}

function normalize(records: readonly RuntimeJsonlRecord[], runtime: "opencode" | "copilot" = "opencode"): NormalizedRuntimeEvent[] {
  const normalizer = createAcpEventNormalizer({ runId: "run_1", missionId: "mission_1", runtime, now: () => new Date("2026-10-21T04:00:00.000Z") });
  return [...records.flatMap((entry) => normalizer.accept(entry)), ...normalizer.finish(COMPLETION)];
}

const payload = (event: NormalizedRuntimeEvent | undefined): Record<string, unknown> => (event?.payload ?? {}) as unknown as Record<string, unknown>;
const ofType = (events: readonly NormalizedRuntimeEvent[], type: string): NormalizedRuntimeEvent | undefined => events.find((event) => event.type === type);

describe("an OpenCode 2 step says what it did", () => {
  it("names a read reported as other by the tool word in its title, and the file, not a tool called other", () => {
    const events = normalize([
      record(1, { sessionUpdate: "tool_call", toolCallId: "read-1", kind: "other", title: "read", status: "pending", rawInput: {}, locations: [] }),
      record(2, { sessionUpdate: "tool_call_update", toolCallId: "read-1", kind: "other", title: "read", status: "in_progress", rawInput: { path: FILE }, locations: [{ path: FILE }] }),
      record(3, { sessionUpdate: "tool_call_update", toolCallId: "read-1", status: "completed", locations: [{ path: FILE }] }),
    ]);
    const started = payload(ofType(events, "tool.started"));
    const completed = payload(ofType(events, "tool.completed"));
    expect(started).toMatchObject({ toolKind: "read", name: "read" });
    expect(started.command).toBeUndefined();
    expect(completed).toMatchObject({ toolKind: "read", name: "read", command: FILE, status: "completed" });
    expect(JSON.stringify(events)).not.toContain('"name":"other"');
  });

  it("names a read whose kind was already read, and keeps the file when the completion has only a location", () => {
    const events = normalize([
      record(1, { sessionUpdate: "tool_call", toolCallId: "read-1", kind: "read", title: "read", status: "pending", rawInput: {}, locations: [] }),
      record(2, { sessionUpdate: "tool_call_update", toolCallId: "read-1", kind: "read", title: "read", status: "in_progress", rawInput: { path: FILE }, locations: [{ path: FILE }] }),
      record(3, { sessionUpdate: "tool_call_update", toolCallId: "read-1", status: "completed", locations: [{ path: FILE }] }),
    ]);
    expect(payload(ofType(events, "tool.completed"))).toMatchObject({ name: "read", toolKind: "read", command: FILE });
  });

  it("keeps the shell command the update gave, and never the folder the completion still carries", () => {
    const events = normalize([
      record(1, { sessionUpdate: "tool_call", toolCallId: "sh-1", kind: "execute", title: "shell", status: "pending", rawInput: { cwd: FOLDER }, locations: [{ path: FOLDER }] }),
      record(2, { sessionUpdate: "tool_call_update", toolCallId: "sh-1", kind: "execute", title: "echo hi", status: "in_progress", rawInput: { command: "echo hi", workdir: FOLDER }, locations: [{ path: FOLDER }] }),
      record(3, { sessionUpdate: "tool_call_update", toolCallId: "sh-1", status: "completed", locations: [{ path: FOLDER }] }),
    ]);
    const started = payload(ofType(events, "tool.started"));
    const completed = payload(ofType(events, "tool.completed"));
    expect(started).toMatchObject({ toolKind: "command_execution", name: "bash" });
    expect(started.command).toBeUndefined();
    expect(completed).toMatchObject({ toolKind: "command_execution", name: "bash", command: "echo hi", status: "completed" });
    expect(completed.command).not.toBe(FOLDER);
    expect(JSON.stringify(completed)).not.toContain(FOLDER);
  });

  it("a shell reported as other, titled shell, is that command and not the folder it was started in", () => {
    const events = normalize([
      record(1, { sessionUpdate: "tool_call", toolCallId: "sh-1", kind: "other", title: "shell", status: "pending", rawInput: {}, locations: [{ path: FOLDER }] }),
      record(2, { sessionUpdate: "tool_call_update", toolCallId: "sh-1", kind: "other", title: "echo hi", status: "completed", rawInput: { command: "echo hi" }, locations: [{ path: FOLDER }] }),
    ]);
    const started = payload(ofType(events, "tool.started"));
    const completed = payload(ofType(events, "tool.completed"));
    expect(started.command).toBeUndefined();
    expect(started.name).not.toBe("other");
    expect(completed.command).toBe("echo hi");
    expect(JSON.stringify(completed)).not.toContain(FOLDER);
  });

  it("leaves Copilot's existing rows unchanged: the command and its sentence, from the first report", () => {
    const fixture = readFileSync(new URL("./fixtures/acp/copilot-1.0.88-bash-rejected.jsonl", import.meta.url), "utf8");
    const call = fixture
      .split("\n")
      .map((line) => (line.trim().length === 0 ? undefined : (JSON.parse(line) as { message?: { params?: { update?: Record<string, unknown> } } })))
      .map((line) => line?.message?.params?.update)
      .find((update) => update?.sessionUpdate === "tool_call");
    const events = normalize(
      [
        record(1, call ?? {}),
        record(2, { sessionUpdate: "tool_call_update", toolCallId: "call_GfYFYmhnqXIp2a9V7osocij6", kind: "execute", status: "failed", title: "Run requested probe", rawInput: { command: "echo ACP-PROBE" } }),
        { sequence: 3, raw: JSON.stringify({ method: ACP_PROMPT_RESULT, params: { stopReason: "end_turn" } }) },
      ],
      "copilot",
    );
    expect(payload(ofType(events, "tool.started"))).toMatchObject({
      toolKind: "command_execution",
      name: "bash",
      command: "echo ACP-PROBE",
      title: "Run requested probe",
    });
    expect(payload(ofType(events, "tool.failed"))).toMatchObject({ name: "bash", command: "echo ACP-PROBE", title: "Run requested probe", status: "failed" });
  });

  it("does not invent a tool from a sentence, which is how Copilot describes a call whose kind is other", () => {
    const events = normalize(
      [
        record(1, { sessionUpdate: "tool_call", toolCallId: "t", kind: "other", title: "Run requested probe", status: "pending", rawInput: { command: "echo ACP-PROBE" } }),
        record(2, { sessionUpdate: "tool_call_update", toolCallId: "t", kind: "other", title: "Run requested probe", status: "completed", rawInput: { command: "echo ACP-PROBE" } }),
      ],
      "copilot",
    );
    expect(payload(ofType(events, "tool.started"))).toMatchObject({ name: "other", toolKind: "other", command: "echo ACP-PROBE", title: "Run requested probe" });
    expect(payload(ofType(events, "tool.completed"))).toMatchObject({ name: "other", command: "echo ACP-PROBE", title: "Run requested probe" });
  });
});
