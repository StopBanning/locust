import { afterEach, describe, expect, it, vi } from "vitest";

import type { AppServerRunProcess } from "../src/codex-app-server-run.js";
import { createOpenCodeServeCommand } from "../src/commands.js";
import { startOpenCodeServeRun } from "../src/opencode-serve-run.js";

/*
 * A SERVER WHOSE STORAGE IS BUSY STARTS AGAIN (0.810). On 2026-10-21 a panel of three judges started three OpenCode
 * servers at once; one died before it listened, its last words "database is locked", and that judge was lost. Nothing
 * had been sent to it, so such a server is started again -- later each time, never more than three times.
 */
const SESSION = "ses_busy_storage";
const EXECUTABLE = { commandName: "opencode", discoveredPath: "C:/tools/opencode.exe", executablePath: "C:/tools/opencode.exe", prefixArgs: [] as readonly string[], kind: "native" as const };
// Its own words, from the judge it lost.
const LOCKED = "\u001b[91m\u001b[1mError: \u001b[0mUnexpected error\r\n\r\ndatabase is locked";
const assistant = { type: "message.updated", properties: { sessionID: SESSION, info: { id: "msg_answer", role: "assistant" } } };
const idle = { type: "session.idle", properties: { sessionID: SESSION } };

/** Each server dies with the next of `deaths` (its stderr), or listens once they run out. */
function servers(deaths: readonly string[], options: { readonly lateWords?: boolean } = {}) {
  const started: string[] = [];
  const prompts: string[] = [];
  let prompted!: () => void;
  const promptSent = new Promise<void>((resolve) => { prompted = resolve; });
  const spawn = (): AppServerRunProcess => {
    const death = deaths[started.length];
    started.push(death ?? "listening");
    let said = options.lateWords === true ? "" : death ?? "";
    return {
      write: () => undefined,
      kill: () => undefined,
      onData: (listener) => { if (death === undefined) queueMicrotask(() => listener("opencode server listening on http://127.0.0.1:4096\n")); },
      onExit: (listener) => {
        if (death === undefined) return;
        queueMicrotask(() => {
          listener();
          // Words that reach the pipe just after the exit does.
          said = death;
        });
      },
      stderrTail: () => said,
    };
  };
  const fetcher = (async (input: string | URL | Request) => {
    const path = new URL(String(input)).pathname;
    if (path === "/event") {
      return new Response(new ReadableStream<Uint8Array>({
        async start(controller) {
          await promptSent;
          for (const event of [assistant, idle]) controller.enqueue(new TextEncoder().encode(`data: ${JSON.stringify(event)}\n\n`));
          controller.close();
        },
      }), { status: 200 });
    }
    if (path === "/session") return Response.json({ id: SESSION });
    if (path.endsWith("/prompt_async")) {
      prompts.push(path);
      prompted();
      return new Response(null, { status: 204 });
    }
    return new Response(null, { status: 404 });
  }) as typeof fetch;
  return { spawn, fetcher, started, prompts };
}

const started = (server: ReturnType<typeof servers>, signal?: AbortSignal) =>
  startOpenCodeServeRun({
    spawn: server.spawn,
    command: createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" }),
    prompt: "Judge these three answers.",
    fetch: server.fetcher,
    ...(signal === undefined ? {} : { signal }),
  });

describe("an OpenCode server that dies before it listens", () => {
  afterEach(() => { vi.useRealTimers(); });

  it("starts again when another OpenCode held its storage, and the turn goes on as usual", async () => {
    vi.useFakeTimers();
    const server = servers([LOCKED]);
    const run = started(server);
    await vi.advanceTimersByTimeAsync(1_000);
    vi.useRealTimers();
    for await (const _record of run.records) void _record;
    expect(await run.completion).toMatchObject({ exitCode: 0, cancelled: false, stderr: "" });
    expect(server.started).toEqual([LOCKED, "listening"]);
    expect(server.prompts).toHaveLength(1);
  });

  it("reads words that trail the exit before deciding", async () => {
    vi.useFakeTimers();
    const server = servers([LOCKED], { lateWords: true });
    const run = started(server);
    await vi.advanceTimersByTimeAsync(1_500);
    vi.useRealTimers();
    for await (const _record of run.records) void _record;
    expect((await run.completion).exitCode).toBe(0);
    expect(server.started).toHaveLength(2);
  });

  it("gives up after three restarts, saying the server exited and why", async () => {
    vi.useFakeTimers();
    const server = servers([LOCKED, LOCKED, LOCKED, LOCKED]);
    const run = started(server);
    await vi.advanceTimersByTimeAsync(10_000);
    const ended = await run.completion;
    expect(ended.exitCode).toBe(1);
    expect(ended.stderr).toContain("exited before the turn finished");
    expect(ended.stderr).toContain("database is locked");
    expect(server.started).toHaveLength(4);
    expect(server.prompts).toHaveLength(0);
  });

  it("is not started again for any other reason", async () => {
    vi.useFakeTimers();
    const server = servers(["Error: Config file at opencode.json is not valid JSON"]);
    const run = started(server);
    await vi.advanceTimersByTimeAsync(10_000);
    const ended = await run.completion;
    expect(ended.exitCode).toBe(1);
    expect(ended.stderr).toContain("not valid JSON");
    expect(server.started).toHaveLength(1);
  });

  it("starts nothing more once the person stops it", async () => {
    vi.useFakeTimers();
    const stopping = new AbortController();
    const server = servers([LOCKED]);
    const run = started(server, stopping.signal);
    await vi.advanceTimersByTimeAsync(0);
    stopping.abort();
    await vi.advanceTimersByTimeAsync(10_000);
    expect(await run.completion).toMatchObject({ exitCode: null, cancelled: true });
    expect(server.started).toHaveLength(1);
  });
});
