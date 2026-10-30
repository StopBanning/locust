import { EventEmitter } from "node:events";
import { resolve } from "node:path";
import { describe, expect, it, vi } from "vitest";
import {
  createNodeRuntimeProcessRunner,
} from "../src/index.js";
import type {
  RuntimeJsonlRecord,
  RuntimeSpawn,
  RuntimeSpawnOptions,
  SpawnedRuntimeProcess,
} from "../src/index.js";

const prompt = "private mission prompt";
const spec = {
  runtime: "codex",
  executablePath: resolve("fake-runtime"),
  args: ["exec", "--json", "--sandbox", "read-only", "-"],
  cwd: resolve("fake-workspace"),
  stdin: "prompt",
  stdout: "jsonl",
} as const;

interface FakeChild {
  readonly process: SpawnedRuntimeProcess;
  readonly stdout: EventEmitter;
  readonly stderr: EventEmitter;
  readonly stdin: EventEmitter & {
    readonly writes: string[];
    ended: boolean;
  };
  readonly signals: NodeJS.Signals[];
  close(exitCode: number | null, signal?: NodeJS.Signals | null): void;
  fail(error: Error): void;
}

function fakeChild(options: {
  readonly writeError?: Error;
  readonly killResult?: boolean;
  readonly onKill?: (signal: NodeJS.Signals, child: FakeChild) => void;
} = {}): FakeChild {
  const stdout = new EventEmitter();
  const stderr = new EventEmitter();
  const stdin = new EventEmitter() as FakeChild["stdin"];
  Object.assign(stdin, { writes: [] as string[], ended: false });
  const process = new EventEmitter() as EventEmitter & SpawnedRuntimeProcess;
  const signals: NodeJS.Signals[] = [];
  const child = {
    process,
    stdout,
    stderr,
    stdin,
    signals,
    close(exitCode: number | null, signal: NodeJS.Signals | null = null) {
      process.emit("close", exitCode, signal);
    },
    fail(error: Error) {
      process.emit("error", error);
    },
  } satisfies FakeChild;

  Object.assign(stdin, {
    write(value: string, callback?: (error?: Error | null) => void) {
      stdin.writes.push(value);
      callback?.(options.writeError);
      return true;
    },
    end() {
      stdin.ended = true;
    },
  });
  Object.assign(process, {
    stdin,
    stdout,
    stderr,
    kill(signal: NodeJS.Signals = "SIGTERM") {
      signals.push(signal);
      options.onKill?.(signal, child);
      return options.killResult ?? true;
    },
  });
  return child;
}

async function collect(records: AsyncIterable<RuntimeJsonlRecord>) {
  const collected: RuntimeJsonlRecord[] = [];
  for await (const record of records) collected.push(record);
  return collected;
}

