import { describe, expect, it } from "vitest";

import { discoverInstalledRuntimes, openCodeUsesAcp } from "../src/discovery.js";
import type { RuntimeBinaryFacts, RuntimeFactsCache } from "../src/discovery.js";
import type { CommandResult, CommandRunner, ExecutableLaunch, ExecutableLocator, ProbeCommand } from "../src/types.js";

const LEGACY_HELP = "opencode run [message..]  --format <format>  text | json  -m, --model  -s, --session";
const ACP_HELP = "opencode acp: start an AgentClientProtocol server over stdin and stdout";
const MODELS = "opencode/big-pickle\nopencode/deepseek-v4.1-flash-free\n";

const installed: ExecutableLaunch = {
  commandName: "opencode",
  discoveredPath: "C:\\tools\\opencode.exe",
  executablePath: "C:\\tools\\opencode.exe",
  prefixArgs: [],
  kind: "native",
};
const locator: ExecutableLocator = { find: async (command) => (command === "opencode" ? installed : undefined) };
const ok = (stdout: string): CommandResult => ({ exitCode: 0, stdout, stderr: "" });

async function discover(version: string, help: CommandResult = ok(ACP_HELP)) {
  const calls: ProbeCommand[] = [];
  const runner: CommandRunner = {
    run: async (command) => {
      calls.push(command);
      if (command.purpose === "version") return ok(version);
      if (command.args[0] === "acp") return help;
      return ok(command.purpose === "capabilities" ? LEGACY_HELP : MODELS);
    },
  };
  const [found] = await discoverInstalledRuntimes({ runner, locator, only: new Set(["opencode"]) });
  return { found, calls };
}

