import { describe, expect, it } from "vitest";

import { discoverInstalledRuntimes } from "./discovery.js";
import type { RuntimeBinaryFacts, ProbeCommand } from "./index.js";

/**
 * Fable's probing review, #4. A CLI's `--version` and `--help` are functions
 * of the file on disk, and on Colin's machine each costs 0.5-1.6 s of a
 * sweep that runs at every launch. Readiness is not cached and never will
 * be: signing in changes nothing on disk, which is the whole reason that
 * probe exists.
 */

const launch = (path: string) => ({
  commandName: "x",
  discoveredPath: path,
  executablePath: path,
  prefixArgs: [] as string[],
  kind: "native" as const,
});

function harness(options: {
  /** Size and mtime per path; a path absent here cannot be stat'd. */
  files: Record<string, { size: number; mtimeMs: number }>;
  locate?: (commandName: string) => unknown;
  store?: Map<string, RuntimeBinaryFacts>;
}) {
  const calls: ProbeCommand[] = [];
  const store = options.store ?? new Map<string, RuntimeBinaryFacts>();
  const runner = {
    run: async (command: ProbeCommand) => {
      calls.push(command);
      return { stdout: "1.2.3 --resume --output-format stream-json", stderr: "", exitCode: 0 };
    },
  } as never;
  const locator = {
    find: async (commandName: string) => options.locate?.(commandName) ?? launch(`C:/bin/${commandName}.exe`),
  } as never;
  const statFile = async (path: string) => {
    const file = options.files[path];
    if (file === undefined) throw new Error(`no such file: ${path}`);
    return file;
  };
  const recall = {
    get: (fingerprint: string) => store.get(fingerprint),
    set: (fingerprint: string, facts: RuntimeBinaryFacts) => {
      store.set(fingerprint, facts);
    },
  };
  const sweep = () => discoverInstalledRuntimes({ runner, locator, staggerMs: 0, recall, statFile });
  const purposes = () => calls.map((command) => command.purpose);
  return { sweep, calls, purposes, store, reset: () => calls.splice(0, calls.length) };
}

/**
 * Every runtime's binary, all with the same size and time.
 *
 * By COMMAND NAME, which is not the runtime id: Cursor Agent's id is
 * `cursor` and its command is `cursor-agent`. The first draft of this
 * fixture keyed on the ids, so Cursor's file could not be stat'd, and the
 * test reported a cache miss that was its own fixture's doing.
 */
// Muse's command is `muse` and its launcher is `muse.cmd`; the locator is
// faked here, so the NAME is what has to match — the same trap this file's
// comment above describes for cursor-agent.
const COMMANDS = ["codex", "claude", "cursor-agent", "gemini", "opencode", "copilot", "muse", "kilo", "omniroute"];
const everyBinary = (mtimeMs: number): Record<string, { size: number; mtimeMs: number }> =>
  Object.fromEntries(COMMANDS.map((name) => [`C:/bin/${name}.exe`, { size: 1000, mtimeMs }]));

