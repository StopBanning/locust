import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { createAcpEventNormalizer } from "../src/acp-events.js";
import type { AppServerRunProcess } from "../src/codex-app-server-run.js";
import {
  createOpenCodeAcpCommand,
  OPENCODE_ACP_PROFILE,
  startOpenCodeAcpRun,
} from "../src/opencode-acp-run.js";
import type { OpenCodeAcpRun, OpenCodeAcpRunOptions } from "../src/opencode-acp-run.js";
import type { RuntimeJsonlRecord } from "../src/process-runner.js";
import type { ExecutableLaunch } from "../src/types.js";

/** Scripted ACP v1 over stdio: no installed runtime, sockets or credentials. */
type Message = {
  id?: number;
  method?: string;
  params?: Record<string, unknown>;
  result?: Record<string, unknown>;
  error?: { code: number; message: string };
};

interface Agent {
  readonly attempt: number;
  readonly sent: Message[];
  readonly kill: ReturnType<typeof vi.fn>;
  readonly process: AppServerRunProcess;
  say(message: Message): void;
  answer(message: Message, result: Record<string, unknown>): void;
  close(error?: Error): void;
  exit(): void;
}

const FOLDER = "/workspace/recovery";
const EXECUTABLE = {
  commandName: "opencode",
  discoveredPath: "/installed/opencode",
  executablePath: "/installed/node",
  prefixArgs: ["/installed/opencode.mjs"],
  kind: "node-shim",
  env: { ELECTRON_RUN_AS_NODE: "1" },
} satisfies ExecutableLaunch;
const SESSION = { sessionId: "session-current" };
const INIT = { protocolVersion: 1, agentCapabilities: { loadSession: true } };
const MCP_SERVERS = [{ name: "browser-fixture", command: "node", args: ["/local/browser-bridge.mjs"], env: [{ name: "FIXTURE_SESSION", value: "fixture-only" }] }];
const update = (body: Record<string, unknown>): Message => ({ method: "session/update", params: { sessionId: SESSION.sessionId, update: body } });
const text = (value: string, messageId = "answer-current"): Message => update({ sessionUpdate: "agent_message_chunk", messageId, content: { type: "text", text: value } });

function fixture(
  script: (message: Message, agent: Agent) => void,
  options: Partial<OpenCodeAcpRunOptions> = {},
  acknowledgeExit = true,
) {
  const agents: Agent[] = [];
  const spawned: Array<{ executablePath: string; args: readonly string[]; env: Readonly<Record<string, string>> | undefined; cwd: string | undefined }> = [];
  const run = startOpenCodeAcpRun({
    executable: EXECUTABLE,
    workspacePath: FOLDER,
    prompt: "Inspect the project.",
    retryDelayMs: 10,
    maxRetryDelayMs: 15,
    shutdownTimeoutMs: 20,
    spawn: (executablePath, args, env, cwd) => {
      spawned.push({ executablePath, args, env, cwd });
      let receive: (chunk: string) => void = () => undefined;
      let close: (error?: Error) => void = () => undefined;
      const exits: Array<() => void> = [];
      let exited = false;
      const exit = (): void => {
        if (exited) return;
        exited = true;
        for (const listener of exits) listener();
      };
      const kill = vi.fn(() => { if (acknowledgeExit) queueMicrotask(exit); });
      const agent: Agent = {
        attempt: agents.length + 1,
        sent: [],
        kill,
        say: (message) => queueMicrotask(() => receive(`${JSON.stringify({ jsonrpc: "2.0", ...message })}\n`)),
        answer: (message, result) => agent.say({ id: message.id!, result }),
        close: (error) => queueMicrotask(() => close(error)),
        exit,
        process: {
          write: (line) => {
            const message = JSON.parse(line) as Message;
            agent.sent.push(message);
            script(message, agent);
          },
          kill,
          onData: (listener) => { receive = listener; },
          onExit: (listener) => { exits.push(listener); if (exited) queueMicrotask(listener); },
          onStdoutClose: (listener) => { close = listener; },
        },
      };
      agents.push(agent);
      return agent.process;
    },
    ...options,
  });
  return { run, agents, spawned, prompts: () => agents.flatMap((agent) => agent.sent.filter((message) => message.method === "session/prompt")) };
}