describe("controlled runtime JSONL process runner", () => {
  it("uses the validated argv and cwd, sends the prompt only to stdin, and streams raw records", async () => {
    const child = fakeChild();
    let launched:
      | {
        readonly executablePath: string;
        readonly args: readonly string[];
        readonly options: RuntimeSpawnOptions;
      }
      | undefined;
    const spawnProcess: RuntimeSpawn = (executablePath, args, options) => {
      launched = { executablePath, args, options };
      return child.process;
    };
    const runner = createNodeRuntimeProcessRunner({
      environment: {
        PATH: "C:\\tools",
        USERPROFILE: "C:\\Users\\tester",
        CODEX_HOME: "C:\\runtime-state\\codex",
        ANTHROPIC_API_KEY: "must-not-leak",
        CUSTOM_SECRET: "must-not-leak",
      },
      spawnProcess,
      now: () => new Date("2026-08-31T12:00:00.000Z"),
    });

    const run = runner.start(spec, prompt);
    const emoji = Buffer.from("🙂");
    child.stdout.emit("data", Buffer.concat([Buffer.from('{"one":"'), emoji.subarray(0, 2)]));
    child.stdout.emit("data", Buffer.concat([emoji.subarray(2), Buffer.from('"}\r\n\n{"two":2}') ]));
    child.close(0);

    await expect(collect(run.records)).resolves.toEqual([
      { sequence: 1, raw: '{"one":"🙂"}' },
      { sequence: 2, raw: '{"two":2}' },
    ]);
    await expect(run.completion).resolves.toMatchObject({
      exitCode: 0,
      signal: null,
      recordCount: 2,
      cancelled: false,
      forcedTerminationAttempted: false,
      terminationUnconfirmed: false,
      inputDeliveryFailed: false,
      outputLimitExceeded: false,
      startedAt: "2026-08-31T12:00:00.000Z",
      finishedAt: "2026-08-31T12:00:00.000Z",
    });
    expect(child.stdin.writes).toEqual([prompt]);
    expect(child.stdin.ended).toBe(true);
    expect(launched).toMatchObject({
      executablePath: spec.executablePath,
      args: spec.args,
      options: {
        cwd: spec.cwd,
        shell: false,
        windowsHide: true,
        stdio: ["pipe", "pipe", "pipe"],
        env: {
          PATH: "C:\\tools",
          USERPROFILE: "C:\\Users\\tester",
          CODEX_HOME: "C:\\runtime-state\\codex",
        },
      },
    });
    expect(launched?.args).not.toContain(prompt);
    expect(launched?.options.env).not.toHaveProperty("ANTHROPIC_API_KEY");
    expect(launched?.options.env).not.toHaveProperty("CUSTOM_SECRET");
  });

  it("bounds captured stderr and reports truncation", async () => {
    const child = fakeChild();
    const runner = createNodeRuntimeProcessRunner({
      maxStderrBytes: 5,
      spawnProcess: () => child.process,
    });
    const run = runner.start(spec, prompt);
    child.stderr.emit("data", "abcdefgh");
    child.close(2);

    await expect(run.completion).resolves.toMatchObject({
      exitCode: 2,
      stderr: "abcde",
      stderrTruncated: true,
    });
  });

  it("redacts the prompt from captured stderr", async () => {
    const child = fakeChild();
    const runner = createNodeRuntimeProcessRunner({ spawnProcess: () => child.process });
    const run = runner.start(spec, prompt);
    child.stderr.emit("data", `provider echoed: ${prompt}`);
    child.close(1);

    const completion = await run.completion;
    expect(completion.stderr).toBe("provider echoed: [prompt redacted]");
    expect(completion.stderr).not.toContain(prompt);
  });

  it("cancels gracefully with SIGINT when the child acknowledges it", async () => {
    const controller = new AbortController();
    const child = fakeChild({
      onKill: (signal, target) => {
        if (signal === "SIGINT") queueMicrotask(() => target.close(null, "SIGINT"));
      },
    });
    const runner = createNodeRuntimeProcessRunner({
      cancellationGraceMs: 5,
      spawnProcess: () => child.process,
    });
    const run = runner.start(spec, prompt, { signal: controller.signal });
    controller.abort();

    await expect(run.completion).resolves.toMatchObject({
      signal: "SIGINT",
      cancelled: true,
      forcedTerminationAttempted: false,
    });
    expect(child.signals).toEqual(["SIGINT"]);
  });

  it("forces termination after the cancellation grace period and never retries", async () => {
    const controller = new AbortController();
    const child = fakeChild();
    let spawnCount = 0;
    const runner = createNodeRuntimeProcessRunner({
      cancellationGraceMs: 5,
      terminationConfirmationMs: 5,
      spawnProcess: () => {
        spawnCount += 1;
        return child.process;
      },
    });
    const run = runner.start(spec, prompt, { signal: controller.signal });
    controller.abort();

    await expect(run.completion).resolves.toMatchObject({
      exitCode: null,
      signal: null,
      cancelled: true,
      forcedTerminationAttempted: true,
      terminationUnconfirmed: true,
    });
    expect(child.signals).toEqual(["SIGINT", "SIGKILL"]);
    expect(spawnCount).toBe(1);
  });

  it("waits for close after a forced signal even when kill returns false", async () => {
    const controller = new AbortController();
    const child = fakeChild({ killResult: false });
    const runner = createNodeRuntimeProcessRunner({
      cancellationGraceMs: 5,
      terminationConfirmationMs: 100,
      spawnProcess: () => child.process,
    });
    const run = runner.start(spec, prompt, { signal: controller.signal });
    let completed = false;
    void run.completion.then(() => {
      completed = true;
    });
    controller.abort();
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 15));

    expect(child.signals).toEqual(["SIGINT", "SIGKILL"]);
    expect(completed).toBe(false);
    child.close(null, "SIGKILL");
    await expect(run.completion).resolves.toMatchObject({
      signal: "SIGKILL",
      forcedTerminationAttempted: true,
      terminationUnconfirmed: false,
    });
  });

  it("skips an oversized JSONL record and lets the run carry on", async () => {
    // MEASURED 2026-09-07, Cursor Agent on grok-4.6, mission ef164de4: 1,596
    // records had arrived and the answer was streaming normally when record
    // 1,597 came in over the cap. The run was SIGINT-ed and everything already
    // on screen was thrown away, on a mission whose whole output was 871
    // tokens. Losing that record and losing the run are different sizes of
    // loss; only one of them is a reason to stop.
    const child = fakeChild();
    const runner = createNodeRuntimeProcessRunner({
      maxRecordBytes: 8,
      spawnProcess: () => child.process,
    });
    const run = runner.start(spec, prompt);
    const iterator = run.records[Symbol.asyncIterator]();
    child.stdout.emit("data", `${prompt}\n{"ok":1}\n`);
    child.close(0, null);

    // The record after the oversized one still arrives: that is the point.
    await expect(iterator.next()).resolves.toEqual({
      done: false,
      value: { sequence: 1, raw: '{"ok":1}' },
    });
    await expect(run.completion).resolves.toMatchObject({
      outputLimitExceeded: false,
      oversizedRecordsDropped: 1,
      recordCount: 1,
      cancelled: false,
    });
    // And it is still never killed for it.
    expect(child.signals).toEqual([]);
  });

  it("never exposes the content of a record it skipped", async () => {
    // The reason the cap exists at all. Skipping must not become a way for an
    // oversized payload to reach the ledger by another door.
    const child = fakeChild();
    const runner = createNodeRuntimeProcessRunner({
      maxRecordBytes: 8,
      spawnProcess: () => child.process,
    });
    const run = runner.start(spec, prompt);
    const drained: string[] = [];
    child.stdout.emit("data", `${prompt}\n`);
    child.close(0, null);
    const completion = await run.completion;
    for (const record of run.records.drainAvailable()) drained.push(record.raw);
    expect(drained.join(" ")).not.toContain(prompt);
    expect(JSON.stringify(completion)).not.toContain(prompt);
  });

  it("resynchronises on the next newline after a line it gave up on", async () => {
    // A partial line already past the cap is thrown away, and the REST of that
    // line keeps arriving. Without a resync the tail would be read as a record
    // of its own -- half a line of JSON, reported as the runtime sending
    // nonsense, which it did not.
    const child = fakeChild();
    const runner = createNodeRuntimeProcessRunner({
      maxRecordBytes: 24,
      spawnProcess: () => child.process,
    });
    const run = runner.start(spec, prompt);
    const iterator = run.records[Symbol.asyncIterator]();
    // Arrives in pieces, with no newline until well past the cap.
    child.stdout.emit("data", '{"big":"aaaaaaaaaaaaaaa');
    child.stdout.emit("data", 'aaaaaaaaaaaaaaaaaaaaaaa');
    child.stdout.emit("data", 'tail-of-the-same-line"}\n{"next":2}\n');
    child.close(0, null);

    await expect(iterator.next()).resolves.toEqual({
      done: false,
      value: { sequence: 1, raw: '{"next":2}' },
    });
    await expect(run.completion).resolves.toMatchObject({
      oversizedRecordsDropped: 1,
      recordCount: 1,
    });
  });

  it("carries a long, productive burst without stopping the run", async () => {
    /*
     * MEASURED 2026-09-08: a Cursor teammate on grok-4.6 writing a long report
     * died with "Cursor Agent sent more output than Locust could take in" at
     * 373k in / 15k out, and an earlier run of the same shape carried 1,596
     * records. The queue held 64.
     *
     * The consumer already batches to avoid exactly this, but the pile-up
     * happens during the await that writes a batch -- so the only thing that
     * fixes it is a queue deep enough to hold what a fast runtime emits while
     * one write completes.
     */
    const child = fakeChild();
    const runner = createNodeRuntimeProcessRunner({ spawnProcess: () => child.process });
    const run = runner.start(spec, prompt);
    // Far more than the old ceiling, in one burst, with nobody reading yet.
    const burst = Array.from({ length: 500 }, (_, index) => JSON.stringify({ n: index })).join("\n");
    child.stdout.emit("data", burst + "\n");
    child.close(0, null);

    const collected = await collect(run.records);
    expect(collected).toHaveLength(500);
    await expect(run.completion).resolves.toMatchObject({
      outputLimitExceeded: false,
      recordCount: 500,
      cancelled: false,
    });
    expect(child.signals).toEqual([]);
  });

  it("bounds queued records when a consumer falls behind", async () => {
    const child = fakeChild({
      onKill: (signal, target) => {
        if (signal === "SIGINT") queueMicrotask(() => target.close(null, "SIGINT"));
      },
    });
    const runner = createNodeRuntimeProcessRunner({
      maxQueuedRecords: 1,
      spawnProcess: () => child.process,
    });
    const run = runner.start(spec, prompt);
    child.stdout.emit("data", '{"one":1}\n{"two":2}\n');
    const iterator = run.records[Symbol.asyncIterator]();

    await expect(iterator.next()).resolves.toEqual({
      done: false,
      value: { sequence: 1, raw: '{"one":1}' },
    });
    await expect(iterator.next()).rejects.toThrow(
      "Runtime JSONL output exceeded its safety limit",
    );
    await expect(run.completion).resolves.toMatchObject({
      outputLimitExceeded: true,
      recordCount: 1,
    });
  });

  it("does not launch when the AbortSignal is already aborted", () => {
    const controller = new AbortController();
    controller.abort();
    const spawnProcess = vi.fn<RuntimeSpawn>();
    const runner = createNodeRuntimeProcessRunner({ spawnProcess });

    expect(() => runner.start(spec, prompt, { signal: controller.signal })).toThrow(
      "Runtime process was cancelled before launch",
    );
    expect(spawnProcess).not.toHaveBeenCalled();
  });

  it("sanitizes command-validation errors even when the prompt matches an unsafe argument", () => {
    const unsafePrompt = "--full-auto";
    const spawnProcess = vi.fn<RuntimeSpawn>();
    const runner = createNodeRuntimeProcessRunner({ spawnProcess });

    expect(() => runner.start({ ...spec, args: [unsafePrompt] }, unsafePrompt)).toThrow(
      "Runtime command failed safety validation",
    );
    try {
      runner.start({ ...spec, args: [unsafePrompt] }, unsafePrompt);
    } catch (error) {
      expect(String(error)).not.toContain(unsafePrompt);
    }
    expect(spawnProcess).not.toHaveBeenCalled();
  });

  /*
   * QA-2026-09-29 round 2, R25: a program that is gone rejected `completion`
   * before anyone awaited it; Node called it unhandled and the app answered
   * with a blocking dialog that froze every run. It must reject for whoever
   * awaits it, and never as unhandled -- and say which failure it was.
   */
  it("a failed start is never an unhandled rejection, and names the system's code", async () => {
    const unhandled: unknown[] = [];
    const listen = (reason: unknown) => unhandled.push(reason);
    process.on("unhandledRejection", listen);
    try {
      const child = fakeChild();
      const run = createNodeRuntimeProcessRunner({ spawnProcess: () => child.process }).start(spec, prompt);
      child.fail(Object.assign(new Error("spawn C:\\tools\\claude.exe ENOENT"), { code: "ENOENT" }));
      // Nobody awaits it for a while, as the mission code does not.
      await new Promise((resolve) => setTimeout(resolve, 30));
      expect(unhandled).toEqual([]);
      await expect(run.completion).rejects.toThrow("Runtime process failed to start (ENOENT)");
    } finally {
      process.off("unhandledRejection", listen);
    }
  });

  it("sanitizes asynchronous launch errors and does not retry", async () => {
    const child = fakeChild();
    let spawnCount = 0;
    const runner = createNodeRuntimeProcessRunner({
      spawnProcess: () => {
        spawnCount += 1;
        return child.process;
      },
    });
    const run = runner.start(spec, prompt);
    const firstRecord = run.records[Symbol.asyncIterator]().next();
    child.fail(new Error(`launch failed while handling ${prompt}`));

    await expect(run.completion).rejects.toThrow("Runtime process failed to start");
    await expect(firstRecord).rejects.toThrow("Runtime process failed to start");
    await run.completion.catch((error: unknown) => {
      expect(String(error)).not.toContain(prompt);
    });
    expect(spawnCount).toBe(1);
  });

  it("sanitizes synchronous spawn errors", () => {
    const runner = createNodeRuntimeProcessRunner({
      spawnProcess: () => {
        throw new Error(`cannot launch ${prompt}`);
      },
    });

    expect(() => runner.start(spec, prompt)).toThrow("Runtime process failed to start");
    try {
      runner.start(spec, prompt);
    } catch (error) {
      expect(String(error)).not.toContain(prompt);
    }
  });

  it("marks stdin delivery failure without treating it as user cancellation", async () => {
    const child = fakeChild({ writeError: new Error(`write failed: ${prompt}`) });
    const runner = createNodeRuntimeProcessRunner({
      cancellationGraceMs: 5,
      terminationConfirmationMs: 5,
      spawnProcess: () => child.process,
    });
    const run = runner.start(spec, prompt);

    await expect(run.completion).resolves.toMatchObject({
      cancelled: false,
      forcedTerminationAttempted: true,
      terminationUnconfirmed: true,
      inputDeliveryFailed: true,
    });
    expect(child.signals).toEqual(["SIGINT", "SIGKILL"]);
  });

  it("writes nothing to stdin for a runtime whose prompt is already in its argv", async () => {
    // OpenCode and Copilot CLI take the prompt as a positional. Writing it a
    // second time would put it where the CLI is not reading, and some CLIs
    // treat anything on stdin as further input.
    const child = fakeChild();
    const runner = createNodeRuntimeProcessRunner({
      environment: { PATH: "C:\\tools" },
      spawnProcess: () => child.process,
    });

    const run = runner.start(
      { ...spec, runtime: "opencode", args: ["run", "--format", "json", prompt], stdin: "none" },
      prompt,
    );
    child.close(0);
    await run.completion;

    expect(child.stdin.writes).toEqual([]);
    expect(child.stdin.ended).toBe(true);
  });

  it("puts a spec's own variables on top of the allowlist, where nothing on the machine can undo them", async () => {
    // OpenCode's read-only permission config arrives this way and is the only
    // thing holding that runtime back. Two things have to hold: a machine that
    // exported the same name does not get a say (it is not on the allowlist,
    // so it never reaches the child), and where a name IS on the allowlist the
    // spec's value still wins.
    const child = fakeChild();
    let launched: RuntimeSpawnOptions | undefined;
    const runner = createNodeRuntimeProcessRunner({
      environment: { PATH: "C:\\tools", NO_COLOR: "0", OPENCODE_CONFIG_CONTENT: "{}" },
      spawnProcess: (_executablePath, _args, options) => {
        launched = options;
        return child.process;
      },
    });

    const run = runner.start(
      {
        ...spec,
        runtime: "opencode",
        args: ["run", "--format", "json", prompt],
        stdin: "none",
        env: { OPENCODE_CONFIG_CONTENT: '{"permission":{"write":"deny"}}', NO_COLOR: "1" },
      },
      prompt,
    );
    child.close(0);
    await run.completion;

    expect(launched?.env).toMatchObject({
      PATH: "C:\\tools",
      NO_COLOR: "1",
      OPENCODE_CONFIG_CONTENT: '{"permission":{"write":"deny"}}',
    });
  });
});

