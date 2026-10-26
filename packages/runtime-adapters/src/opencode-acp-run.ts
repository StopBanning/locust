import { startAcpRun } from "./acp-run.js";
import type { AcpRun, AcpRunOptions } from "./acp-run.js";
import { createAcpAgentCommand } from "./commands.js";
import { NotificationQueue } from "./codex-app-server-run.js";
import type { AppServerRunProcess } from "./codex-app-server-run.js";
import type { RuntimeJsonlRecord, RuntimeProcessCompletion } from "./process-runner.js";
import type { ExecutableLaunch, MissionSandbox, RuntimeCommandSpec } from "./types.js";

/** OpenCode 2's ACP profile; OpenCode 1 keeps its existing HTTP/SSE route. */
export const OPENCODE_ACP_PROFILE = Object.freeze({
  runtime: "opencode",
  transport: "stdio",
  protocolVersion: 1,
} as const);

/** OpenCode 2's ACP CLI uses the same command name, not an HTTP socket. */
export function createOpenCodeAcpCommand(executable: ExecutableLaunch, workspacePath: string, launch?: { readonly env?: Readonly<Record<string, string>>; readonly sandbox?: MissionSandbox }): RuntimeCommandSpec {
  const base = createAcpAgentCommand(executable, ["acp"], workspacePath);
  return {
    ...base,
    runtime: "opencode",
    ...(launch?.sandbox === undefined ? {} : { sandbox: launch.sandbox }),
    ...(launch?.env === undefined ? {} : { env: { ...base.env, ...launch.env } }),
  };
}

export interface OpenCodeAcpRunOptions extends Omit<AcpRunOptions, "command"> {
  readonly executable: ExecutableLaunch;
  readonly workspacePath: string;
  /** Per-run host configuration, including the permission mode and command guard. */
  readonly launch?: { readonly env?: Readonly<Record<string, string>>; readonly sandbox?: MissionSandbox };
  /**
   * A command already built for this run. OpenCode builds its own; Kilo passes
   * `kilo acp` so this handshake is shared without pointing Kilo at OpenCode's
   * `locust` mode or OPENCODE_* configuration.
   */
  readonly command?: RuntimeCommandSpec;
  /** Named in a failure. OpenCode's own runs say OpenCode. */
  readonly label?: string;
  /** Transport restarts before any prompt write. Capped at three. */
  readonly maxStartupRetries?: number;
  readonly retryDelayMs?: number;
  readonly maxRetryDelayMs?: number;
  /** Bound waiting for the owned process's exit after its tree was stopped. */
  readonly shutdownTimeoutMs?: number;
}

export interface OpenCodeAcpFailure {
  readonly code: "transport-closed" | "startup-timeout" | "agent-error" | "process-start-failed" | "output-limit" | "termination-unconfirmed";
  readonly stage: "startup" | "prompt";
  readonly attempts: number;
  readonly message: string;
}

export type OpenCodeAcpRun = AcpRun & { readonly failure: Promise<OpenCodeAcpFailure | undefined> };

function bound(value: number | undefined, fallback: number, maximum: number, minimum = 1): number {
  return value !== undefined && Number.isFinite(value) ? Math.min(maximum, Math.max(minimum, Math.floor(value))) : fallback;
}

/**
 * Reuse ACP's handshake, permissions, session replay suppression and events.
 * OpenCode's ACP endpoint is a child process, so reconnect means restarting
 * that transport. Once a prompt write is attempted its delivery is uncertain:
 * neither that prompt nor any of its tool calls can safely be replayed.
 */
