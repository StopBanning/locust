import { describe, expect, it } from "vitest";

import { startCodexAppServerRun } from "../src/codex-app-server-run.js";
import type { AppServerRunProcess } from "../src/codex-app-server-run.js";
import {
  createCodexAppServerCommand,
  createOpenCodeRunCommand,
  createOpenCodeServeCommand,
  openCodeModeEnv,
} from "../src/commands.js";

/*
 * SKILLS FOR CODEX AND OPENCODE (0.813). A teammate's own skills, a tune column's one and the ones kept from GitHub
 * live in Locust's profile, where neither agent looks. MEASURED 2026-10-22, no model called: Codex 0.163.0's
 * app-server lists a skill from a folder named in `skills/extraRoots/set`; OpenCode 1.18.27 lists one from
 * `$OPENCODE_CONFIG_DIR/skills`. So a run is handed its folder one of those two ways, and nothing of the person's is
 * written.
 */

const EXECUTABLE = { executablePath: "/usr/bin/codex", prefixArgs: [] as readonly string[] };
const OPENCODE = { executablePath: "/usr/bin/opencode", prefixArgs: [] as readonly string[] };

function fakeServer() {
  const written: Array<Record<string, unknown>> = [];
  let emit: (chunk: string) => void = () => undefined;
  const process: AppServerRunProcess = {
    write: (line) => {
      for (const part of line.split("\n")) if (part.trim().length > 0) written.push(JSON.parse(part) as Record<string, unknown>);
    },
    kill: () => undefined,
    onData: (listener) => {
      emit = listener;
    },
    onExit: () => undefined,
  };
  const idOf = (method: string): number | undefined => {
    const found = written.find((message) => message.method === method);
    return typeof found?.id === "number" ? found.id : undefined;
  };
  return {
    process,
    written,
    idOf,
    paramsOf: (method: string) => written.find((message) => message.method === method)?.params,
    answer: (id: number, result: unknown) => emit(`${JSON.stringify({ jsonrpc: "2.0", id, result })}\n`),
    refuse: (id: number) => emit(`${JSON.stringify({ jsonrpc: "2.0", id, error: { code: -32601, message: "Method not found" } })}\n`),
  };
}

const settle = async (): Promise<void> => {
  for (let turn = 0; turn < 8; turn += 1) await Promise.resolve();
};

const start = (server: ReturnType<typeof fakeServer>, skillRoots?: readonly string[]) =>
  startCodexAppServerRun({
    spawn: () => server.process,
    command: createCodexAppServerCommand(EXECUTABLE, { workspacePath: "/work", sandbox: "workspace-write" }),
    prompt: "use the skill",
    sandbox: "workspace-write",
    approvalPolicy: "never",
    ...(skillRoots === undefined ? {} : { skillRoots }),
  });

describe("a Codex run is handed its own skills", () => {
  it("names the run's skill folder after the handshake and before the thread starts", async () => {
    const server = fakeServer();
    start(server, ["/profile/run-skills/abc/skills"]);
    await settle();
    server.answer(server.idOf("initialize")!, { userAgent: "codex" });
    await settle();
    expect(server.paramsOf("skills/extraRoots/set")).toEqual({ extraRoots: ["/profile/run-skills/abc/skills"] });
    // The thread waits for the answer, so its first turn already lists them.
    expect(server.idOf("thread/start")).toBeUndefined();
    server.answer(server.idOf("skills/extraRoots/set")!, {});
    await settle();
    expect(server.idOf("thread/start")).toBeDefined();
    const order = server.written.map((message) => message.method);
    expect(order.indexOf("initialized")).toBeLessThan(order.indexOf("skills/extraRoots/set"));
  });

  it("runs the turn without them when the server does not know the request", async () => {
    const server = fakeServer();
    start(server, ["/profile/run-skills/abc/skills"]);
    await settle();
    server.answer(server.idOf("initialize")!, { userAgent: "codex" });
    await settle();
    server.refuse(server.idOf("skills/extraRoots/set")!);
    await settle();
    expect(server.idOf("thread/start")).toBeDefined();
  });

  it("says nothing about skills to a run that has none", async () => {
    const server = fakeServer();
    start(server);
    await settle();
    server.answer(server.idOf("initialize")!, { userAgent: "codex" });
    await settle();
    expect(server.idOf("skills/extraRoots/set")).toBeUndefined();
    expect(server.idOf("thread/start")).toBeDefined();
  });
});

describe("an OpenCode 1 run is handed its own skills", () => {
  const skillsOf = (env: Readonly<Record<string, string>> | undefined): unknown => (JSON.parse(env?.OPENCODE_CONFIG_CONTENT ?? "{}") as { skills?: unknown }).skills;

  it("in its per-run config's skills.paths, in every mode and on both routes, beside the rest of that config", () => {
    for (const sandbox of ["read-only", "workspace-write", "full-access"] as const) {
      const env = openCodeModeEnv({ sandbox, skillsDir: "/profile/run-skills/abc/skills" });
      expect(skillsOf(env)).toEqual({ paths: ["/profile/run-skills/abc/skills"] });
      // The mode's own config is still there.
      expect((JSON.parse(env.OPENCODE_CONFIG_CONTENT!) as { permission?: unknown }).permission).toBeDefined();
    }
    const run = createOpenCodeRunCommand(OPENCODE, { workspacePath: "/work", sandbox: "workspace-write", prompt: "go", skillsDir: "/profile/run-skills/abc/skills" });
    expect(skillsOf(run.env)).toEqual({ paths: ["/profile/run-skills/abc/skills"] });
    // Approve each builds its own config; the folder still goes with it.
    const asking = createOpenCodeServeCommand(OPENCODE, { workspacePath: "/work", skillsDir: "/profile/run-skills/abc/skills" });
    expect(skillsOf(asking.env)).toEqual({ paths: ["/profile/run-skills/abc/skills"] });
  });

  it("never as a config folder: OpenCode would install its plugin package into it before every run", () => {
    expect(openCodeModeEnv({ sandbox: "workspace-write", skillsDir: "/profile/run-skills/abc/skills" })).not.toHaveProperty("OPENCODE_CONFIG_DIR");
    expect(createOpenCodeServeCommand(OPENCODE, { workspacePath: "/work", skillsDir: "/profile/run-skills/abc/skills" }).env).not.toHaveProperty("OPENCODE_CONFIG_DIR");
    expect(skillsOf(openCodeModeEnv({ sandbox: "workspace-write" }))).toBeUndefined();
  });
});