// H8: a mission started through a .cmd launcher is spawned with the command
// line cmd.exe reads correctly -- one verbatim argument, not Node's quoting.
describe("a mission started through a .cmd launcher", () => {
  // cmd.exe and its .cmd launchers exist on a Windows host only, and only one takes System32's path as absolute.
  it.skipIf(process.platform !== "win32")("gets one verbatim command line, launcher and every argument quoted", () => {
    const child = fakeChild();
    let launched: { readonly args: readonly string[]; readonly options: RuntimeSpawnOptions } | undefined;
    const runner = createNodeRuntimeProcessRunner({
      environment: { PATH: "C:\\tools" },
      spawnProcess: (_executablePath, args, options) => {
        launched = { args, options };
        return child.process;
      },
    });
    const muse = "C:\\Users\\Jane Doe\\AppData\\Local\\Programs\\muse\\muse.cmd";
    runner.start({ ...spec, executablePath: "C:\\Windows\\System32\\cmd.exe", args: ["/d", "/s", "/c", muse, "exec", "--json", "R&D"] }, "go");
    expect(launched?.args).toHaveLength(1);
    expect(launched?.args[0]?.startsWith('/d /s /c "')).toBe(true);
    expect(launched?.args[0]).toContain("Jane^ Doe");
    expect(launched?.args[0]).toContain('^"R^&D^"');
    expect((launched?.options as { windowsVerbatimArguments?: boolean } | undefined)?.windowsVerbatimArguments).toBe(true);
  });
});