function ordinary(turn: (message: Message, agent: Agent) => void) {
  return (message: Message, agent: Agent): void => {
    if (message.method === "initialize") agent.answer(message, INIT);
    else if (message.method === "session/new") agent.answer(message, SESSION);
    else if (message.method === "session/prompt") turn(message, agent);
  };
}

async function flush(): Promise<void> {
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
}

async function recordsOf(run: OpenCodeAcpRun): Promise<RuntimeJsonlRecord[]> {
  await run.completion;
  const records: RuntimeJsonlRecord[] = [];
  for await (const record of run.records) records.push(record);
  return records;
}

async function eventsOf(run: OpenCodeAcpRun) {
  const normalizer = createAcpEventNormalizer({ runId: "opencode-acp-test", runtime: "opencode" });
  const records = await recordsOf(run);
  const receipt = await run.completion;
  return [...records.flatMap((record) => normalizer.accept(record)), ...normalizer.finish(receipt)];
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => { vi.clearAllTimers(); vi.useRealTimers(); });

describe("the opt-in OpenCode ACP profile", () => {
  it("passes the desktop launch environment and permission receipt through the profile", async () => {
    const launch = { sandbox: "read-only" as const, env: { OPENCODE_CONFIG_CONTENT: '{"agents":{"locust":{}}}', OPENCODE_CONFIG_PROJECT_DISABLE: "1" } };
    expect(createOpenCodeAcpCommand(EXECUTABLE, FOLDER, launch)).toMatchObject({ sandbox: "read-only", env: { ...EXECUTABLE.env, ...launch.env } });
    const f = fixture(ordinary((prompt, agent) => agent.answer(prompt, { stopReason: "end_turn" })), { launch });
    await flush();
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: 0 });
    expect(f.spawned[0]!.env).toEqual({ ...EXECUTABLE.env, ...launch.env });
  });

  it("reads effort options advertised after switching the selected model", async () => {
    const f = fixture((message, agent) => {
      if (message.method === "initialize") agent.answer(message, INIT);
      else if (message.method === "session/new") agent.answer(message, { ...SESSION, configOptions: [{ id: "model", currentValue: "provider/old" }] });
      else if (message.method === "session/set_config_option") agent.answer(message, { configOptions: [
        { id: "model", currentValue: "provider/new" },
        { id: "effort", currentValue: message.params?.configId === "effort" ? "high" : "low" },
      ] });
      else if (message.method === "session/prompt") agent.answer(message, { stopReason: "end_turn" });
    }, { requiredConfig: { model: "provider/new", effort: "high" } });
    await flush();
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: 0 });
    expect(f.agents[0]!.sent.filter((message) => message.method === "session/set_config_option").map((message) => message.params?.configId)).toEqual(["model", "effort"]);
    expect(f.prompts()).toHaveLength(1);
  });

  it("keeps OpenCode identity, launch prefix and environment on the stdio command", async () => {
    const command = createOpenCodeAcpCommand(EXECUTABLE, FOLDER);
    expect(OPENCODE_ACP_PROFILE).toEqual({ runtime: "opencode", transport: "stdio", protocolVersion: 1 });
    expect(command).toMatchObject({ runtime: "opencode", executablePath: EXECUTABLE.executablePath, args: ["/installed/opencode.mjs", "acp"], cwd: FOLDER, env: EXECUTABLE.env, stdin: "protocol", sandbox: "workspace-write" });
    const f = fixture(ordinary((prompt, agent) => agent.answer(prompt, { stopReason: "end_turn" })), { mcpServers: MCP_SERVERS });
    await f.run.completion;
    expect(f.spawned).toEqual([{ executablePath: EXECUTABLE.executablePath, args: command.args, env: EXECUTABLE.env, cwd: FOLDER }]);
    expect(f.agents[0]!.sent.find((message) => message.method === "session/new")?.params?.mcpServers).toEqual(MCP_SERVERS);
    await expect(f.run.failure).resolves.toBeUndefined();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("loads an ACP v1 session without recording its replayed history", async () => {
    const capabilities = vi.fn();
    const f = fixture((message, agent) => {
      if (message.method === "initialize") agent.answer(message, INIT);
      else if (message.method === "session/load") {
        agent.say(text("historical answer must stay out", "history"));
        agent.say(update({ sessionUpdate: "user_message_chunk", content: { type: "text", text: "historical prompt" } }));
        agent.answer(message, SESSION);
      } else if (message.method === "session/prompt") {
        agent.say(text("Current answer."));
        agent.answer(message, { stopReason: "end_turn", usage: { inputTokens: 7, outputTokens: 3 } });
      }
    }, { resumeSessionId: SESSION.sessionId, onCapabilities: capabilities, mcpServers: MCP_SERVERS });
    const events = await eventsOf(f.run);
    const sent = f.agents[0]!.sent;
    expect(sent.find((message) => message.method === "initialize")?.params).toEqual({ protocolVersion: 1, clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false } });
    expect(sent.find((message) => message.method === "session/load")?.params).toEqual({ sessionId: SESSION.sessionId, cwd: FOLDER, mcpServers: MCP_SERVERS });
    expect(sent.some((message) => message.method === "session/new")).toBe(false);
    expect(JSON.stringify(events)).not.toContain("historical");
    expect(events.flatMap((event) => event.type === "message.delta" && event.payload.final ? [event.payload.text] : [])).toEqual(["Current answer."]);
    expect(events.every((event) => event.sourceAdapter === "opencode")).toBe(true);
    expect(events.at(-1)).toMatchObject({ type: "run.completed", runtimeThreadId: SESSION.sessionId, payload: { usage: { inputTokens: 7, outputTokens: 3 } } });
    expect(capabilities).toHaveBeenCalledWith(expect.objectContaining({ continuesSessions: true }));
    await expect(f.run.failure).resolves.toBeUndefined();
  });

  it("maps permissions and successful and failed tool calls through the existing normalizer", async () => {
    const permission = vi.fn(async () => "allow_always" as const);
    let prompt: Message | undefined;
    const f = fixture((message, agent) => {
      if (message.method === "initialize") agent.answer(message, INIT);
      else if (message.method === "session/new") agent.answer(message, SESSION);
      else if (message.method === "session/prompt") {
        prompt = message;
        agent.say(update({ sessionUpdate: "tool_call", toolCallId: "tool-ok", kind: "execute", title: "Run the tests", status: "pending", rawInput: { command: "npm test" } }));
        agent.say({ id: 90, method: "session/request_permission", params: {
          sessionId: SESSION.sessionId, toolCall: { toolCallId: "tool-ok", kind: "execute", rawInput: { command: "npm test" } },
          options: [{ optionId: "once", kind: "allow_once", name: "Allow once" }, { optionId: "always", kind: "allow_always", name: "Always" }],
        } });
      } else if (message.id === 90 && message.result !== undefined) {
        agent.say(update({ sessionUpdate: "tool_call_update", toolCallId: "tool-ok", status: "completed", content: [{ type: "content", content: { type: "text", text: "All tests passed" } }] }));
        agent.say(update({ sessionUpdate: "tool_call", toolCallId: "tool-error", kind: "execute", status: "in_progress", rawInput: { command: "read missing-file" } }));
        agent.say(update({ sessionUpdate: "tool_call_update", toolCallId: "tool-error", status: "failed", content: [{ type: "content", content: { type: "text", text: "ENOENT: missing-file does not exist" } }] }));
        agent.say(text("Tests passed; the missing file was reported."));
        agent.answer(prompt!, { stopReason: "end_turn" });
      }
    }, { onPermission: permission });
    const events = await eventsOf(f.run);
    expect(permission).toHaveBeenCalledWith(expect.objectContaining({ toolCallId: "tool-ok", command: "npm test" }));
    expect(f.agents[0]!.sent.find((message) => message.id === 90)?.result).toEqual({ outcome: { outcome: "selected", optionId: "once" } });
    expect(events.filter((event) => event.type === "tool.started").map((event) => event.payload.itemId)).toEqual(["tool-ok", "tool-error"]);
    expect(events.find((event) => event.type === "tool.completed")).toMatchObject({ payload: { itemId: "tool-ok", status: "completed", output: "All tests passed" } });
    expect(events.find((event) => event.type === "tool.failed")).toMatchObject({ payload: { itemId: "tool-error", status: "failed", output: "ENOENT: missing-file does not exist" } });
    expect(events.every((event) => event.sourceAdapter === "opencode")).toBe(true);
    expect(events.at(-1)?.type).toBe("run.completed");
    expect(f.prompts()).toHaveLength(1);
    await expect(f.run.failure).resolves.toBeUndefined();
  });

  it("queues a second turn on the same ACP session without telemetry identity drift", async () => {
    let first: Message | undefined;
    const f = fixture(ordinary((prompt, agent) => {
      if (first === undefined) { first = prompt; agent.say(text("First answer.", "first")); return; }
      agent.say(text("Second answer.", "second"));
      agent.answer(prompt, { stopReason: "end_turn" });
    }));
    await flush();
    await expect(f.run.steer("Now inspect the tests.")).resolves.toBe(true);
    f.agents[0]!.answer(first!, { stopReason: "end_turn" });
    const records = await recordsOf(f.run);
    expect(f.prompts().map((message) => message.params)).toEqual([
      { sessionId: SESSION.sessionId, prompt: [{ type: "text", text: "Inspect the project." }] },
      { sessionId: SESSION.sessionId, prompt: [{ type: "text", text: "Now inspect the tests." }] },
    ]);
    expect(f.agents).toHaveLength(1);
    expect(records.map((record) => record.sequence)).toEqual(records.map((_, index) => index + 1));
    const normalizer = createAcpEventNormalizer({ runId: "multi-turn", runtime: "opencode" });
    const events = [...records.flatMap((record) => normalizer.accept(record)), ...normalizer.finish(await f.run.completion)];
    expect(events.flatMap((event) => event.type === "message.delta" && event.payload.final ? [event.payload.text] : [])).toEqual(["First answer.", "Second answer."]);
    expect(events.filter((event) => event.type === "run.completed")).toHaveLength(1);
    expect(events.every((event) => event.sourceAdapter === "opencode")).toBe(true);
    await expect(f.run.steer("Too late")).resolves.toBe(false);
  });

  it("retries a pre-prompt stdout close only after bounded startup backoff", async () => {
    const normal = ordinary((prompt, agent) => { agent.say(text("Recovered.")); agent.answer(prompt, { stopReason: "end_turn" }); });
    const f = fixture((message, agent) => {
      if (agent.attempt === 1 && message.method === "initialize") agent.close();
      else normal(message, agent);
    });
    await flush();
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(0);
    expect(f.agents[0]!.kill).toHaveBeenCalledTimes(1);
    await vi.advanceTimersByTimeAsync(9);
    expect(f.agents).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: 0, cancelled: false, terminationUnconfirmed: false });
    expect(f.agents).toHaveLength(2);
    expect(f.prompts()).toHaveLength(1);
    expect(JSON.stringify(await recordsOf(f.run))).toContain("Recovered.");
    await expect(f.run.failure).resolves.toBeUndefined();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("exhausts capped startup retries with a structured transport failure", async () => {
    const f = fixture((message, agent) => { if (message.method === "initialize") agent.close(); }, { maxStartupRetries: 100 });
    await flush();
    await vi.advanceTimersByTimeAsync(10);
    expect(f.agents).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(14);
    expect(f.agents).toHaveLength(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(f.agents).toHaveLength(3);
    await vi.advanceTimersByTimeAsync(15);
    await expect(f.run.failure).resolves.toMatchObject({ code: "transport-closed", stage: "startup", attempts: 4 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, cancelled: false, terminationUnconfirmed: false });
    expect(f.agents).toHaveLength(4);
    expect(f.agents.every((agent) => agent.kill.mock.calls.length === 1)).toBe(true);
    expect(f.prompts()).toHaveLength(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("cancels during startup backoff without creating a replacement process", async () => {
    const controller = new AbortController();
    const f = fixture((message, agent) => { if (message.method === "initialize") agent.close(); }, { signal: controller.signal, retryDelayMs: 100 });
    await flush();
    controller.abort();
    await expect(f.run.completion).resolves.toMatchObject({ cancelled: true, exitCode: null });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(0);
    await expect(f.run.failure).resolves.toBeUndefined();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("bounds the total startup deadline even when retry backoff remains", async () => {
    const f = fixture((message, agent) => { if (message.method === "initialize") agent.close(); }, { handshakeTimeoutMs: 25, retryDelayMs: 100, maxRetryDelayMs: 100 });
    await flush();
    await vi.advanceTimersByTimeAsync(25);
    await expect(f.run.failure).resolves.toMatchObject({ code: "startup-timeout", stage: "startup", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, cancelled: false });
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("never resends an admitted prompt after stdout closes during its tool call", async () => {
    const f = fixture(ordinary((_prompt, agent) => {
      agent.say(update({ sessionUpdate: "tool_call", toolCallId: "once-only", kind: "execute", status: "in_progress", rawInput: { command: "publish-once" } }));
      agent.close();
    }));
    await flush();
    await expect(f.run.failure).resolves.toMatchObject({ code: "transport-closed", stage: "prompt", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, cancelled: false });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(1);
    const events = await eventsOf(f.run);
    expect(events.filter((event) => event.type === "tool.started")).toHaveLength(1);
    expect(events.at(-1)?.type).toBe("run.failed");
  });

  it("never retries a prompt write exception after possible partial delivery", async () => {
    const f = fixture(ordinary(() => { throw new Error("Prompt write failed after partial delivery"); }));
    await flush();
    await expect(f.run.failure).resolves.toMatchObject({ code: "transport-closed", stage: "prompt", attempts: 1, message: expect.stringContaining("partial delivery") });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(1);
    expect(f.agents[0]!.kill).toHaveBeenCalledTimes(1);
  });

  it("reports authentication errors without treating them as reconnectable transport loss", async () => {
    const f = fixture((message, agent) => {
      if (message.method === "initialize") agent.say({ id: message.id!, error: { code: -32_000, message: "Sign in to OpenCode before starting ACP" } });
    });
    await flush();
    await expect(f.run.failure).resolves.toMatchObject({ code: "agent-error", stage: "startup", attempts: 1, message: "Sign in to OpenCode before starting ACP" });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(0);
  });

  it("rejects unsupported ACP protocol versions without reconnecting", async () => {
    const f = fixture((message, agent) => { if (message.method === "initialize") agent.answer(message, { protocolVersion: 2 }); });
    await flush();
    await expect(f.run.failure).resolves.toMatchObject({ code: "agent-error", stage: "startup", attempts: 1, message: expect.stringContaining("version 2") });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(0);
  });

  it("fails a refused required configuration without sending a prompt or retrying", async () => {
    const configOptions = [{ id: "model", type: "select", currentValue: "provider/other" }];
    const f = fixture((message, agent) => {
      if (message.method === "initialize") agent.answer(message, INIT);
      else if (message.method === "session/new") agent.answer(message, { ...SESSION, configOptions });
      else if (message.method === "session/set_config_option") agent.answer(message, { configOptions });
    }, { requiredConfig: { model: "provider/selected" } });
    await flush();
    await expect(f.run.failure).resolves.toMatchObject({ code: "agent-error", stage: "startup", attempts: 1, message: expect.stringContaining("would not set model to provider/selected") });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(0);
    expect(f.agents[0]!.sent.find((message) => message.method === "session/set_config_option")?.params).toEqual({ sessionId: SESSION.sessionId, configId: "model", value: "provider/selected" });
  });

  it("refuses replacement when the stopped process never acknowledges exit", async () => {
    const f = fixture((message, agent) => { if (message.method === "initialize") agent.close(); }, {}, false);
    await flush();
    await vi.advanceTimersByTimeAsync(19);
    expect(f.agents).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    await expect(f.run.failure).resolves.toMatchObject({ code: "termination-unconfirmed", stage: "startup", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, terminationUnconfirmed: true });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(0);
    expect(f.agents[0]!.kill).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("does not spawn a process for an already cancelled run", async () => {
    const controller = new AbortController();
    controller.abort();
    const f = fixture(ordinary(() => undefined), { signal: controller.signal });
    await expect(f.run.completion).resolves.toMatchObject({ cancelled: true, recordCount: 0 });
    expect(f.agents).toHaveLength(0);
    await expect(f.run.failure).resolves.toBeUndefined();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("bounds a non-finite handshake option without overflowing the inner timer", async () => {
    const f = fixture(() => undefined, { handshakeTimeoutMs: Infinity });
    let completed = false;
    void f.run.completion.then(() => { completed = true; });
    await vi.advanceTimersByTimeAsync(59_999);
    expect(completed).toBe(false);
    expect(f.agents).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);
    await expect(f.run.failure).resolves.toMatchObject({ code: "startup-timeout", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ cancelled: false, terminationUnconfirmed: false });
    expect(f.prompts()).toHaveLength(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("cleans up a spawned process when listener registration fails", async () => {
    let exit: () => void = () => undefined;
    const kill = vi.fn(() => exit());
    const f = fixture(() => undefined, { spawn: () => ({
      write: () => undefined, kill,
      onData: () => { throw new Error("Cannot subscribe to transport"); },
      onExit: (listener) => { exit = listener; },
    }) });
    await vi.advanceTimersByTimeAsync(20);
    await expect(f.run.failure).resolves.toMatchObject({ code: "process-start-failed", stage: "startup", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, terminationUnconfirmed: false });
    expect(kill).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("reports a failed spawn once without retrying an invalid command", async () => {
    const spawn = vi.fn(() => { throw new Error("ENOENT"); });
    const f = fixture(() => undefined, { spawn });
    await expect(f.run.failure).resolves.toMatchObject({ code: "process-start-failed", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, terminationUnconfirmed: false });
    await vi.advanceTimersByTimeAsync(1_000);
    expect(spawn).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps queued steering and excludes abandoned session receipts on a startup retry", async () => {
    let turn = 0;
    const normal = ordinary((prompt, agent) => {
      turn += 1;
      agent.say(text(`Answer ${turn}.`));
      agent.answer(prompt, { stopReason: "end_turn" });
    });
    const f = fixture((message, agent) => {
      if (agent.attempt === 1 && message.method === "session/new") {
        agent.answer(message, { sessionId: "abandoned-session", configOptions: [{ id: "model", currentValue: "provider/old" }] });
      } else if (agent.attempt === 1 && message.method === "session/set_config_option") {
        agent.close();
      } else if (message.method === "session/new") {
        agent.answer(message, { ...SESSION, configOptions: [{ id: "model", currentValue: "provider/selected" }] });
      } else normal(message, agent);
    }, { requiredConfig: { model: "provider/selected" } });
    await expect(f.run.steer("A queued follow-up.")).resolves.toBe(true);
    await flush();
    await vi.advanceTimersByTimeAsync(10);
    const records = await recordsOf(f.run);
    expect(f.agents).toHaveLength(2);
    expect(f.prompts().map((message) => message.params?.prompt)).toEqual([
      [{ type: "text", text: "Inspect the project." }],
      [{ type: "text", text: "A queued follow-up." }],
    ]);
    expect(JSON.stringify(records)).not.toContain("abandoned-session");
    expect(records.filter((record) => JSON.parse(record.raw).method === "locust/session")).toHaveLength(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("clears the startup deadline when a long-running prompt is admitted", async () => {
    let prompt: Message | undefined;
    const f = fixture(ordinary((message) => { prompt = message; }));
    await flush();
    let completed = false;
    void f.run.completion.then(() => { completed = true; });
    await vi.advanceTimersByTimeAsync(120_000);
    expect(completed).toBe(false);
    expect(f.agents[0]!.kill).not.toHaveBeenCalled();
    f.agents[0]!.answer(prompt!, { stopReason: "end_turn" });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: 0, cancelled: false });
    await expect(f.run.failure).resolves.toBeUndefined();
    expect(f.prompts()).toHaveLength(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("stops on record overflow without replacing the admitted process", async () => {
    const f = fixture(ordinary((prompt, agent) => {
      for (let i = 0; i < 10; i += 1) agent.say(text(`Chunk ${i}.`));
      agent.answer(prompt, { stopReason: "end_turn" });
    }), { maxQueuedRecords: 1, cancelGraceMs: 1 });
    await vi.advanceTimersByTimeAsync(20);
    await expect(f.run.failure).resolves.toMatchObject({ code: "output-limit", stage: "prompt", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, outputLimitExceeded: true, cancelled: false });
    expect(f.agents).toHaveLength(1);
    expect(f.prompts()).toHaveLength(1);
    expect(f.agents[0]!.kill).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("settles a failed permission reply without an unhandled rejection or a second write", async () => {
    const f = fixture((message, agent) => {
      if (message.method === "initialize") agent.answer(message, INIT);
      else if (message.method === "session/new") agent.answer(message, SESSION);
      else if (message.method === "session/prompt") agent.say({
        id: 90, method: "session/request_permission", params: {
          sessionId: SESSION.sessionId,
          toolCall: { toolCallId: "call-once", kind: "execute", rawInput: { command: "run-once" } },
          options: [{ optionId: "once", kind: "allow_once", name: "Allow" }],
        },
      });
      else if (message.id === 90) throw new Error("Cannot write to ACP response pipe");
    }, { onPermission: async () => "allow_once" });
    await flush();
    await expect(f.run.failure).resolves.toMatchObject({ code: "transport-closed", stage: "prompt", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, terminationUnconfirmed: false });
    expect(f.agents[0]!.sent.filter((message) => message.id === 90)).toHaveLength(1);
    expect(f.agents[0]!.kill).toHaveBeenCalledTimes(1);
    expect(f.prompts()).toHaveLength(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("refuses an unadvertised model instead of silently changing the requested route", async () => {
    const f = fixture(ordinary(() => undefined), { requiredConfig: { model: "provider/selected" } });
    await flush();
    await expect(f.run.failure).resolves.toMatchObject({ code: "agent-error", stage: "startup", message: expect.stringContaining("did not offer the required model") });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null });
    expect(f.prompts()).toHaveLength(0);
    expect(f.agents).toHaveLength(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("bounds staged pre-prompt permission receipts even when the inner queue keeps draining", async () => {
    const f = fixture((message, agent) => {
      if (message.method === "initialize") agent.answer(message, INIT);
      // Hold session/new: any pre-prompt permission replies cannot be this
      // turn's ledger, but must not become unbounded staging memory either.
    }, { maxQueuedRecords: 2 });
    await flush();
    for (let i = 0; i < 3; i += 1) {
      f.agents[0]!.say({ id: 90 + i, method: "session/request_permission", params: {
        toolCall: { toolCallId: `before-prompt-${i}` },
        options: [{ kind: "reject_once", optionId: "no", name: "Deny" }],
      } });
      await flush();
    }
    await vi.advanceTimersByTimeAsync(20);
    await expect(f.run.failure).resolves.toMatchObject({ code: "output-limit", stage: "startup", attempts: 1 });
    await expect(f.run.completion).resolves.toMatchObject({ exitCode: null, cancelled: false, outputLimitExceeded: true, recordCount: 0 });
    expect(f.prompts()).toHaveLength(0);
    expect(f.agents).toHaveLength(1);
    expect(await recordsOf(f.run)).toEqual([]);
    expect(f.agents[0]!.kill).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });
});
