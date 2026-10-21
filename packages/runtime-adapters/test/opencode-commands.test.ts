import { describe, expect, it, vi } from "vitest";

import { createOpenCodeRunCommand, createOpenCodeServeCommand } from "../src/commands.js";
import { readOpenCodeCommands } from "../src/opencode-commands.js";
import type { AppServerRunProcess } from "../src/codex-app-server-run.js";

/*
 * OpenCode's own commands (0.427). The list's shape is 1.18.27's `GET
 * /command`, measured 2026-09-28: `{name, description, source, template,
 * hints}`, with `init` and `review` built in and every skill beside them.
 */
const EXECUTABLE = { executablePath: "C:/tools/opencode.exe", prefixArgs: [] as readonly string[] };
const LISTED = [
  { name: "init", description: "guided AGENTS.md setup", source: "command", template: "Create or update AGENTS.md ... $ARGUMENTS", hints: ["$ARGUMENTS"] },
  { name: "review", description: "review changes [commit|branch|pr], defaults to uncommitted", source: "command", template: "You are a code reviewer ... $ARGUMENTS", hints: ["$ARGUMENTS"] },
  { name: "deploy", description: "Ship it", source: "command", template: "Deploy $1 to $2", hints: ["$1", "$2"] },
  { name: "a-skill", description: "A skill", source: "skill", template: "..." },
  { name: "not a name", description: "x", source: "command", template: "x" },
];

function fakeServer(answer: () => Response) {
  const state = { env: undefined as Readonly<Record<string, string>> | undefined, cwd: undefined as string | undefined, killed: false, urls: [] as string[], auth: [] as (string | undefined)[] };
  const spawn = (_exe: string, _args: readonly string[], env?: Readonly<Record<string, string>>, cwd?: string): AppServerRunProcess => {
    state.env = env;
    state.cwd = cwd;
    return {
      write: () => undefined,
      kill: () => { state.killed = true; },
      onData: (listener) => { setTimeout(() => listener("opencode server listening on http://127.0.0.1:4096\n"), 0); },
      onExit: () => undefined,
    };
  };
  const fetcher = (async (input: string | URL | Request, init?: RequestInit) => {
    state.urls.push(String(input));
    state.auth.push(((init?.headers ?? {}) as Record<string, string>).authorization);
    return answer();
  }) as typeof fetch;
  return { spawn, fetcher, state };
}

describe("OpenCode's own commands, listed", () => {
  it("never starts the V1 command server when OpenCode 2 is installed", async () => {
    const spawn = vi.fn(() => { throw new Error("A V1 server must not start for V2"); });
    const fetcher = vi.fn();
    const command = createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" });
    await expect(readOpenCodeCommands({ spawn, command, version: { version: "2.1.1" }, fetch: fetcher as typeof fetch })).resolves.toEqual([]);
    expect(spawn).not.toHaveBeenCalled();
    expect(fetcher).not.toHaveBeenCalled();
  });
  it("are read from a server started in the folder, with a password and no plugins, which is then stopped", async () => {
    const server = fakeServer(() => Response.json(LISTED));
    const command = createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" });
    const commands = await readOpenCodeCommands({ spawn: server.spawn, command, fetch: server.fetcher });
    expect(commands).toEqual([
      { name: "init", description: "guided AGENTS.md setup", argumentHint: "[arguments]" },
      { name: "review", description: "review changes [commit|branch|pr], defaults to uncommitted", argumentHint: "[arguments]" },
      { name: "deploy", description: "Ship it", argumentHint: "$1 $2" },
      { name: "a-skill", description: "A skill", argumentHint: "" },
    ]);
    expect(server.state.cwd).toBe("C:/work/pebble");
    expect(server.state.urls).toEqual([`http://127.0.0.1:4096/command?directory=${encodeURIComponent("C:/work/pebble")}`]);
    expect(server.state.env?.OPENCODE_PURE).toBe("1");
    expect(server.state.env?.OPENCODE_SERVER_PASSWORD?.length).toBeGreaterThan(20);
    expect(server.state.auth[0]).toMatch(/^Basic /);
    expect(server.state.killed).toBe(true);
  });

  it("is a refusal, not an empty menu, when the server will not say, and the server is still stopped", async () => {
    const server = fakeServer(() => new Response(null, { status: 401 }));
    const command = createOpenCodeServeCommand(EXECUTABLE, { workspacePath: "C:/work/pebble" });
    await expect(readOpenCodeCommands({ spawn: server.spawn, command, fetch: server.fetcher })).rejects.toThrow(/401/);
    expect(server.state.killed).toBe(true);
  });
});

describe("one of OpenCode's commands, run", () => {
  it("is named on the command line, and its arguments are the message on stdin", () => {
    const spec = createOpenCodeRunCommand(EXECUTABLE, { workspacePath: "C:/work/pebble", prompt: "Keep it under five lines.", slashCommand: "init" });
    const at = spec.args.indexOf("--command");
    expect(spec.args[at + 1]).toBe("init");
    expect(spec.stdin).toBe("prompt");
    expect(spec.args).not.toContain("Keep it under five lines.");
  });

  it("refuses a name no command could have", () => {
    expect(() => createOpenCodeRunCommand(EXECUTABLE, { workspacePath: "C:/work/pebble", prompt: "x", slashCommand: "init; rm -rf" })).toThrow(/not a command name/);
    expect(() => createOpenCodeRunCommand(EXECUTABLE, { workspacePath: "C:/work/pebble", prompt: "x", slashCommand: "--auto" })).toThrow(/not a command name/);
  });

  it("is an ordinary run when there is no command", () => {
    expect(createOpenCodeRunCommand(EXECUTABLE, { workspacePath: "C:/work/pebble", prompt: "hi" }).args).not.toContain("--command");
  });
});