describe("OpenCode discovery chooses the supported version's protocol", () => {
  it("checks V2 ACP help without launching V1 probes or spending a turn", async () => {
    const { found, calls } = await discover("opencode v2.0.32");
    expect(found).toMatchObject({
      availability: "available", readiness: "ready", version: { major: 2 },
      supportedFeatures: ["acp-stdio"], requiredFeatures: ["acp-stdio"], diagnostics: [],
    });
    expect(calls.map((command) => ({ purpose: command.purpose, args: command.args }))).toEqual([
      { purpose: "version", args: ["--version"] },
      { purpose: "capabilities", args: ["acp", "--help"] },
    ]);
    expect(calls.every((command) => command.timeoutMs === 10_000)).toBe(true);
  });

  it("leaves V2 model choices to the ACP session instead of inventing CLI hints", async () => {
    const { found, calls } = await discover("2.1.0", ok(`${ACP_HELP}\n${MODELS}\n--effort low|high`));
    expect(found?.readiness).toBe("ready");
    expect(found?.modelHints).toBeUndefined();
    expect(calls.some((command) => command.purpose === "models" || command.purpose === "readiness")).toBe(false);
  });

  it("keeps the V1 help, model-list readiness and free-model parsing", async () => {
    const { found, calls } = await discover("1.19.3");
    expect(found).toMatchObject({ availability: "available", readiness: "ready", version: { major: 1 } });
    expect(found?.supportedFeatures).not.toContain("acp-stdio");
    expect(found?.requiredFeatures).toEqual(["non-interactive", "jsonl-events"]);
    expect(calls.map((command) => command.args)).toEqual([["--version"], ["models", "--verbose"], ["run", "--help"]]);
    expect(found?.modelHints?.models).toEqual([
      { id: "opencode/big-pickle", displayName: "big-pickle" },
      { id: "opencode/deepseek-v4.1-flash-free", displayName: "deepseek-v4.1-flash-free", description: "free" },
    ]);
    expect(found?.diagnostics).toEqual([]);
  });

  it("refuses future major versions without guessing either protocol", async () => {
    const { found, calls } = await discover("3.0.0");
    expect(found?.readiness).toBe("unsupported");
    expect(found?.diagnostics).toEqual([expect.objectContaining({ code: "version-unsupported", message: expect.stringContaining("3.0.0") })]);
    expect(calls.map((command) => command.args)).toEqual([["--version"]]);
  });

  it("does not promote an unrecognized version into ACP", async () => {
    const { found, calls } = await discover("opencode nightly");
    expect(found?.version).toBeUndefined();
    expect(found?.diagnostics.map((entry) => entry.code)).toContain("version-unrecognized");
    expect(calls.map((command) => command.args)).not.toContainEqual(["acp", "--help"]);
    expect(found?.supportedFeatures).not.toContain("acp-stdio");
  });

  it.each([
    ["old run help", ok(LEGACY_HELP)],
    ["nonzero exit despite an ACP label", { exitCode: 1, stdout: ACP_HELP, stderr: "unknown command" }],
    ["timed out help", { ...ok(ACP_HELP), timedOut: true }],
  ] as const)("refuses V2 without safe ACP capability evidence: %s", async (_reason, help) => {
    const { found } = await discover("2.0.32", help);
    expect(found?.readiness).toBe("unsupported");
    expect(found?.supportedFeatures).toEqual([]);
    expect(found?.diagnostics.map((entry) => entry.code)).toContain("required-capability-missing");
  });

  it("accepts ACP help printed on stderr with the spaced protocol name", async () => {
    const { found } = await discover("2.0.32", { exitCode: 0, stdout: "", stderr: "Agent Client Protocol over stdio" });
    expect(found?.readiness).toBe("ready");
  });

  it("contains a failed ACP help runner without starting a prompt", async () => {
    const calls: ProbeCommand[] = [];
    const runner: CommandRunner = {
      run: async (command) => {
        calls.push(command);
        if (command.purpose === "version") return ok("2.0.32");
        throw new Error("process could not start");
      },
    };
    const [found] = await discoverInstalledRuntimes({ runner, locator, only: new Set(["opencode"]) });
    expect(found?.readiness).toBe("unsupported");
    expect(found?.diagnostics).toContainEqual(expect.objectContaining({ code: "capability-probe-failed", message: expect.stringContaining("process could not start") }));
    expect(calls).toHaveLength(2);
  });

  it("ignores legacy cached V2 help and never saves ACP help under a V1 cache key", async () => {
    const writes: RuntimeBinaryFacts[] = [];
    const recall: RuntimeFactsCache = {
      get: () => ({ versionText: "2.0.32", capabilityText: LEGACY_HELP }),
      set: (_fingerprint, facts) => { writes.push(facts); },
    };
    const calls: ProbeCommand[] = [];
    const runner: CommandRunner = { run: async (command) => { calls.push(command); return ok(ACP_HELP); } };
    const [found] = await discoverInstalledRuntimes({ runner, locator, only: new Set(["opencode"]), recall, statFile: async () => ({ size: 10, mtimeMs: 1 }) });
    expect(found?.readiness).toBe("ready");
    expect(calls.map((command) => command.args)).toEqual([["acp", "--help"]]);
    expect(writes).toEqual([]);
  });

  it("recognizes a changed binary upgraded from V1 to V2 without reusing V1 capabilities", async () => {
    const facts = new Map<string, RuntimeBinaryFacts>();
    const recall: RuntimeFactsCache = { get: (key) => facts.get(key), set: (key, value) => { facts.set(key, value); } };
    let version = "1.19.3";
    let stamp = 1;
    const calls: ProbeCommand[] = [];
    const runner: CommandRunner = {
      run: async (command) => {
        calls.push(command);
        if (command.purpose === "version") return ok(version);
        if (command.args[0] === "acp") return ok(ACP_HELP);
        return ok(command.purpose === "capabilities" ? LEGACY_HELP : MODELS);
      },
    };
    const options = { runner, locator, only: new Set(["opencode"] as const), recall, statFile: async () => ({ size: 10, mtimeMs: stamp }) };
    expect((await discoverInstalledRuntimes(options))[0]?.readiness).toBe("ready");
    expect(facts.size).toBe(1);
    calls.length = 0;
    version = "2.0.32";
    stamp = 2;
    const [found] = await discoverInstalledRuntimes(options);
    expect(found?.supportedFeatures).toEqual(["acp-stdio"]);
    expect(found?.modelHints).toBeUndefined();
    expect(calls.map((command) => command.args)).toEqual([["--version"], ["acp", "--help"]]);
    expect(facts.size).toBe(1);
  });

  it("uses the executable's launch prefix and environment for the ACP help check", async () => {
    const launch: ExecutableLaunch = { ...installed, prefixArgs: ["opencode-cli.js"], env: { ELECTRON_RUN_AS_NODE: "1" } };
    const calls: ProbeCommand[] = [];
    const runner: CommandRunner = { run: async (command) => { calls.push(command); return ok(command.purpose === "version" ? "2.0.32" : ACP_HELP); } };
    const [found] = await discoverInstalledRuntimes({ runner, locator: { find: async () => launch }, only: new Set(["opencode"]) });
    expect(found?.readiness).toBe("ready");
    expect(calls[1]).toMatchObject({ args: ["opencode-cli.js", "acp", "--help"], env: { ELECTRON_RUN_AS_NODE: "1" }, timeoutMs: 10_000 });
  });

  it("shares one exact-major selector without assigning ACP to unknown, V1 or future versions", () => {
    expect(openCodeUsesAcp({ version: "2.0.32" })).toBe(true);
    expect(openCodeUsesAcp({ version: "2.1.0-rc.1" })).toBe(true);
    expect(openCodeUsesAcp(undefined)).toBe(false);
    expect(openCodeUsesAcp({ version: "unrecognized" })).toBe(false);
    expect(openCodeUsesAcp({ version: "1.19.3" })).toBe(false);
    expect(openCodeUsesAcp({ version: "3.0.0" })).toBe(false);
  });
});
