import { describe, expect, it } from "vitest";

import { discoverInstalledRuntimes } from "../src/discovery.js";
import { KILO_AUTO_FREE_MODEL, createKiloAcpCommand, kiloModelForRun, kiloSessionMode } from "../src/kilo-acp-run.js";
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
