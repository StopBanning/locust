import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { createOpenCodeServeCommand } from "../src/commands.js";
import { createOpenCodeEventNormalizer } from "../src/opencode-events.js";
import { runRecordFor, startOpenCodeServeRun } from "../src/opencode-serve-run.js";
import type { OpenCodePermission } from "../src/opencode-serve-run.js";
import type { AppServerRunProcess } from "../src/codex-app-server-run.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";

/*
 * A6.7. The events are opencode 1.18.27's own, from one served turn captured
 * 2026-09-25 (free Ling, `bash: "ask"`): read README.md, ask to run
 * `echo SERVED`, run it once approved, answer with the README's first line.
 */
const EVENTS = readFileSync(new URL("./fixtures/opencode/serve-events.jsonl", import.meta.url), "utf8")
  .split("\n").filter((line) => line.length > 0);
const SESSION = "ses_f27b5f958ffea6iVnmfRjD2imJ";
const EXECUTABLE = { executablePath: "C:/tools/opencode.exe", prefixArgs: [] as readonly string[] };

function fakeServer(events: readonly string[] = EVENTS) {
  const calls: { url: string; auth: string | undefined; body: unknown }[] = [];
  let spawnedEnv: Readonly<Record<string, string>> | undefined;
  let spawnedArgs: readonly string[] = [];
  let spawnedIn: string | undefined;
  let killed = false;
  let prompted!: () => void;
  const promptSent = new Promise<void>((resolve) => { prompted = resolve; });
  const spawn = (_exe: string, args: readonly string[], env?: Readonly<Record<string, string>>, cwd?: string): AppServerRunProcess => {
    spawnedArgs = args;
    spawnedEnv = env;
    spawnedIn = cwd;
    return {
      write: () => undefined,
      kill: () => { killed = true; },
      onData: (listener) => { setTimeout(() => listener("opencode server listening on http://127.0.0.1:4096\n"), 0); },
      onExit: () => undefined,
    };
  };
  const fetcher = (async (input: string | URL | Request, init?: RequestInit) => {
    const url = String(input);
    const headers = (init?.headers ?? {}) as Record<string, string>;
    calls.push({ url, auth: headers.authorization, body: init?.body === undefined ? undefined : JSON.parse(String(init.body)) });
    if (url.endsWith("/event")) {
      const encoder = new TextEncoder();
      const body = new ReadableStream<Uint8Array>({
        async start(controller) {
          await promptSent;
          for (const line of events) controller.enqueue(encoder.encode(`data: ${line}\n\n`));
        },
      });
      return new Response(body, { status: 200 });
    }
    if (url.endsWith("/session")) return Response.json({ id: SESSION });
    if (url.endsWith("/prompt_async")) {
      prompted();
      return new Response(null, { status: 204 });
    }
    // A command (0.427) answers only once its turn is over.
    if (url.endsWith("/command")) {
      prompted();
      return Response.json({ info: {}, parts: [] });
    }
    if (url.includes("/permission/")) return Response.json(true);
    return new Response(null, { status: 404 });
  }) as typeof fetch;
  return { spawn, fetcher, calls, get env() { return spawnedEnv; }, get args() { return spawnedArgs; }, get cwd() { return spawnedIn; }, get killed() { return killed; } };
}

