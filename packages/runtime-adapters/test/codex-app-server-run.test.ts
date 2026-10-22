import { describe, expect, it } from "vitest";

import {
  asProcessNormalizer,
  startCodexAppServerRun,
} from "../src/codex-app-server-run.js";
import type { AppServerRunProcess } from "../src/codex-app-server-run.js";
import {
  assertSafeRuntimeCommand,
  codexAppServerPolicy,
  codexPlanToolArguments,
  createCodexAppServerCommand,
} from "../src/commands.js";
import { createAppServerEventNormalizer } from "../src/app-server-events.js";
import type { RuntimeJsonlRecord } from "../src/process-runner.js";

const EXECUTABLE = {
  executablePath: "/usr/bin/codex",
  prefixArgs: [] as readonly string[],
};

/** A fake `codex app-server`: it records what it was told and answers on cue. */
function fakeServer() {
  const written: Array<Record<string, unknown>> = [];
  let emit: (chunk: string) => void = () => undefined;
  let exited: () => void = () => undefined;
  let killed = false;
  const process: AppServerRunProcess = {
    write: (line) => {
      for (const part of line.split("\n")) {
        if (part.trim().length === 0) continue;
        written.push(JSON.parse(part) as Record<string, unknown>);
      }
    },
    kill: () => {
      killed = true;
    },
    onData: (listener) => {
      emit = listener;
    },
    onExit: (listener) => {
      exited = listener;
    },
  };
  return {
    process,
    written,
    get killed() {
      return killed;
    },
    /** Answer the request with this id, the way the real server does. */
    answer: (id: number, result: unknown) =>
      emit(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`),
    notify: (method: string, params: unknown) =>
      emit(`${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`),
    exit: () => exited(),
    /** The id of the first request with this method, or undefined. */
    idOf: (method: string) => {
      const found = written.find((message) => message.method === method);
      return typeof found?.id === "number" ? found.id : undefined;
    },
    paramsOf: (method: string) =>
      written.find((message) => message.method === method)?.params as
        | Record<string, unknown>
        | undefined,
  };
}

const settle = async (): Promise<void> => {
  for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
};

async function handshaken(overrides: Record<string, unknown> = {}) {
  const server = fakeServer();
  const run = startCodexAppServerRun({
    spawn: () => server.process,
    command: createCodexAppServerCommand(EXECUTABLE, {
      workspacePath: "/work",
      sandbox: "workspace-write",
    }),
    prompt: "count the files",
    sandbox: "workspace-write",
    approvalPolicy: "never",
    ...overrides,
  });
  await settle();
  server.answer(server.idOf("initialize")!, { userAgent: "codex" });
  await settle();
  server.answer(server.idOf("thread/start") ?? server.idOf("thread/resume") ?? server.idOf("thread/fork")!, {
    thread: { id: "thread_9" },
  });
  await settle();
  server.answer(server.idOf("turn/start")!, { turn: { id: "turn_1" } });
  await settle();
  return { server, run };
}

describe("the policy a Codex thread is started with", () => {
  it("asks for danger-full-access only when the mission really has full access", () => {
    expect(codexAppServerPolicy("full-access")).toEqual({
      sandbox: "danger-full-access",
      approvalPolicy: "never",
    });
    expect(codexAppServerPolicy("workspace-write").sandbox).toBe("workspace-write");
    expect(codexAppServerPolicy("read-only").sandbox).toBe("read-only");
    // The dangerous value can only come from the one sandbox that means it.
    // A mission with no sandbox at all reads as read-only, never wider.
    expect(codexAppServerPolicy(undefined).sandbox).toBe("read-only");
  });

  it("never asks anything, because the exec transport it replaces never could", () => {
    for (const sandbox of ["read-only", "workspace-write", "full-access"] as const) {
      expect(codexAppServerPolicy(sandbox).approvalPolicy).toBe("never");
    }
  });
});

describe("the command that launches it", () => {
  it("is `codex app-server`, with the prompt in neither argv nor stdin", () => {
    const command = createCodexAppServerCommand(EXECUTABLE, {
      workspacePath: "/work",
      sandbox: "workspace-write",
    });
    expect(command.args).toEqual(["app-server"]);
    expect(command.stdin).toBe("protocol");
    expect(command.sandbox).toBe("workspace-write");
    expect(command.cwd).toBe("/work");
  });

  // Codex 0.153 offers update_plan only when its config says so; without it
  // a model briefed to keep a todo list typed one into its reply (2026-09-23).
  it("switches the plan tool on from 0.153.0, where it was measured, and not below", () => {
    const ON = ["-c", "tools.update_plan.enabled=true"];
    expect(codexPlanToolArguments("0.153.0")).toEqual(ON);
    expect(codexPlanToolArguments("0.156.1")).toEqual(ON);
    expect(codexPlanToolArguments("1.0.0")).toEqual(ON);
    expect(codexPlanToolArguments("0.152.9")).toEqual([]);
    expect(codexPlanToolArguments("0.151.0-alpha.7.2")).toEqual([]);
    expect(codexPlanToolArguments("not a version")).toEqual([]);
    expect(codexPlanToolArguments(undefined)).toEqual([]);
    for (const sandbox of ["read-only", "workspace-write", "full-access"] as const) {
      const command = createCodexAppServerCommand(EXECUTABLE, { workspacePath: "/work", sandbox, cliVersion: "0.156.1" });
      expect(command.args).toEqual(["app-server", ...ON]);
      expect(() => assertSafeRuntimeCommand(command, sandbox)).not.toThrow();
    }
  });
});

describe("a turn over app-server", () => {
  it("hands the folder, the policy, the model and the prompt to the right calls", async () => {
    const { server } = await handshaken({ model: "gpt-5.6-luna", effort: "low" });
    expect(server.paramsOf("thread/start")).toMatchObject({
      cwd: "/work",
      sandbox: "workspace-write",
      approvalPolicy: "never",
    });
    expect(server.paramsOf("turn/start")).toMatchObject({
      threadId: "thread_9",
      model: "gpt-5.6-luna",
      effort: "low",
      input: [{ type: "text", text: "count the files" }],
    });
  });

  it("resumes a prior thread rather than starting one, which is what keeps a follow-up warm", async () => {
    const { server } = await handshaken({ resumeThreadId: "thread_earlier" });
    expect(server.idOf("thread/start")).toBeUndefined();
    expect(server.paramsOf("thread/resume")).toMatchObject({
      threadId: "thread_earlier",
      cwd: "/work",
      // Without it the answer carries the whole history as one line: a 20 MB thread passed the 8 MB limit (10/02).
      excludeTurns: true,
    });
  });

  it("forks the thread for a question on the side, a copy the side can fork again (0.461)", async () => {
    const { server } = await handshaken({ resumeThreadId: "thread_earlier", forkThread: true, sandbox: "read-only" });
    expect(server.idOf("thread/resume")).toBeUndefined();
    expect(server.idOf("thread/start")).toBeUndefined();
    expect(server.paramsOf("thread/fork")).toMatchObject({ threadId: "thread_earlier", sandbox: "read-only", cwd: "/work", excludeTurns: true });
    expect(server.paramsOf("thread/fork")).not.toHaveProperty("ephemeral");
  });

  it("lays the thread's own config over the person's, on a start, a resume and a fork alike (0.720)", async () => {
    const threadConfig = { bypass_hook_trust: true, hooks: { PreToolUse: [{ matcher: "Bash", hooks: [{ type: "command", command: "guard" }] }] } };
    expect((await handshaken({ threadConfig })).server.paramsOf("thread/start")).toMatchObject({ config: threadConfig });
    expect((await handshaken({ threadConfig, resumeThreadId: "thread_earlier" })).server.paramsOf("thread/resume")).toMatchObject({ config: threadConfig });
    expect((await handshaken({ threadConfig, resumeThreadId: "thread_earlier", forkThread: true })).server.paramsOf("thread/fork")).toMatchObject({ config: threadConfig });
    // A typed command's effort travels in the same config, and neither drops the other.
    const typed = (await handshaken({ threadConfig, slashCommand: "compact", model: "gpt-6-luna", effort: "low" })).server.paramsOf("thread/start");
    expect(typed).toMatchObject({ model: "gpt-6-luna", config: { model_reasoning_effort: "low", ...threadConfig } });
    // Without one, no config is sent at all, as before.
    expect((await handshaken()).server.paramsOf("thread/start")).not.toHaveProperty("config");
  });

  // 0.824: Ask and Plan start none of the MCP servers the person's Codex runs as programs, named from its settings.
  function readingSettings(made: (settings: unknown) => { ok: true; config: Record<string, boolean> } | { ok: false; message: string }) {
    const server = fakeServer();
    const seen: unknown[] = [];
    const run = startCodexAppServerRun({
      spawn: () => server.process,
      command: createCodexAppServerCommand(EXECUTABLE, { workspacePath: "/work", sandbox: "read-only" }),
      prompt: "count the files",
      sandbox: "read-only",
      approvalPolicy: "never",
      threadConfig: { "mcp_servers.locust_browser.enabled": true },
      threadConfigFromSettings: (settings) => {
        seen.push(settings);
        return made(settings);
      },
    });
    return { server, run, seen };
  }

  it("reads the person's settings before the thread starts, and lays what it makes under Locust's own config (0.824)", async () => {
    const { server, seen } = readingSettings(() => ({ ok: true, config: { "features.plugins": false, "mcp_servers.notes.enabled": false } }));
    await settle();
    server.answer(server.idOf("initialize")!, { userAgent: "codex" });
    await settle();
    // Nothing starts before the settings are read: config/read is asked first, in the run's folder.
    expect(server.idOf("thread/start")).toBeUndefined();
    expect(server.paramsOf("config/read")).toEqual({ cwd: "/work" });
    server.answer(server.idOf("config/read")!, { config: { mcp_servers: { notes: { command: "notes-server" } } } });
    await settle();
    expect(seen).toEqual([{ mcp_servers: { notes: { command: "notes-server" } } }]);
    expect(server.paramsOf("thread/start")).toMatchObject({
      config: { "features.plugins": false, "mcp_servers.notes.enabled": false, "mcp_servers.locust_browser.enabled": true },
    });
  });

  it("starts no thread when the settings refuse it, and ends with the refusal's own words (0.824)", async () => {
    const message = "Locust could not read your Codex settings, so it did not start this turn.";
    const { server, run, seen } = readingSettings(() => ({ ok: false, message }));
    (server.process as { stderrTail?: () => string }).stderrTail = () => "some unrelated server line\n";
    await settle();
    server.answer(server.idOf("initialize")!, { userAgent: "codex" });
    await settle();
    // An answer that holds no settings reads as none at all.
    server.answer(server.idOf("config/read")!, { not: "settings" });
    const completion = await run.completion;
    expect(seen).toEqual([undefined]);
    expect(server.idOf("thread/start")).toBeUndefined();
    expect(completion.exitCode).toBeNull();
    // Locust's own reason, not followed by a server line it has nothing to do with.
    expect(completion.stderr).toBe(message);
  });

  it("streams every notification as a record and ends when the turn does", async () => {
    const { server, run } = await handshaken();
    server.notify("item/agentMessage/delta", { itemId: "m1", delta: "Cach" });
    server.notify("item/agentMessage/delta", { itemId: "m1", delta: "ing" });
    server.notify("turn/completed", { threadId: "thread_9" });

    const seen: RuntimeJsonlRecord[] = [];
    for await (const record of run.records) seen.push(record);
    expect(seen).toHaveLength(3);
    expect(JSON.parse(seen[0]!.raw)).toMatchObject({
      method: "item/agentMessage/delta",
      params: { delta: "Cach" },
    });

    const completion = await run.completion;
    expect(completion.exitCode).toBe(0);
    expect(completion.cancelled).toBe(false);
    expect(completion.recordCount).toBe(3);
    // The server is not left running once its turn is over.
    expect(server.killed).toBe(true);
  });

  it("reports a server that dies mid-turn as a lost transport, not a clean finish", async () => {
    const { server, run } = await handshaken();
    server.notify("item/agentMessage/delta", { itemId: "m1", delta: "half a th" });
    server.exit();
    const completion = await run.completion;
    expect(completion.exitCode).toBeNull();
    expect(completion.cancelled).toBe(false);
    expect(completion.stderr).toContain("exited");
  });

  // QA-2026-09-29 round 2, N10: the server's own reason was thrown away, and
  // the card quoted the app's sentence back as "the runtime's last word".
  it("keeps what a dying server wrote to stderr, as its last line", async () => {
    const { server, run } = await handshaken();
    (server.process as { stderrTail?: () => string }).stderrTail = () => "thread 'main' panicked\nError: sandbox could not start: seatbelt denied\n";
    server.exit();
    const completion = await run.completion;
    const lines = completion.stderr.trim().split("\n");
    expect(lines[0]).toContain("exited");
    expect(lines.at(-1)).toBe("Error: sandbox could not start: seatbelt denied");
  });

  it("ends the run when the mission is cancelled", async () => {
    const controller = new AbortController();
    const { run } = await handshaken({ signal: controller.signal });
    controller.abort();
    const completion = await run.completion;
    expect(completion.cancelled).toBe(true);
    expect(completion.exitCode).toBeNull();
  });

  it("gives up rather than hanging when the server never answers the handshake", async () => {
    const server = fakeServer();
    const run = startCodexAppServerRun({
      spawn: () => server.process,
      command: createCodexAppServerCommand(EXECUTABLE, {
        workspacePath: "/work",
        sandbox: "read-only",
      }),
      prompt: "hello",
      sandbox: "read-only",
      approvalPolicy: "never",
      handshakeTimeoutMs: 1,
    });
    const completion = await run.completion;
    expect(completion.exitCode).toBeNull();
    expect(completion.stderr).toContain("did not answer");
  });
});

describe("the app-server normalizer, worn as a process normalizer", () => {
  const normalizerFor = () =>
    asProcessNormalizer(
      createAppServerEventNormalizer({ runId: "run_1", missionId: "mission_1", runtime: "codex" }),
    );

  it("turns a record back into the notification it was written from", () => {
    const normalizer = normalizerFor();
    const events = normalizer.accept({
      sequence: 1,
      raw: JSON.stringify({
        method: "item/agentMessage/delta",
        params: { itemId: "m1", delta: "Caching" },
      }),
    });
    expect(events.some((event) => event.type === "message.delta")).toBe(true);
  });

  it("drops a line that is not a notification instead of guessing at it", () => {
    const normalizer = normalizerFor();
    expect(normalizer.accept({ sequence: 1, raw: "not json" })).toEqual([]);
    expect(normalizer.accept({ sequence: 2, raw: JSON.stringify({ id: 4 }) })).toEqual([]);
  });

  it("reads a cancelled completion as cancelled and anything else as a lost transport", () => {
    const receipt = {
      exitCode: null,
      signal: null,
      stderr: "",
      stderrTruncated: false,
      recordCount: 0,
      forcedTerminationAttempted: false,
      terminationUnconfirmed: false,
      inputDeliveryFailed: false,
      outputLimitExceeded: false,
      oversizedRecordsDropped: 0,
      startedAt: "2026-09-10T00:00:00.000Z",
      finishedAt: "2026-09-10T00:00:01.000Z",
    };
    expect(
      normalizerFor().finish({ ...receipt, cancelled: true })[0]?.type,
    ).toBe("run.cancelled");
    expect(
      normalizerFor().finish({ ...receipt, cancelled: false })[0]?.type,
    ).toBe("run.failed");
  });
});

describe("what a run over this transport cost", () => {
  /*
   * The shapes below are codex 0.156.1's own, captured 2026-09-25 by
   * probe-codex-resume-and-subagent: a first turn's `total` equals its `last`;
   * a RESUMED thread replays the earlier total before its turn starts; a
   * sub-agent's turn runs on the same connection under its own thread id.
   */
  const line = (method: string, params: unknown): RuntimeJsonlRecord => ({
    sequence: 1,
    raw: JSON.stringify({ method, params }),
  });
  const counted = (threadId: string, total: number, last: number, cached = 0) =>
    line("thread/tokenUsage/updated", {
      threadId,
      tokenUsage: {
        total: { totalTokens: total, inputTokens: total - 5, cachedInputTokens: cached, cacheWriteInputTokens: 0, outputTokens: 5, reasoningOutputTokens: 0 },
        last: { totalTokens: last, inputTokens: last - 5, cachedInputTokens: cached, cacheWriteInputTokens: 0, outputTokens: 5, reasoningOutputTokens: 0 },
      },
    });
  const fresh = () =>
    asProcessNormalizer(createAppServerEventNormalizer({ runId: "run_1", missionId: "mission_1", runtime: "codex" }));
  const usageOf = (events: readonly { type: string; payload: unknown }[]) =>
    (events.find((event) => event.type === "run.completed")?.payload as { usage?: Record<string, number> } | undefined)?.usage;

  it("reports the tokens the thread counted, in the vocabulary every receipt uses", () => {
    const normalizer = fresh();
    normalizer.accept(line("thread/started", { thread: { id: "t1" } }));
    normalizer.accept(line("turn/started", { threadId: "t1", turn: { id: "turn_1" } }));
    normalizer.accept(counted("t1", 18_375, 18_375, 11_008));
    const events = normalizer.accept(line("turn/completed", { threadId: "t1", turn: { id: "turn_1" } }));
    expect(usageOf(events)).toMatchObject({ inputTokens: 18_370, outputTokens: 5, cacheReadTokens: 11_008 });
  });

  it("bills a resumed turn for itself, not the whole conversation", () => {
    // Measured: the replay said 18,375 before the turn; the turn's own read
    // 36,767 total, 18,392 last. The receipt used to say 36,767.
    const normalizer = fresh();
    normalizer.accept(counted("t1", 18_375, 18_375));
    normalizer.accept(line("turn/started", { threadId: "t1", turn: { id: "turn_2" } }));
    normalizer.accept(counted("t1", 36_767, 18_392));
    const events = normalizer.accept(line("turn/completed", { threadId: "t1", turn: { id: "turn_2" } }));
    expect(usageOf(events)).toMatchObject({ inputTokens: 18_392, outputTokens: 0 });
  });

  it("is not ended by a sub-agent's turn, and counts what the sub-agent spent", () => {
    const normalizer = fresh();
    normalizer.accept(line("thread/started", { thread: { id: "parent" } }));
    normalizer.accept(line("turn/started", { threadId: "parent", turn: { id: "p1" } }));
    normalizer.accept(counted("parent", 18_548, 18_548));
    normalizer.accept(line("item/started", { threadId: "parent", item: { id: "c1", type: "collabAgentToolCall", tool: "spawnAgent", status: "inProgress" } }));
    // The sub-agent, on the same connection, ending FIRST -- as measured.
    const sub = [
      ...normalizer.accept(line("turn/started", { threadId: "child", turn: { id: "k1" } })),
      ...normalizer.accept(line("item/agentMessage/delta", { threadId: "child", itemId: "km", delta: "PING" })),
      ...normalizer.accept(counted("child", 18_379, 18_379)),
      ...normalizer.accept(line("turn/completed", { threadId: "child", turn: { id: "k1" } })),
    ];
    expect(sub).toEqual([]);
    expect(normalizer.finalized).toBe(false);
    expect(normalizer.runtimeThreadId).toBe("parent");
    const closed = normalizer.accept(line("item/completed", { threadId: "parent", item: { id: "c1", type: "collabAgentToolCall", tool: "spawnAgent", status: "completed" } }));
    expect(closed[0]).toMatchObject({ type: "tool.completed", payload: { name: "subagent:spawnAgent" } });
    normalizer.accept(counted("parent", 46_273, 27_725));
    normalizer.accept(line("item/agentMessage/delta", { threadId: "parent", itemId: "pm", delta: "It said PING." }));
    const events = normalizer.accept(line("turn/completed", { threadId: "parent", turn: { id: "p1" } }));
    expect(events[0]?.type).toBe("run.completed");
    // The parent's 46,273 and the sub-agent's 18,379 -- the run caused both.
    expect(usageOf(events)?.inputTokens).toBe(46_268 + 18_374);
  });

  it("keeps the transport running through a sub-agent's turn ending", async () => {
    const { server, run } = await handshaken();
    server.notify("turn/started", { threadId: "thread_9", turn: { id: "turn_1" } });
    server.notify("turn/started", { threadId: "sub_1", turn: { id: "sub_turn" } });
    server.notify("turn/completed", { threadId: "sub_1", turn: { id: "sub_turn" } });
    await settle();
    expect(server.killed).toBe(false);
    // The steer still aims at the parent's turn.
    void run.steer("heads up");
    await settle();
    expect(server.paramsOf("turn/steer")).toMatchObject({ threadId: "thread_9", expectedTurnId: "turn_1" });
    server.notify("turn/completed", { threadId: "thread_9", turn: { id: "turn_1" } });
    await settle();
    expect(server.killed).toBe(true);
  });

  it("says nothing about cost when the thread never counted, rather than claiming zero", () => {
    const normalizer = asProcessNormalizer(
      createAppServerEventNormalizer({ runId: "run_2", missionId: "mission_2", runtime: "codex" }),
    );
    const [completed] = normalizer.accept({
      sequence: 1,
      raw: JSON.stringify({ method: "turn/completed", params: { threadId: "t1" } }),
    });
    expect((completed?.payload as { usage?: unknown }).usage).toBeUndefined();
  });
});

/*
 * A B4 lead from the code review, settled: a full notification queue dropped
 * records and said nothing, and the run could still end "completed" with an
 * unknown hole in its record. The exec transport treats a backed-up queue as
 * fatal for exactly that reason; so does this.
 */
describe("a notification queue that backs up", () => {
  it("stops the run and says why, instead of dropping records quietly", async () => {
    const { server, run } = await handshaken({ maxQueuedRecords: 3 });
    for (let n = 0; n < 5; n += 1) server.notify("item/agentMessage/delta", { threadId: "thread_9", turnId: "turn_1", itemId: "m", delta: "x" });
    const completion = await run.completion;
    expect(completion.outputLimitExceeded).toBe(true);
    expect(completion.stderr).toMatch(/faster than Locust could record it/i);
    expect(server.killed).toBe(true);
  });
});

describe("one of Codex's own commands, typed by the person (0.428)", () => {
  async function started(slashCommand: "review" | "compact", prompt: string) {
    const server = fakeServer();
    const run = startCodexAppServerRun({
      spawn: () => server.process,
      command: createCodexAppServerCommand(EXECUTABLE, { workspacePath: "/work", sandbox: "read-only" }),
      prompt,
      sandbox: "read-only",
      approvalPolicy: "never",
      model: "gpt-6-luna",
      effort: "low",
      resumeThreadId: "thread_9",
      slashCommand,
    });
    await settle();
    server.answer(server.idOf("initialize")!, { userAgent: "codex" });
    await settle();
    server.answer(server.idOf("thread/resume")!, { thread: { id: "thread_9" } });
    await settle();
    return { server, run };
  }

  it("reviews the uncommitted changes as its terminal does, on the thread's own route, and starts no ordinary turn", async () => {
    const { server } = await started("review", "  ");
    expect(server.paramsOf("review/start")).toEqual({ threadId: "thread_9", target: { type: "uncommittedChanges" }, delivery: "inline" });
    expect(server.paramsOf("thread/resume")).toMatchObject({ threadId: "thread_9", model: "gpt-6-luna", config: { model_reasoning_effort: "low" } });
    expect(server.idOf("turn/start")).toBeUndefined();
  });

  it("reviews what the person asked for, when they said", async () => {
    const { server } = await started("review", "only the error handling");
    expect(server.paramsOf("review/start")).toMatchObject({ target: { type: "custom", instructions: "only the error handling" } });
  });

  it("compacts the thread, and the run ends when that turn does", async () => {
    const { server, run } = await started("compact", "");
    expect(server.paramsOf("thread/compact/start")).toEqual({ threadId: "thread_9" });
    expect(server.idOf("turn/start")).toBeUndefined();
    server.answer(server.idOf("thread/compact/start")!, {});
    server.notify("turn/started", { threadId: "thread_9", turn: { id: "turn_c" } });
    server.notify("turn/completed", { threadId: "thread_9", turn: { id: "turn_c", status: "completed" } });
    const completion = await run.completion;
    expect(completion.exitCode).toBe(0);
  });
});