/*
 * A2.10, the Claude half. `--input-format stream-json` takes the prompt as a
 * user turn and reads more while the turn runs -- MEASURED 2026-09-25 on
 * claude 2.1.282: a message written after the first tool call was taken into
 * the same turn. And it does NOT exit after its result while input is open,
 * so the runner must close input on that record or the run never ends.
 */
describe("a stream-json run", () => {
  const streamSpec = { ...spec, runtime: "claude", args: ["--print"], stdin: "stream-json" } as const;
  const userTurn = (text: string) => `${JSON.stringify({ type: "user", message: { role: "user", content: text } })}\n`;

  it("sends the prompt as a user turn and keeps input open for the running turn", async () => {
    const child = fakeChild();
    const run = createNodeRuntimeProcessRunner({ spawnProcess: () => child.process }).start(streamSpec, prompt);
    expect(child.stdin.writes).toEqual([userTurn(prompt)]);
    expect(child.stdin.ended).toBe(false);
    child.stdout.emit("data", '{"type":"assistant","message":{"content":[]}}\n');
    expect(run.send?.("take this too")).toBe(true);
    expect(child.stdin.writes.at(-1)).toBe(userTurn("take this too"));
    expect(child.stdin.ended).toBe(false);
    child.close(0);
    await run.completion;
  });

  it("closes input on the turn's result, after which nothing more is sent", async () => {
    const child = fakeChild();
    const run = createNodeRuntimeProcessRunner({ spawnProcess: () => child.process }).start(streamSpec, prompt);
    child.stdout.emit("data", '{"type":"result","subtype":"success","result":"done"}\n');
    expect(child.stdin.ended).toBe(true);
    expect(run.send?.("too late")).toBe(false);
    expect(child.stdin.writes).toHaveLength(1);
    child.close(0);
    await run.completion;
  });

  /*
   * 2026-10-09, arena round 4: Sonnet's result came at 01:21, but a `python`
   * it had started (the Windows Store stub, which waits forever) kept Claude
   * Code alive, and the column read "working" until a person ended it.
   */
  it("ends a run that is still alive after its result, and says it ended after its result", async () => {
    vi.useFakeTimers()
    try {
      const child = fakeChild({ onKill: (_signal, self) => self.close(1) });
      const run = createNodeRuntimeProcessRunner({ lingerAfterResultMs: 5_000, spawnProcess: () => child.process }).start(streamSpec, prompt);
      child.stdout.emit("data", '{"type":"result","subtype":"success","result":"done"}\n');
      expect(child.stdin.ended).toBe(true);
      await vi.advanceTimersByTimeAsync(4_999);
      expect(child.signals).toEqual([]);
      await vi.advanceTimersByTimeAsync(1);
      await expect(run.completion).resolves.toMatchObject({ endedAfterResult: true, cancelled: false, exitCode: 1 });
    } finally {
      vi.useRealTimers()
    }
  });

  it("leaves alone a run that exits on its own after its result", async () => {
    vi.useFakeTimers()
    try {
      const child = fakeChild();
      const run = createNodeRuntimeProcessRunner({ lingerAfterResultMs: 5_000, spawnProcess: () => child.process }).start(streamSpec, prompt);
      child.stdout.emit("data", '{"type":"result","subtype":"success","result":"done"}\n');
      child.close(0);
      await vi.advanceTimersByTimeAsync(10_000);
      expect(child.signals).toEqual([]);
      await expect(run.completion).resolves.toMatchObject({ endedAfterResult: false, exitCode: 0 });
    } finally {
      vi.useRealTimers()
    }
  });

  it("does not take text that merely mentions a result for the result", async () => {
    const child = fakeChild();
    const run = createNodeRuntimeProcessRunner({ spawnProcess: () => child.process }).start(streamSpec, prompt);
    child.stdout.emit("data", '{"type":"assistant","message":{"content":[{"type":"text","text":"the \\"result\\" is 4"}]}}\n');
    expect(child.stdin.ended).toBe(false);
    child.close(0);
    await run.completion;
  });

  /*
   * QA-2026-09-29, Q2: the result record repeats the whole answer, so a
   * 300 KB answer made a result line past the 256 KB record cap. It was
   * dropped, input stayed open, and the CLI waited for a next turn forever.
   */
  it("keeps a result line past the ordinary cap, and ends the turn on it, arriving in pieces", async () => {
    const child = fakeChild();
    const run = createNodeRuntimeProcessRunner({ maxRecordBytes: 64, spawnProcess: () => child.process }).start(streamSpec, prompt);
    const iterator = run.records[Symbol.asyncIterator]();
    const answer = "x".repeat(300);
    const line = JSON.stringify({ type: "result", subtype: "success", result: answer, session_id: "s1" });
    child.stdout.emit("data", line.slice(0, 100));
    child.stdout.emit("data", line.slice(100, 250));
    child.stdout.emit("data", `${line.slice(250)}\n`);
    expect(child.stdin.ended).toBe(true);
    child.close(0);
    await expect(iterator.next()).resolves.toEqual({ done: false, value: { sequence: 1, raw: line } });
    await expect(run.completion).resolves.toMatchObject({ oversizedRecordsDropped: 0, recordCount: 1 });
  });

  it("still ends the turn on a result line too large even for its own cap", async () => {
    const child = fakeChild();
    const run = createNodeRuntimeProcessRunner({ maxRecordBytes: 32, maxResultRecordBytes: 64, spawnProcess: () => child.process }).start(streamSpec, prompt);
    const line = JSON.stringify({ type: "result", subtype: "success", result: "y".repeat(200) });
    child.stdout.emit("data", line.slice(0, 120));
    expect(child.stdin.ended).toBe(true);
    child.stdout.emit("data", `${line.slice(120)}\n`);
    child.close(0);
    await expect(run.completion).resolves.toMatchObject({ oversizedRecordsDropped: 1, recordCount: 0 });
  });

  it("gives an ordinary line no larger cap, whatever it says", async () => {
    const child = fakeChild();
    const run = createNodeRuntimeProcessRunner({ maxRecordBytes: 64, spawnProcess: () => child.process }).start(streamSpec, prompt);
    child.stdout.emit("data", `${JSON.stringify({ type: "assistant", text: `"type":"result" ${"z".repeat(200)}` })}\n`);
    expect(child.stdin.ended).toBe(false);
    child.close(0);
    await expect(run.completion).resolves.toMatchObject({ oversizedRecordsDropped: 1, recordCount: 0 });
  });

  it("offers no send on any other transport", () => {
    const child = fakeChild();
    const run = createNodeRuntimeProcessRunner({ spawnProcess: () => child.process }).start(spec, prompt);
    expect(run.send).toBeUndefined();
    expect(child.stdin.ended).toBe(true);
  });
});
