import { spawnShape } from "./cmd-line.js";
import { windowsTaskkillPath } from "./process-runner.js";
import { spawn, spawnSync } from "node:child_process";
import { posix, win32 } from "node:path";
import type { CommandResult, CommandRunner, ProbeCommand } from "./types.js";

interface ProbeReadable {
  on(event: "data", listener: (chunk: Uint8Array | string) => void): unknown;
}

export interface SpawnedProbeProcess {
  readonly stdout: ProbeReadable;
  readonly stderr: ProbeReadable;
  once(event: "error", listener: (error: Error) => void): unknown;
  once(event: "close", listener: (exitCode: number | null) => void): unknown;
  kill(signal?: NodeJS.Signals): boolean;
  readonly pid?: number;
}

export type ProbeSpawn = (
  executablePath: string,
  args: readonly string[],
  options: {
    readonly shell: false;
    readonly windowsHide: true;
    readonly stdio: ["ignore", "pipe", "pipe"];
    /** Its own process group off Windows, so a timed-out tree can be signalled as one. */
    readonly detached?: boolean;
  },
) => SpawnedProbeProcess;

export interface NodeProbeRunnerOptions {
  readonly maxOutputBytes?: number;
  readonly maximumTimeoutMs?: number;
  readonly killGraceMs?: number;
  /** Test seam; production callers should leave this undefined. */
  readonly spawnProcess?: ProbeSpawn;
  /**
   * How to end everything a timed-out probe started, not only the process
   * itself. On Windows a CLI reached through a `.cmd` shim is cmd.exe with
   * the real program as its child, and `kill()` ends cmd.exe alone: the
   * program runs on. MEASURED 2026-09-18 with shims that ping for an hour --
   * five leaked per sweep, fifty running after two drives, holding the
   * folder they lived in. The default takes the tree down with taskkill on
   * Windows and does nothing extra elsewhere; a test hands in a spy.
   */
  readonly killTree?: (pid: number) => void;
  /** Test seam: which platform's rules to follow. Production leaves it to the process. */
  readonly platform?: NodeJS.Platform;
}

/**
 * A probe runner that can also be told the app is leaving. Every probe still
 * in flight is killed with its tree: a hung shim's sleep outlived Locust
 * otherwise -- Fable's pass 1 counted ten on Linux after a close mid-sweep,
 * five shims and their five sleeps, each in its own session where nothing
 * that signals the app's own group can reach it.
 */
export interface ProbeRunner extends CommandRunner {
  dispose(): void;
}

const defaultKillTree = (pid: number, platform: NodeJS.Platform): void => {
  if (platform !== "win32") {
    // The probe is spawned as its own process group below, so the negative
    // pid reaches the shim AND what it started. Grok's pass 12, Linux: 41
    // sleeping shims after Locust exited, because this returned early.
    try {
      process.kill(-pid, "SIGKILL");
    } catch {
      // Already gone, or not a group leader: the signals below still apply.
    }
    return;
  }
  try {
    // Synchronous on purpose. The first version started taskkill and sent
    // SIGTERM in the same tick; cmd.exe died before taskkill had walked its
    // tree, the ping was orphaned, and `/T` could no longer find it. Measured
    // with the runner alone: "after 1", and the calling process then hung on
    // the orphan's open pipe. The walk takes tens of milliseconds on a path
    // that has already waited five seconds.
    // By absolute path, as the process runner does: a bare name is looked
    // up in the working folder first, where a planted taskkill.exe would run.
    spawnSync(windowsTaskkillPath(), ["/PID", String(pid), "/T", "/F"], { stdio: "ignore", windowsHide: true });
  } catch {
    // The process itself still gets the signals below.
  }
};

/**
 * Ends a spawned child and everything it started: its process group off
 * Windows (spawn it `detached` so it has one), `taskkill /T` on Windows. For
 * the host's own bounded probes (`npm --version`, `npm config get prefix`),
 * which are spawned through a shell and hang with it.
 */
export const killSpawnedTree = (pid: number, platform: NodeJS.Platform): void => defaultKillTree(pid, platform);

interface BoundedOutput {
  readonly chunks: Uint8Array[];
  keptBytes: number;
}

function appendBounded(output: BoundedOutput, chunk: Uint8Array | string, limit: number): void {
  const bytes = typeof chunk === "string" ? Buffer.from(chunk) : Buffer.from(chunk);
  const remaining = limit - output.keptBytes;
  if (remaining <= 0) return;
  const kept = bytes.subarray(0, remaining);
  output.chunks.push(kept);
  output.keptBytes += kept.byteLength;
}

function outputText(output: BoundedOutput): string {
  return Buffer.concat(output.chunks, output.keptBytes).toString("utf8");
}

function positiveInteger(value: number | undefined, fallback: number, label: string): number {
  const resolved = value ?? fallback;
  if (!Number.isSafeInteger(resolved) || resolved <= 0) {
    throw new Error(`${label} must be a positive integer`);
  }
  return resolved;
}

const defaultSpawn: ProbeSpawn = (executablePath, args, options) =>
  spawn(executablePath, args, options) as unknown as SpawnedProbeProcess;

/**
 * Executes bounded discovery probes only. It never invokes a shell, accepts an
 * environment override, or executes a mission prompt.
 */