export function startOpenCodeAcpRun(options: OpenCodeAcpRunOptions): OpenCodeAcpRun {
  const command = options.command ?? createOpenCodeAcpCommand(options.executable, options.workspacePath, options.launch);
  const who = options.label ?? "OpenCode";
  const now = options.now ?? (() => new Date());
  const startedAt = now().toISOString();
  const queueLimit = bound(options.maxQueuedRecords, 20_000, 20_000);
  const records = new NotificationQueue(queueLimit);
  const maxRetries = bound(options.maxStartupRetries, 2, 3, 0);
  const retryDelay = bound(options.retryDelayMs, 250, 10_000);
  const maxRetryDelay = bound(options.maxRetryDelayMs, 1_000, 10_000);
  const shutdownTimeout = bound(options.shutdownTimeoutMs, 1_000, 10_000);
  const startupBudget = bound(options.handshakeTimeoutMs, 60_000, 60_000);
  const stopController = new AbortController();
  let timedOut = false;
  let admitted = false;
  let settled = false;
  let count = 0;
  let overflowed = false;
  let attempts = 0;
  let current: AcpRun | undefined;
  let stopOwned: () => void = () => undefined;
  let confirmOwned: () => Promise<boolean> = async () => true;
  const queued: string[] = [];
  let settleCompletion!: (receipt: RuntimeProcessCompletion) => void;
  const completion = new Promise<RuntimeProcessCompletion>((resolve) => { settleCompletion = resolve; });
  let settleFailure!: (failure: OpenCodeAcpFailure | undefined) => void;
  const failure = new Promise<OpenCodeAcpFailure | undefined>((resolve) => { settleFailure = resolve; });
  const stop = (): void => { stopController.abort(); };
  if (options.signal?.aborted === true) stop();
  else options.signal?.addEventListener("abort", stop, { once: true });
  const startupTimer = setTimeout(() => {
    timedOut = true;
    stop();
  }, startupBudget);

  const wait = (ms: number): Promise<void> => new Promise((resolve) => {
    if (stopController.signal.aborted) { resolve(); return; }
    const done = (): void => {
      clearTimeout(timer);
      stopController.signal.removeEventListener("abort", done);
      resolve();
    };
    const timer = setTimeout(done, ms);
    stopController.signal.addEventListener("abort", done, { once: true });
  });
  const emit = (record: RuntimeJsonlRecord): void => {
    count += 1;
    records.push({ sequence: count, raw: record.raw });
    if (records.droppedForOverflow) { overflowed = true; stop(); }
  };
  const flushSteering = (): void => {
    for (const text of queued.splice(0)) void current?.steer(text);
  };
  const finish = (receipt: RuntimeProcessCompletion | undefined, code?: OpenCodeAcpFailure["code"], message?: string, terminationUnconfirmed = false): void => {
    settled = true;
    clearTimeout(startupTimer);
    options.signal?.removeEventListener("abort", stop);
    const cancelled = options.signal?.aborted === true;
    const why = message ?? receipt?.stderr ?? "";
    const result = code === undefined || cancelled ? undefined : { code, stage: admitted ? "prompt" as const : "startup" as const, attempts, message: why };
    settleFailure(result);
    records.end();
    settleCompletion({
      exitCode: cancelled || code !== undefined ? null : receipt?.exitCode ?? null,
      signal: receipt?.signal ?? null,
      stderr: why,
      stderrTruncated: receipt?.stderrTruncated ?? false,
      recordCount: count,
      cancelled,
      forcedTerminationAttempted: receipt?.forcedTerminationAttempted ?? false,
      terminationUnconfirmed: receipt?.terminationUnconfirmed ?? terminationUnconfirmed,
      inputDeliveryFailed: receipt?.inputDeliveryFailed ?? false,
      outputLimitExceeded: overflowed || (receipt?.outputLimitExceeded ?? false),
      oversizedRecordsDropped: receipt?.oversizedRecordsDropped ?? 0,
      startedAt,
      finishedAt: now().toISOString(),
    });
  };

  void (async () => {
    while (!stopController.signal.aborted) {
      attempts += 1;
      let transportClosed = false;
      let exited = false;
      let stopping = false;
      let process: AppServerRunProcess | undefined;
      let exitListener: (() => void) | undefined;
      let closeListener: ((error?: Error) => void) | undefined;
      let markExited!: () => void;
      const exit = new Promise<void>((resolve) => { markExited = resolve; });
      const buffered: RuntimeJsonlRecord[] = [];
      confirmOwned = async () => {
        if (process === undefined || exited) return true;
        let timer: ReturnType<typeof setTimeout> | undefined;
        await Promise.race([exit, new Promise<void>((resolve) => { timer = setTimeout(resolve, shutdownTimeout); })]);
        clearTimeout(timer);
        return exited;
      };
      try {
        current = startAcpRun({
          ...options,
          command,
          signal: stopController.signal,
          handshakeTimeoutMs: startupBudget,
          cancelGraceMs: bound(options.cancelGraceMs, 3_000, 10_000),
          maxQueuedRecords: queueLimit,
          strictConfig: true,
          spawn: (executablePath, args, env, cwd) => {
            process = options.spawn(executablePath, args, env, cwd);
            const child = process;
            stopOwned = () => {
              if (stopping) return;
              stopping = true;
              child.kill();
            };
            process.onExit(() => {
              if (exited) return;
              exited = true;
              if (!stopping) transportClosed = true;
              markExited();
              exitListener?.();
            });
            return {
              write: (line) => {
                const message = JSON.parse(line) as { method?: string };
                // Set BEFORE write: an exception may follow partial delivery.
                if (message.method === "session/prompt") {
                  admitted = true;
                  clearTimeout(startupTimer);
                  queueMicrotask(flushSteering);
                  // GIT INHERITS ACP STDIN ON WINDOWS (2026-10-26). Kilo 7.8.17 runs
                  // `git config --get remote.origin.url` through Bun's shell on every
                  // gateway request, and that child inherits this pipe. On Windows it
                  // blocks until another line arrives, so session/prompt never returns
                  // and the client is never asked for a file or a permission. A blank
                  // line is not a protocol message. Measured on 7.8.17: a blank line
                  // 500ms after the prompt, and the model is selected about 15ms later.
                  // 750ms leaves room for a slower prepare. OpenCode does not do this
                  // lookup. The local `process` is the child, so the platform is read
                  // from Node itself.
                  if (command.runtime === "kilo" && globalThis.process.platform === "win32") {
                    const release = setTimeout(() => {
                      try { child.write("\n"); } catch { /* The pipe already closed. */ }
                    }, 750);
                    if (typeof release === "object" && release !== null && "unref" in release) release.unref();
                  }
                }
                try {
                  child.write(line);
                } catch (error) {
                  transportClosed = true;
                  // A failed permission reply must not reject the RPC client's
                  // background response handler. Settle the run via the same
                  // transport-loss path; never try to send that reply again.
                  if (message.method === undefined) {
                    queueMicrotask(() => {
                      if (closeListener !== undefined) closeListener(new Error("The ACP response pipe failed."));
                      else exitListener?.();
                    });
                    return;
                  }
                  throw error;
                }
              },
              kill: stopOwned,
              onData: (listener) => child.onData(listener),
              onExit: (listener) => {
                exitListener = listener;
                if (exited) queueMicrotask(listener);
              },
              ...(child.onStdoutClose === undefined ? {} : {
                onStdoutClose: (listener: (error?: Error) => void) => {
                  closeListener = listener;
                  child.onStdoutClose!((error) => {
                    if (!stopping) transportClosed = true;
                    listener(error);
                  });
                },
              }),
              ...(child.stderrTail === undefined ? {} : { stderrTail: () => child.stderrTail!() }),
            };
          },
        });
      } catch {
        try { stopOwned(); } catch { /* Exit confirmation remains bounded. */ }
        const confirmed = await confirmOwned();
        finish(undefined, "process-start-failed", `${who}'s ACP process could not start. Check its installed command; no prompt was sent.`, !confirmed);
        return;
      }
      const relay = (async () => {
        for await (const record of current!.records) {
          if (!admitted) {
            if (buffered.length >= queueLimit) { overflowed = true; stop(); }
            else buffered.push(record);
          }
          else {
            for (const prior of buffered.splice(0)) emit(prior);
            emit(record);
          }
        }
      })();
      let receipt = await current.completion;
      await relay;
      // The native process bridge kills the whole process tree. An exit is
      // still required before replacing it; failure to confirm is reported.
      const confirmed = await confirmOwned();
      receipt = { ...receipt, terminationUnconfirmed: !confirmed };
      if (options.signal?.aborted === true) { finish(receipt); return; }
      if (overflowed || receipt.outputLimitExceeded) { finish(receipt, "output-limit", `${who} sent output faster than Locust could record it. The run was stopped and was not retried.`); return; }
      if (timedOut) { finish(receipt, "startup-timeout", `${who} did not complete ACP startup within the startup deadline. No prompt was resent.`); return; }
      if (!exited) { finish(receipt, "termination-unconfirmed", `${who}'s ACP process did not confirm that it exited after being stopped. No replacement process was started.`); return; }
      if (!admitted && transportClosed && attempts <= maxRetries) {
        await wait(Math.min(retryDelay * 2 ** (attempts - 1), maxRetryDelay));
        continue;
      }
      const code = receipt.exitCode === 0 ? undefined : transportClosed ? "transport-closed" : "agent-error";
      finish(receipt, code);
      return;
    }
    finish(undefined, timedOut ? "startup-timeout" : undefined, timedOut ? `${who}'s ACP startup deadline expired before a prompt was sent.` : undefined);
  })().catch(async () => {
    try { stopOwned(); } catch { /* Exit confirmation remains bounded. */ }
    const confirmed = await confirmOwned();
    finish(undefined, "agent-error", `${who}'s ACP run could not finish. No prompt was resent.`, !confirmed);
  });

  return {
    records,
    completion,
    failure,
    steer: async (text) => {
      if (settled || stopController.signal.aborted || text.trim().length === 0) return false;
      if (admitted && current !== undefined) return await current.steer(text);
      queued.push(text);
      return true;
    },
  };
}
