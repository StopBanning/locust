import { randomBytes } from "node:crypto";

import type { AppServerRunProcess } from "./codex-app-server-run.js";
import { runtimeCommandsFrom } from "./claude-events.js";
import type { RuntimeCommandInfo } from "./claude-events.js";
import type { RuntimeCommandSpec } from "./types.js";
import { openCodeUsesAcp } from "./discovery.js";

/**
 * OPENCODE'S OWN COMMANDS, FOR THE `/` MENU (0.427).
 *
 * `opencode run` never lists them; its server does, at `GET /command`.
 * MEASURED 2026-09-28 on 1.18.27, in an empty folder: 21 records of
 * `{name, description, source, template, hints}` -- `init` and `review`
 * (source "command") and every skill on the machine (source "skill").
 *
 * So a server is started for as long as it takes to ask, in the person's
 * folder (a folder's own `.opencode/command` files are listed there), with a
 * random password like every other server Locust starts (opencode-serve-run),
 * and with plugins off (OPENCODE_PURE): listing commands is no reason to run
 * code a repository chose. The template is never read past this function --
 * it is the command's whole prompt, and the menu needs its name.
 */
export interface OpenCodeCommandsOptions {
  readonly spawn: (
    executablePath: string,
    args: readonly string[],
    env?: Readonly<Record<string, string>>,
    cwd?: string,
  ) => AppServerRunProcess;
  /** `createOpenCodeServeCommand`'s spec for the folder. */
  readonly command: RuntimeCommandSpec;
  /** V2 has no serve command. Its ACP command advertisement is separate. */
  readonly version?: { readonly version: string };
  readonly timeoutMs?: number;
  /** Test seam. */
  readonly fetch?: typeof fetch;
}

const LISTENING = /listening on (http:\/\/127\.0\.0\.1:\d+)/;

/** What goes after a command, from OpenCode's `hints`: `$ARGUMENTS` says only that it takes some. */
function hintOf(hints: unknown): string {
  if (!Array.isArray(hints)) return "";
  const named = hints.filter((hint): hint is string => typeof hint === "string" && hint !== "$ARGUMENTS");
  return named.length > 0 ? named.join(" ") : hints.includes("$ARGUMENTS") ? "[arguments]" : "";
}

export function readOpenCodeCommands(options: OpenCodeCommandsOptions): Promise<readonly RuntimeCommandInfo[]> {
  if (openCodeUsesAcp(options.version)) return Promise.resolve([]);
  const request = options.fetch ?? fetch;
  const password = randomBytes(24).toString("base64url");
  const auth = { authorization: `Basic ${Buffer.from(`opencode:${password}`).toString("base64")}` };
  return new Promise((resolve, reject) => {
    let settled = false;
    let stdout = "";
    const child = options.spawn(
      options.command.executablePath,
      options.command.args,
      { ...(options.command.env ?? {}), OPENCODE_SERVER_PASSWORD: password, OPENCODE_PURE: "1" },
      options.command.cwd,
    );
    const end = (error: Error | undefined, commands?: readonly RuntimeCommandInfo[]): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      try {
        child.kill();
      } catch {
        // Already gone.
      }
      if (error !== undefined) reject(error);
      else resolve(commands ?? []);
    };
    const timer = setTimeout(() => end(new Error("OpenCode's server did not answer in time.")), options.timeoutMs ?? 30_000);
    child.onExit(() => end(new Error("OpenCode's server exited before it listed its commands.")));
    child.onData((chunk) => {
      if (settled || LISTENING.test(stdout)) return;
      stdout = `${stdout}${chunk}`.slice(-4_000);
      const found = LISTENING.exec(stdout);
      if (found === null) return;
      const folder = options.command.cwd === undefined ? "" : `?directory=${encodeURIComponent(options.command.cwd)}`;
      void request(`${found[1]}/command${folder}`, { headers: auth })
        .then(async (response) => {
          if (!response.ok) throw new Error(`OpenCode's server answered ${String(response.status)} for its commands.`);
          const listed: unknown = await response.json();
          const rows = Array.isArray(listed)
            ? listed.map((entry: unknown) =>
                typeof entry === "object" && entry !== null
                  ? { name: (entry as Record<string, unknown>).name, description: (entry as Record<string, unknown>).description, argumentHint: hintOf((entry as Record<string, unknown>).hints) }
                  : entry)
            : [];
          end(undefined, runtimeCommandsFrom(rows));
        })
        .catch((error: unknown) => end(error instanceof Error ? error : new Error("OpenCode's server could not be reached.")));
    });
  });
}
