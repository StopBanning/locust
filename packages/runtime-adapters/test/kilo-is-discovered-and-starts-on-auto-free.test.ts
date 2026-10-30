import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { afterAll, describe, expect, it, vi } from "vitest";

import { discoverInstalledRuntimes } from "../src/discovery.js";
import { KILO_AUTO_FREE_MODEL, createKiloAcpCommand, kiloModelForRun, kiloSessionMode, resolveGitOnPath, startKiloAcpRun, withKiloGitShim } from "../src/kilo-acp-run.js";
import type { CommandResult, CommandRunner, ExecutableLaunch, ExecutableLocator, ProbeCommand } from "../src/types.js";

const ACP_HELP = "kilo acp: Agent Client Protocol over stdin and stdout";
const installed: ExecutableLaunch = {
  commandName: "kilo",
  discoveredPath: "C:\\tools\\kilo.exe",
  executablePath: "C:\\tools\\kilo.exe",
  prefixArgs: [],
  kind: "native",
};

const ok = (stdout: string): CommandResult => ({ exitCode: 0, stdout, stderr: "" });

async function discover(version: string, help: CommandResult = ok(ACP_HELP)) {
  const calls: ProbeCommand[] = [];
  const runner: CommandRunner = {
    run: async (command) => {
      calls.push(command);
      if (command.purpose === "version") return ok(version);
      if (command.args.includes("acp")) return help;
      return ok("");
    },
  };
  const locator: ExecutableLocator = { find: async (command) => (command === "kilo" ? installed : undefined) };
  const [found] = await discoverInstalledRuntimes({ runner, locator, only: new Set(["kilo"]) });
  return { found, calls };
}

describe("Kilo is discovered from its ACP command", () => {
  it("is ready from the version and acp help, and does not list models or look for a credential file", async () => {
    const { found, calls } = await discover("7.8.17");
    expect(found).toMatchObject({
      id: "kilo",
      availability: "available",
      readiness: "ready",
      version: { major: 7, minor: 8, patch: 17 },
      supportedFeatures: ["acp-stdio"],
      requiredFeatures: ["acp-stdio"],
      diagnostics: [],
    });
    expect(found?.modelHints).toBeUndefined();
    expect(calls.map((command) => command.args)).toEqual([["--version"], ["acp", "--help"]]);
    expect(calls.some((command) => command.purpose === "models" || command.purpose === "readiness")).toBe(false);
  });

  it("is not ready when acp help does not name the protocol", async () => {
    const { found } = await discover("7.8.17", ok("usage: kilo acp"));
    expect(found?.readiness).toBe("unsupported");
    expect(found?.supportedFeatures).toEqual([]);
  });

  it("is not installed when the command is absent", async () => {
    const runner: CommandRunner = { run: async () => ok("7.8.17") };
    const locator: ExecutableLocator = { find: async () => undefined };
    const [found] = await discoverInstalledRuntimes({ runner, locator, only: new Set(["kilo"]) });
    expect(found?.availability).toBe("unavailable");
    expect(found?.executable).toBeUndefined();
  });
});

describe("a new Kilo run starts on Auto Free", () => {
  it("never keeps the session default", () => {
    expect(kiloModelForRun(undefined)).toBe(KILO_AUTO_FREE_MODEL);
    expect(kiloModelForRun("account-default")).toBe(KILO_AUTO_FREE_MODEL);
    expect(kiloModelForRun("  ")).toBe(KILO_AUTO_FREE_MODEL);
    expect(kiloModelForRun("kilo/google/gemini-3-pro-image")).toBe("kilo/google/gemini-3-pro-image");
    expect(KILO_AUTO_FREE_MODEL).toBe("kilo/kilo-auto/free");
  });
});

describe("Kilo's modes", () => {
  it("maps Ask and Plan onto ask, and Edit and Auto onto code", () => {
    expect(kiloSessionMode("ask")).toBe("ask");
    expect(kiloSessionMode("plan")).toBe("ask");
    expect(kiloSessionMode("accept-edits")).toBe("code");
    expect(kiloSessionMode("auto")).toBe("code");
  });

  it("launches kilo acp and keeps a per-run environment on the command", () => {
    const command = createKiloAcpCommand(installed, "C:\\work\\scratch", { env: { KILO_CONFIG_DIR: "C:\\work\\scratch\\kilo-config" } });
    expect(command.runtime).toBe("kilo");
    expect(command.args).toEqual(["acp"]);
    expect(command.env?.KILO_CONFIG_DIR).toBe("C:\\work\\scratch\\kilo-config");
  });
});

