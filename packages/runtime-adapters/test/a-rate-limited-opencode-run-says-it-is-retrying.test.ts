import { EventEmitter } from "node:events";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { createOpenCodeRunCommand } from "../src/commands.js";
import { createOpenCodeEventNormalizer } from "../src/opencode-events.js";
import { createNodeRuntimeProcessRunner } from "../src/process-runner.js";
import type { RuntimeJsonlRecord, SpawnedRuntimeProcess } from "../src/process-runner.js";

/*
 * A free OpenCode model that is rate limited sat on "Starting" for minutes
 * (Colin, 2026-09-25: "is this a locust issue or open code issue?"). MEASURED
 * the same day against a local provider answering 429: `opencode run` retries
 * with nothing on stdout or stderr until it gives up. With `--print-logs
 * --log-level ERROR` each attempt is one stderr line, and those lines now
 * reach the thread as one sentence.
 *
 * Not a timer: Colin had the timed "no word back yet" line removed on
 * 2026-09-13 because it fired on ordinary runs. This says something only
 * when OpenCode itself says a request failed.
 */

const CAPTURED = readFileSync(new URL("./fixtures/opencode/rate-limited-1.18.27.stderr.txt", import.meta.url), "utf8")
  .split("\n")
  .filter((line) => line.trim().length > 0);

const NOW = "2026-09-25T19:39:22.000Z";

function normalizer() {
  return createOpenCodeEventNormalizer({ runId: "run_1", missionId: "mission_1", cliVersion: "1.18.27", now: () => new Date(NOW) });
}

function stderrRecord(line: string, sequence: number): RuntimeJsonlRecord {
  return { sequence, raw: JSON.stringify({ type: "locust.stderr", line }) };
}

describe("a rate-limited OpenCode run says it is retrying", () => {
  it("says so once for the captured run's six rate-limited attempts, naming what the provider said", () => {
    const opencode = normalizer();
    const events = CAPTURED.flatMap((line, index) => opencode.accept(stderrRecord(line, index + 1)));

    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({
      type: "adapter.diagnostic",
      payload: {
        level: "warning",
        // `*.runtime_error` is what the thread shows before any tool has run,
        // the rule Codex's "Reconnecting... 2/5" already goes through.
        code: "opencode.provider_busy.runtime_error",
        message: 'The model\'s provider answered "Rate limit exceeded", and OpenCode is trying again on its own. To go on now, press Stop and pick another model.',
      },
    });
  }, 10_000);

  it("ignores the title agent's failure and every other log line", () => {
    const opencode = normalizer();
    const title = CAPTURED.find((line) => /agent=title/.test(line));
    const process = CAPTURED.find((line) => /message=process/.test(line));
    expect(title).toBeDefined();
    expect(process).toBeDefined();

    expect(opencode.accept(stderrRecord(title!, 1))).toEqual([]);
    expect(opencode.accept(stderrRecord(process!, 2))).toEqual([]);
    expect(opencode.accept(stderrRecord("permission requested: bash (ls); auto-rejecting", 3))).toEqual([]);
  }, 10_000);

  it("says a different error again, once, when it too is retried", () => {
    const opencode = normalizer();
    const line = (said: string) =>
      `timestamp=${NOW} level=ERROR run=r message="stream error" providerID=p modelID=m session.id=s small=false agent=build mode=primary error.error="${said}"`;

    // A rate limit and an overload are always retried: each is said on its first line (0.368).
    expect(opencode.accept(stderrRecord(line("AI_APICallError: Rate limit exceeded"), 1))).toHaveLength(1);
    expect(opencode.accept(stderrRecord(line("AI_APICallError: Rate limit exceeded"), 2))).toHaveLength(0);
    expect(opencode.accept(stderrRecord(line("AI_APICallError: Rate limit exceeded"), 3))).toHaveLength(0);
    const overloaded = opencode.accept(stderrRecord(line("AI_APICallError: Overloaded"), 4));
    expect(opencode.accept(stderrRecord(line("AI_APICallError: Overloaded"), 5))).toHaveLength(0);
    expect(overloaded).toHaveLength(1);
    expect(overloaded[0]?.type === "adapter.diagnostic" ? overloaded[0].payload.message : "").toContain('"Overloaded"');
  }, 10_000);

  it("says it at once for the real free model, whose next try is minutes away (0.368)", () => {
    // The 0.367 sweep: one build line, then minutes of "Starting" with nothing
    // said, because the rule waited for a second line that was minutes off.
    const free = readFileSync(new URL("./fixtures/opencode/rate-limited-free-1.18.27.stderr.txt", import.meta.url), "utf8")
      .split("\n")
      .filter((line) => line.trim().length > 0);
    const opencode = normalizer();
    const first = opencode.accept(stderrRecord(free[0]!, 1));
    expect(first).toHaveLength(1);
    const said = first[0]?.type === "adapter.diagnostic" ? first[0].payload.message : "";
    expect(said).toContain('"Rate limit exceeded. Please try again later."');
    expect(said).toContain("press Stop and pick another model");
    expect(free.slice(1).flatMap((line, index) => opencode.accept(stderrRecord(line, index + 2)))).toEqual([]);
  }, 10_000);

  it("waits for the second line of an error of no known retried kind", () => {
    const opencode = normalizer();
    const odd =
      `timestamp=${NOW} level=ERROR run=r message="stream error" providerID=p modelID=m session.id=s small=false agent=build mode=primary error.error="AI_APICallError: The proxy closed the connection"`;
    expect(opencode.accept(stderrRecord(odd, 1))).toEqual([]);
    const second = opencode.accept(stderrRecord(odd, 2));
    expect(second).toHaveLength(1);
    expect(second[0]?.type === "adapter.diagnostic" ? second[0].payload.message : "").not.toContain("press Stop");
  }, 10_000);

  it("claims no retry for an error that is not retried (0.358)", () => {
    // A model that cannot take tools answers 400 once, and the run stops.
    // The line is logged, and nothing follows it: there is no retry to say.
    const opencode = normalizer();
    const refused =
      `timestamp=${NOW} level=ERROR run=r message="stream error" providerID=own-1 modelID=plain-1 session.id=s small=false agent=build mode=primary error.error="AI_APICallError: registry.example/plain-1 does not support tools"`;
    expect(opencode.accept(stderrRecord(refused, 1))).toEqual([]);
  }, 10_000);

  it("is asked for: every OpenCode run prints its errors and forwards them", () => {
    const spec = createOpenCodeRunCommand(
      { executablePath: "C:\\tools\\opencode.exe", prefixArgs: [] },
      { workspacePath: "C:\\work\\pebble", prompt: "say pong", sandbox: "workspace-write" },
    );

    const at = spec.args.indexOf("--print-logs");
    expect(at).toBeGreaterThan(0);
    expect(spec.args.slice(at, at + 3)).toEqual(["--print-logs", "--log-level", "ERROR"]);
    expect(spec.stderrRecords).toBe(true);
  }, 10_000);
});