export function createNodeProbeRunner(options: NodeProbeRunnerOptions = {}): ProbeRunner {
  const maxOutputBytes = positiveInteger(options.maxOutputBytes, 64 * 1024, "maxOutputBytes");
  const maximumTimeoutMs = positiveInteger(options.maximumTimeoutMs, 10_000, "maximumTimeoutMs");
  const killGraceMs = positiveInteger(options.killGraceMs, 500, "killGraceMs");
  const spawnProcess = options.spawnProcess ?? defaultSpawn;
  const platform = options.platform ?? process.platform;
  const killTree = options.killTree ?? ((pid: number) => defaultKillTree(pid, platform));
  // Every probe between spawn and close, so `dispose` can reach them. The
  // timer is otherwise the only caller of the kill, and there is no timer
  // once the app has gone.
  const live = new Set<SpawnedProbeProcess>();
  /*
   * ONE-WAY. Once disposed, this runner spawns nothing again.
   *
   * dispose() killed what was live and cleared the set, and run() would
   * happily spawn afterwards -- so a probe started AFTER the kill was never
   * reachable by anything, because dispose is called once, from before-quit.
   *
   * The window is real and is the bounded shutdown: the ledger flushes for up
   * to SHUTDOWN_DEADLINE_MS with the renderer still alive and its re-check
   * chains still firing, and the host still willing to sweep. Fable found it
   * by reading, pass 2: "one disposed = true flag and a reject in run() away
   * from impossible". With five hung CLIs a sweep landing in that window is
   * ten processes outliving the app from an ORDINARY quit, by the same route
   * as the SIGKILL case that cannot be fixed.
   */
  let disposed = false;

  return {
    dispose(): void {
      disposed = true;
      for (const child of live) {
        if (typeof child.pid === "number") killTree(child.pid);
        try {
          child.kill("SIGKILL");
        } catch {
          // Already gone.
        }
      }
      live.clear();
    },
    run(command: ProbeCommand): Promise<CommandResult> {
      // Refused rather than ignored: a caller that asked after the app began
      // quitting gets an error it can see, not a promise that never settles.
      if (disposed) {
        return Promise.reject(new Error("Probe runner was disposed"));
      }
      // Absolute by the platform's own rules, as the spawn below is shaped for it.
      if (!(platform === "win32" ? win32 : posix).isAbsolute(command.executablePath)) {
        return Promise.reject(new Error("Probe executable path must be absolute"));
      }
      if (command.args.some((argument) => argument.includes("\0"))) {
        return Promise.reject(new Error("Probe arguments cannot contain NUL bytes"));
      }

      const timeoutMs = Math.min(
        positiveInteger(command.timeoutMs, maximumTimeoutMs, "timeoutMs"),
        maximumTimeoutMs,
      );

      return new Promise((resolve, reject) => {
        const stdout: BoundedOutput = { chunks: [], keptBytes: 0 };
        const stderr: BoundedOutput = { chunks: [], keptBytes: 0 };
        let timedOut = false;
        let settled = false;
        let forceTimer: ReturnType<typeof setTimeout> | undefined;

        // H8: a .cmd launcher gets the command line cmd.exe reads correctly.
        let shape: ReturnType<typeof spawnShape>;
        try {
          shape = spawnShape(command.executablePath, command.args);
        } catch (error) {
          reject(error instanceof Error ? error : new Error(String(error)));
          return;
        }
        const child = spawnProcess(command.executablePath, shape.args, {
          ...(shape.windowsVerbatimArguments === true ? { windowsVerbatimArguments: true } : {}),
          shell: false,
          windowsHide: true,
          stdio: ["ignore", "pipe", "pipe"],
          // H7: the launch's own environment over the host's, when it has one.
          ...(command.env === undefined ? {} : { env: { ...process.env, ...command.env } }),
          // A group of its own off Windows: kill(-pid) then ends the shim
          // and whatever it started. On Windows a detached child gets its
          // own console, which is not wanted; taskkill /T walks the tree.
          ...(platform === "win32" ? {} : { detached: true }),
        });

        live.add(child);
        const finish = (exitCode: number | null): void => {
          if (settled) return;
          settled = true;
          live.delete(child);
          clearTimeout(timeoutTimer);
          if (forceTimer !== undefined) clearTimeout(forceTimer);
          const result: CommandResult = {
            exitCode,
            stdout: outputText(stdout),
            stderr: outputText(stderr),
          };
          resolve(timedOut ? { ...result, timedOut: true } : result);
        };

        const timeoutTimer = setTimeout(() => {
          timedOut = true;
          // The tree first: what the shim started is the thing that hangs.
          if (typeof child.pid === "number") killTree(child.pid);
          child.kill("SIGTERM");
          forceTimer = setTimeout(() => {
            child.kill("SIGKILL");
            finish(null);
          }, killGraceMs);
        }, timeoutMs);

        child.stdout.on("data", (chunk) => appendBounded(stdout, chunk, maxOutputBytes));
        child.stderr.on("data", (chunk) => appendBounded(stderr, chunk, maxOutputBytes));
        child.once("error", (error) => {
          if (settled) return;
          settled = true;
          live.delete(child);
          clearTimeout(timeoutTimer);
          if (forceTimer !== undefined) clearTimeout(forceTimer);
          reject(error);
        });
        child.once("close", finish);
      });
    },
  };
}