describe("a Kilo child gets a git that cannot read the ACP pipe", () => {
  // Real files, so under the temp folder: a fixed C:\work\scratch path was a folder nobody cleaned on Windows,
  // and a folder of that name inside the package anywhere else.
  const root = mkdtempSync(join(tmpdir(), "locust-kilo-shim-unit-"));
  afterAll(() => rmSync(root, { recursive: true, force: true }));
  const pathKey = Object.keys(process.env).find((key) => key.toLowerCase() === "path") ?? "PATH";

  it("prefers git.exe over a git.cmd that appears earlier on PATH", () => {
    mkdirSync(join(root, "exe"), { recursive: true });
    mkdirSync(join(root, "cmd"), { recursive: true });
    writeFileSync(join(root, "exe", "git.exe"), "");
    writeFileSync(join(root, "cmd", "git.cmd"), "");
    expect(resolveGitOnPath(`${join(root, "cmd")};${join(root, "exe")}`, "win32")).toBe(join(root, "exe", "git.exe"));
    expect(resolveGitOnPath(join(root, "cmd"), "win32")).toBe(join(root, "cmd", "git.cmd"));
  });

  it("puts git.cmd first on the child PATH and leaves the process PATH alone", () => {
    const before = process.env[pathKey];
    const directory = join(root, "shim");
    const env = withKiloGitShim(
      { KILO_CONFIG_DIR: "C:\\work\\scratch\\kilo-config" },
      {
        platform: "win32",
        pathEnv: "C:\\Program Files\\Git\\cmd",
        pathKey: "Path",
        resolveGit: () => "C:\\Program Files\\Git\\cmd\\git.exe",
        directory,
      },
    );
    expect(env.KILO_CONFIG_DIR).toBe("C:\\work\\scratch\\kilo-config");
    expect(env.Path).toBe(`${directory};C:\\Program Files\\Git\\cmd`);
    expect(readFileSync(join(directory, "git.cmd"), "utf8")).toBe("@echo off\r\n@\"C:\\Program Files\\Git\\cmd\\git.exe\" %* < NUL\r\n");
    expect(process.env[pathKey]).toBe(before);
  });

  it("does not write a shim whose path could be read as a command", () => {
    const env = withKiloGitShim(
      { KILO_CONFIG_DIR: "C:\\work\\scratch\\kilo-config" },
      {
        platform: "win32",
        pathEnv: "C:\\Git\\cmd",
        pathKey: "PATH",
        resolveGit: () => "C:\\Git\\cmd\\git&.exe",
        directory: join(root, "rejected"),
      },
    );
    expect(env).toEqual({ KILO_CONFIG_DIR: "C:\\work\\scratch\\kilo-config" });
  });

  it("on another platform redirects stdin from /dev/null", () => {
    const directory = join(root, "unix");
    const env = withKiloGitShim(undefined, {
      platform: "linux",
      pathEnv: "/usr/bin",
      pathKey: "PATH",
      resolveGit: () => "/usr/bin/git",
      directory,
    });
    expect(env.PATH).toBe(`${directory}:/usr/bin`);
    expect(readFileSync(join(directory, "git"), "utf8")).toBe("#!/bin/sh\nexec \"/usr/bin/git\" \"$@\" </dev/null\n");
  });

  it("does not write a blank line after the prompt", async () => {
    vi.useFakeTimers();
    try {
      const lines: string[] = [];
      let receive: (chunk: string) => void = () => undefined;
      let childPath = "";
      const before = process.env[pathKey];
      const run = startKiloAcpRun({
        executable: installed,
        workspacePath: "C:\\work\\scratch",
        prompt: "Reply with pebble.",
        retryDelayMs: 10,
        shutdownTimeoutMs: 20,
        spawn: (_executable, _args, env) => {
          childPath = env?.[pathKey] ?? "";
          let exited: () => void = () => undefined;
          return {
            write: (line) => {
              lines.push(line);
              if (!line.startsWith("{")) return;
              const message = JSON.parse(line) as { id?: number; method?: string };
              const result = message.method === "initialize"
                ? { protocolVersion: 1 }
                : message.method === "session/new"
                  ? { sessionId: "s1" }
                  : message.method === "session/prompt"
                    ? { stopReason: "end_turn" }
                    : {};
              queueMicrotask(() => receive(`${JSON.stringify({ jsonrpc: "2.0", id: message.id, result })}\n`));
            },
            kill: () => { exited(); },
            onData: (listener) => { receive = listener; },
            onExit: (listener) => { exited = listener; },
          };
        },
      });
      for (let step = 0; step < 30; step += 1) await Promise.resolve();
      expect(lines.some((line) => line.includes("session/prompt"))).toBe(true);
      await vi.advanceTimersByTimeAsync(750);
      for (let step = 0; step < 10; step += 1) await Promise.resolve();
      expect(lines.filter((line) => line === "\n")).toEqual([]);
      expect(process.env[pathKey]).toBe(before);
      if (process.platform === "win32" && resolveGitOnPath(before, "win32") !== undefined) {
        const shimDir = childPath.split(";")[0] ?? "";
        expect(readFileSync(join(shimDir, "git.cmd"), "utf8")).toContain("< NUL");
        expect(shimDir).not.toBe((before ?? "").split(";")[0]);
      }
      await expect(run.completion).resolves.toMatchObject({ exitCode: 0 });
    } finally {
      vi.useRealTimers();
    }
  });
});