describe("the runner forwards stderr lines as records when a spec asks", () => {
  function child() {
    const stdout = new EventEmitter();
    const stderr = new EventEmitter();
    const stdin = Object.assign(new EventEmitter(), {
      write: (_value: string, callback?: (error?: Error | null) => void) => {
        callback?.();
        return true;
      },
      end: () => undefined,
    });
    const process = Object.assign(new EventEmitter(), { stdin, stdout, stderr, kill: () => true });
    return { process: process as unknown as SpawnedRuntimeProcess, stdout, stderr, close: (code: number) => process.emit("close", code, null) };
  }
  // The runner starts on the machine running the test, which takes only its own kind of absolute path.
  const windows = process.platform === "win32";
  const base = {
    runtime: "opencode",
    executablePath: windows ? "C:\\tools\\opencode.exe" : "/tools/opencode",
    args: ["run", "--format", "json"],
    cwd: windows ? "C:\\work\\pebble" : "/work/pebble",
    stdin: "prompt",
    stdout: "jsonl",
  } as const;

  async function recordsOf(stderrRecords: boolean, emit: (fake: ReturnType<typeof child>) => void): Promise<string[]> {
    const fake = child();
    const run = createNodeRuntimeProcessRunner({ spawnProcess: () => fake.process }).start(
      { ...base, ...(stderrRecords ? { stderrRecords: true } : {}) },
      "the secret prompt",
    );
    emit(fake);
    fake.close(1);
    const raw: string[] = [];
    for await (const record of run.records) raw.push(record.raw);
    await run.completion;
    return raw;
  }

  it("in order with stdout, whole lines only, the prompt redacted", async () => {
    const raw = await recordsOf(true, (fake) => {
      fake.stdout.emit("data", '{"type":"step_start"}\n');
      fake.stderr.emit("data", "level=ERROR message=\"stream error\" first half");
      fake.stderr.emit("data", " and the rest\nsaid: the secret prompt\n");
    });

    expect(raw).toEqual([
      '{"type":"step_start"}',
      JSON.stringify({ type: "locust.stderr", line: 'level=ERROR message="stream error" first half and the rest' }),
      JSON.stringify({ type: "locust.stderr", line: "said: [prompt redacted]" }),
    ]);
  }, 10_000);

  it("not at all for a spec that did not ask", async () => {
    const raw = await recordsOf(false, (fake) => {
      fake.stderr.emit("data", "level=ERROR anything\n");
    });
    expect(raw).toEqual([]);
  }, 10_000);

  it("a character split across two chunks arrives whole", async () => {
    const bytes = Buffer.from("café\n", "utf8");
    const raw = await recordsOf(true, (fake) => {
      fake.stderr.emit("data", bytes.subarray(0, 4));
      fake.stderr.emit("data", bytes.subarray(4));
    });
    expect(raw).toEqual([JSON.stringify({ type: "locust.stderr", line: "café" })]);
  }, 10_000);
});