describe("a binary that did not change is not asked twice", () => {
  it("asks everything the first time, and remembers what the files said", async () => {
    const { sweep, purposes, store } = harness({ files: everyBinary(10) });
    await sweep();
    expect(purposes()).toContain("version");
    expect(purposes()).toContain("capabilities");
    expect(store.size).toBeGreaterThan(0);
  });

  it("skips version and help on the second sweep, and still asks readiness", async () => {
    const files = everyBinary(10);
    const store = new Map<string, RuntimeBinaryFacts>();
    await harness({ files, store }).sweep();
    const second = harness({ files, store });
    await second.sweep();
    expect(second.purposes()).not.toContain("version");
    expect(second.purposes()).not.toContain("capabilities");
    // Signing in changes no file, so this one is always asked.
    expect(second.purposes()).toContain("readiness");
  });

  it("reports the same version from the remembered text as from the probe", async () => {
    const files = everyBinary(10);
    const store = new Map<string, RuntimeBinaryFacts>();
    const fresh = await harness({ files, store }).sweep();
    const remembered = await harness({ files, store }).sweep();
    expect(remembered.map((entry) => entry.version)).toEqual(fresh.map((entry) => entry.version));
    expect(remembered.map((entry) => entry.supportedFeatures)).toEqual(fresh.map((entry) => entry.supportedFeatures));
  });

  it("asks again when the file changed, which is what an update looks like", async () => {
    const store = new Map<string, RuntimeBinaryFacts>();
    await harness({ files: everyBinary(10), store }).sweep();
    const updated = harness({ files: everyBinary(20), store });
    await updated.sweep();
    expect(updated.purposes()).toContain("version");
    expect(updated.purposes()).toContain("capabilities");
  });

  it("asks again when the size changed but the time did not", async () => {
    const store = new Map<string, RuntimeBinaryFacts>();
    await harness({ files: everyBinary(10), store }).sweep();
    const files = Object.fromEntries(
      Object.entries(everyBinary(10)).map(([path, file]) => [path, { ...file, size: file.size + 1 }]),
    );
    const resized = harness({ files, store });
    await resized.sweep();
    expect(resized.purposes()).toContain("version");
  });

  it("fingerprints the script a shim runs, not only the shim", async () => {
    // An npm `.cmd` shim is run as `node <script>`: the file that changes on
    // an update is the script in the prefix arguments, and the shim beside
    // it may not move at all. Fingerprinting only the executable would serve
    // a stale version for ever.
    const shim = {
      commandName: "claude",
      discoveredPath: "C:/npm/claude.cmd",
      executablePath: "C:/node/node.exe",
      prefixArgs: ["C:/npm/node_modules/claude/cli.js"],
      kind: "node-shim" as const,
    };
    const files = (scriptMtime: number) => ({
      "C:/npm/claude.cmd": { size: 300, mtimeMs: 1 },
      "C:/node/node.exe": { size: 90_000, mtimeMs: 1 },
      "C:/npm/node_modules/claude/cli.js": { size: 5_000, mtimeMs: scriptMtime },
    });
    const store = new Map<string, RuntimeBinaryFacts>();
    const locate = (commandName: string) => (commandName === "claude" ? shim : undefined);
    await harness({ files: files(1), store, locate }).sweep();
    const updated = harness({ files: files(2), store, locate });
    await updated.sweep();
    expect(updated.purposes()).toContain("version");
  });

  it("probes rather than guesses when nothing about the file can be read", async () => {
    // No stat succeeds, so there is no fact to key on. Absence of a
    // fingerprint must never read as "unchanged".
    const store = new Map<string, RuntimeBinaryFacts>();
    await harness({ files: {}, store }).sweep();
    const second = harness({ files: {}, store });
    await second.sweep();
    expect(second.purposes()).toContain("version");
    expect(store.size).toBe(0);
  });

  it("remembers nothing from a probe that failed", async () => {
    // A CLI that timed out once under load must not be remembered as
    // versionless until somebody reinstalls it.
    const store = new Map<string, RuntimeBinaryFacts>();
    const failing = {
      run: async () => ({ stdout: "", stderr: "boom", exitCode: 1 }),
    } as never;
    await discoverInstalledRuntimes({
      runner: failing,
      locator: { find: async (name: string) => launch(`C:/bin/${name}.exe`) } as never,
      staggerMs: 0,
      recall: { get: (key) => store.get(key), set: (key, facts) => void store.set(key, facts) },
      statFile: async () => ({ size: 1000, mtimeMs: 10 }),
    });
    expect(store.size).toBe(0);
  });

  it("asks everything when no cache is given at all", async () => {
    const calls: ProbeCommand[] = [];
    await discoverInstalledRuntimes({
      runner: {
        run: async (command: ProbeCommand) => {
          calls.push(command);
          return { stdout: "1.2.3", stderr: "", exitCode: 0 };
        },
      } as never,
      locator: { find: async (name: string) => launch(`C:/bin/${name}.exe`) } as never,
      staggerMs: 0,
    });
    expect(calls.map((command) => command.purpose)).toContain("version");
  });
});