describe("OpenCode through its own server (A6.7)", () => {
  it("starts the server IN the teammate's folder, never where the app itself stands (0.378)", async () => {
    // A session made without naming a folder is the server's own folder's.
    // Started with none, the server stood in the app's -- in real use its
    // install directory -- and an approved edit would have landed there.
    const server = fakeServer();
    const command = createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" });
    const run = startOpenCodeServeRun({ spawn: server.spawn, command, prompt: "hi", fetch: server.fetcher });
    expect(command.cwd).toBe("C:/work/pebble");
    expect(server.cwd).toBe("C:/work/pebble");
    for await (const record of run.records) void record;
    await run.completion;
  });

  it("asks the person, sends their answer back, and reads as an ordinary OpenCode run", async () => {
    const server = fakeServer();
    const asked: OpenCodePermission[] = [];
    const command = createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" });
    const run = startOpenCodeServeRun({
      spawn: server.spawn,
      command,
      prompt: "Read README.md, then run echo SERVED.",
      model: "opencode/ling-3.0-flash-fin-free",
      variant: "high",
      fetch: server.fetcher,
      onPermission: async (request) => {
        asked.push(request);
        return "once";
      },
    });
    const normalizer = createOpenCodeEventNormalizer({ runId: "run_1", missionId: "mission_1", cliVersion: "1.18.27" });
    const events: NormalizedRuntimeEvent[] = [];
    const types: string[] = [];
    for await (const record of run.records) {
      types.push(String((JSON.parse(record.raw) as { type: string }).type));
      events.push(...normalizer.accept(record));
    }
    const completion = await run.completion;
    events.push(...normalizer.finish(completion));

    // The run's own records, in `run --format json`'s words: two steps, two
    // tools, the answer -- and never the person's prompt echoed back.
    // The answer, finished: this recording's one update before that carries no text yet, and empty text is not
    // passed on (0.677; a reply that streams is an-opencode-reply-streams and opencode-every-mode-through-its-server).
    expect(types).toEqual(["step_start", "tool_use", "tool_use", "step_finish", "step_start", "text", "step_finish"]);
    // The shell call was put to the person, and their answer went back.
    expect(asked.map((one) => [one.permission, one.metadata.command])).toEqual([["bash", "echo SERVED"]]);
    const reply = server.calls.find((call) => call.url.includes("/permission/"));
    expect(reply?.body).toEqual({ reply: "once" });
    // The model and effort as the server takes them.
    expect(server.calls.find((call) => call.url.endsWith("/prompt_async"))?.body).toMatchObject({
      model: { providerID: "opencode", modelID: "ling-3.0-flash-fin-free" },
      variant: "high",
    });
    // Every call carried the password, which is in the child's environment only.
    const password = server.env?.OPENCODE_SERVER_PASSWORD ?? "";
    expect(password.length).toBeGreaterThan(20);
    expect(server.calls.every((call) => call.auth === `Basic ${Buffer.from(`opencode:${password}`).toString("base64")}`)).toBe(true);
    expect(JSON.stringify(command)).not.toContain(password);
    expect(server.args).toEqual(["serve", "--port", "0", "--hostname", "127.0.0.1"]);
    // And it ends like any OpenCode run that answered.
    expect(completion.exitCode).toBe(0);
    expect(server.killed).toBe(true);
    const said = events.filter((event) => event.type === "message.delta").map((event) => (event.payload as { text: string }).text).join("");
    expect(said).toContain("First line of the readme");
    expect(events.at(-1)?.type).toBe("run.completed");
  });

  // QA-2026-09-29 round 2, R36: a subagent (the `task` tool) is a child
  // session, and its request came on this stream under the child's id --
  // neither shown nor answered, and the run waited on it.
  it("asks the person a subagent's request too, marked as the subagent's, and answers it", async () => {
    const CHILD = "ses_child_of_the_run";
    const GRANDCHILD = "ses_grandchild_of_the_run";
    const events = [
      JSON.stringify({ type: "session.created", properties: { info: { id: CHILD, parentID: SESSION } } }),
      JSON.stringify({ type: "session.created", properties: { info: { id: GRANDCHILD, parentID: CHILD } } }),
      JSON.stringify({ type: "permission.asked", properties: { id: "per_child", sessionID: GRANDCHILD, permission: "bash", patterns: ["echo FROM-SUBAGENT"], metadata: { command: "echo FROM-SUBAGENT" }, always: ["echo *"] } }),
      // Another session's request -- not the run's, nor a child of it -- is not ours.
      JSON.stringify({ type: "permission.asked", properties: { id: "per_stranger", sessionID: "ses_someone_else", permission: "bash", patterns: ["rm -rf /"], metadata: { command: "rm -rf /" }, always: [] } }),
      ...EVENTS,
    ];
    const server = fakeServer(events);
    const asked: OpenCodePermission[] = [];
    const command = createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" });
    const run = startOpenCodeServeRun({ spawn: server.spawn, command, prompt: "hi", fetch: server.fetcher, onPermission: async (request) => { asked.push(request); return "once"; } });
    for await (const record of run.records) void record;
    await run.completion;
    expect(asked.map((one) => [one.metadata.command, one.bySubagent === true])).toEqual([["echo FROM-SUBAGENT", true], ["echo SERVED", false]]);
    expect(server.calls.filter((call) => call.url.includes("/permission/")).map((call) => call.url.split("/permission/")[1])).toEqual(["per_child/reply", "per_0d84a14d3001uIu5oHyC0O5Ij8/reply"]);
  });

  it("refuses what nobody is there to answer", async () => {
    const server = fakeServer();
    const run = startOpenCodeServeRun({
      spawn: server.spawn,
      command: createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" }),
      prompt: "go",
      fetch: server.fetcher,
    });
    for await (const _record of run.records) { /* drain */ }
    await run.completion;
    expect(server.calls.find((call) => call.url.includes("/permission/"))?.body).toEqual({ reply: "reject", message: "The person declined this in Locust." });
  });

  it("passes on why the person declined, when they said (0.374)", async () => {
    const server = fakeServer();
    const run = startOpenCodeServeRun({
      spawn: server.spawn,
      command: createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" }),
      prompt: "go",
      fetch: server.fetcher,
      onPermission: async () => ({ reply: "reject", message: "The person declined this, and said: use the build script" }),
    });
    for await (const _record of run.records) { /* drain */ }
    await run.completion;
    expect(server.calls.find((call) => call.url.includes("/permission/"))?.body).toEqual({
      reply: "reject",
      message: "The person declined this, and said: use the build script",
    });
  });

  it("sends one of OpenCode's own commands as the command, its arguments beside it, and reads the turn the same way (0.427)", async () => {
    const server = fakeServer();
    const command = createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" });
    const run = startOpenCodeServeRun({
      spawn: server.spawn,
      command,
      prompt: "focus on the tests",
      slashCommand: "review",
      model: "opencode/ling-3.0-flash-fin-free",
      fetch: server.fetcher,
    });
    const records: string[] = [];
    for await (const record of run.records) records.push(record.raw);
    const completion = await run.completion;
    expect(server.calls.some((call) => call.url.endsWith("/prompt_async"))).toBe(false);
    expect(server.calls.find((call) => call.url.endsWith(`/session/${SESSION}/command`))?.body).toEqual({
      command: "review",
      arguments: "focus on the tests",
      model: "opencode/ling-3.0-flash-fin-free",
    });
    expect(records.length).toBeGreaterThan(0);
    expect(completion.exitCode).toBe(0);
  });

  it("asks for every action in its config, and never announces itself on the network", () => {
    const command = createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" });
    const config = JSON.parse(command.env?.OPENCODE_CONFIG_CONTENT ?? "{}");
    expect(config).toEqual({
      tools: { question: false },
      permission: {
        // Every tool it does not name asks too (0.821): a person's own MCP tool used to run with no card.
        "*": "ask",
        // Reading is never a card; a `.env` file is the person's to open.
        read: { "*": "allow", "*.env": "ask", "*.env.*": "ask", "*.env.example": "allow" },
        glob: "allow",
        grep: "allow",
        list: "allow",
        lsp: "allow",
        task: "allow",
        todowrite: "allow",
        todoread: "allow",
        skill: "allow",
        edit: "ask",
        bash: "ask",
        webfetch: "ask",
        websearch: "ask",
        codesearch: "ask",
        external_directory: "ask",
      },
    });
    // First, or OpenCode's last-match rule would make it outrank the reading tools after it.
    expect(Object.keys(config.permission)[0]).toBe("*");
    expect(command.args).not.toContain("--mdns");
  });

  it("turns settled parts into records, and the reply's text while it is written into partial ones (0.677)", () => {
    expect(runRecordFor({ type: "text", text: "the prompt" }, "user")).toBeUndefined();
    expect(runRecordFor({ type: "text", text: "half", time: { start: 1 } }, "assistant")?.type).toBe("text_partial");
    // Not the person's words, nor OpenCode's own notes, while they are written.
    expect(runRecordFor({ type: "text", text: "half", time: { start: 1 } }, "user")).toBeUndefined();
    expect(runRecordFor({ type: "text", synthetic: true, text: "Contin", time: { start: 1 } }, "assistant")).toBeUndefined();
    expect(runRecordFor({ type: "text", text: "done", time: { start: 1, end: 2 } }, "assistant")?.type).toBe("text");
    // OpenCode's own continue note arrives on a message of its own; the
    // normalizer needs it to see a compaction.
    expect(runRecordFor({ type: "text", synthetic: true, text: "Continue", time: { start: 1, end: 1 } }, "user")?.type).toBe("text");
    expect(runRecordFor({ type: "tool", state: { status: "running" } }, "assistant")).toBeUndefined();
    expect(runRecordFor({ type: "tool", state: { status: "error" } }, "assistant")?.type).toBe("tool_use");
  });
});
