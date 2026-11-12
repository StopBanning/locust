#!/usr/bin/env node
// The command grid: what Electron's own command builders would run for each agent, in each mode, with each option a
// turn can carry. The Rust engine's builders are checked against this file (core/src/runtime/command_grid_tests.rs).
//
//   node scripts/command-grid.mjs [--electron <checkout>] [--out parity/commands.json]
//   node scripts/command-grid.mjs --compare <old.json> [--electron <checkout>]   print what a newer Electron changed
//
// The checkout must have packages/runtime-adapters built (pnpm --filter @teammate/runtime-adapters build). Every path
// in a case is a placeholder, so the file holds nothing from this machine. Options follow how Electron's mission host
// calls each builder (apps/desktop/src/main/codex-mission.ts): Claude gets the permission bridge outside Auto and the
// command guard in every mode; Copilot and OpenCode get their guard plugins; connectors are off unless one is ticked.

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const flag = (name, fallback) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : fallback);
const electron = resolve(flag("--electron", process.env.LOCUST_ELECTRON_CHECKOUT ?? join(here, "..", "..", "..")));
const out = flag("--out", join(here, "..", "parity", "commands.json"));
const compare = flag("--compare", undefined);

const c = await import(pathToFileURL(join(electron, "packages/runtime-adapters/dist/commands.js")).href);

// Which modes Electron's app offers each agent (renderer/src/status.ts modeRunsOn, on a Mac): a case in a mode the
// app never offers is one no person can reach, and the Rust engine refusing it is the same outcome.
const modeRunsOn = await (async () => {
  const desktop = join(electron, "apps", "desktop");
  const { createRequire } = await import("node:module");
  const require = createRequire(join(desktop, "package.json"));
  const esbuild = createRequire(require.resolve("vite/package.json"))("esbuild");
  const { mkdtempSync } = await import("node:fs");
  const { tmpdir } = await import("node:os");
  const work = mkdtempSync(join(tmpdir(), "locust-grid-modes-"));
  writeFileSync(join(work, "entry.ts"), `export { modeRunsOn } from ${JSON.stringify(join(desktop, "src/renderer/src/status.ts"))}\n`);
  await esbuild.build({ entryPoints: [join(work, "entry.ts")], bundle: true, format: "esm", platform: "node", outfile: join(work, "entry.mjs"),
    nodePaths: [join(desktop, "node_modules"), join(electron, "node_modules")], absWorkingDir: electron, logLevel: "warning",
    loader: { ".css": "empty", ".png": "empty", ".svg": "empty" } });
  return (await import(pathToFileURL(join(work, "entry.mjs")).href)).modeRunsOn;
})();

const P = {
  workspace: "/work/project",
  reference: "/work/reference",
  bridge: "/locust/bridge/mcp.json",
  bridgeTool: "mcp__locust__approve",
  browser: "/locust/browser/mcp.json",
  browserCopilot: "/locust/browser/copilot-mcp.json",
  guardSettings: "/locust/guard/settings.json",
  guardPlugin: "/locust/guard",
  openCodeGuard: "file:///guard/locust-command-guard.mjs",
  skills: "/locust/skills",
};
const MODES = {
  ask: "read-only",
  plan: "read-only",
  "accept-edits": "workspace-write",
  "approve-each": "workspace-write",
  auto: "full-access",
};
const VARIANTS = ["base", "model", "effort", "resume", "fork", "browser", "no-guard", "skills", "references", "connectors", "everything"];
const launch = (name) => ({ commandName: name, discoveredPath: `/bin/${name}`, executablePath: `/bin/${name}`, prefixArgs: [] });
const browserMcp = { configPath: P.browser, copilotConfigPath: P.browserCopilot, command: "/locust/locust-native", args: ["--browser-bridge", "/locust/browser/host"], env: {} };

// One grid point, written so the Rust side can read it without knowing TypeScript.
function point(runtime, mode, variant) {
  const on = (v) => variant === v || variant === "everything";
  return {
    runtime,
    mode,
    sandbox: MODES[mode],
    model: on("model") ? "model-x" : null,
    // Cursor takes no effort; "everything" leaves it out there so the case says something (the "effort" case keeps
    // Electron's refusal).
    effort: variant === "effort" || (variant === "everything" && runtime !== "cursor") ? "high" : null,
    resume: on("resume") || on("fork") ? "session-1" : null,
    fork: on("fork"),
    browser: on("browser"),
    guard: variant !== "no-guard",
    skills: on("skills"),
    references: on("references"),
    connectors: on("connectors") ? ["Robinhood"] : [],
  };
}

function electronCommand(p) {
  const common = {
    workspacePath: P.workspace,
    sandbox: p.sandbox,
    ...(p.model === null ? {} : { model: p.model }),
    ...(p.effort === null ? {} : { effort: p.effort }),
    ...(p.resume === null ? {} : { resumeThreadId: p.resume }),
    ...(p.fork ? { forkSession: true } : {}),
    ...(p.references ? { referenceFolders: [P.reference] } : {}),
  };
  switch (p.runtime) {
    case "claude": {
      const askEach = p.mode === "approve-each";
      const rules = askEach || p.sandbox === "read-only" || p.connectors.length === 0 ? {} : { connectors: p.connectors };
      return c.createClaudePrintCommand(launch("claude"), {
        ...common,
        ...(p.sandbox !== "full-access" ? { permissionBridge: { configPath: P.bridge, toolName: P.bridgeTool } } : {}),
        ...(p.browser ? { browserMcp } : {}),
        ...(p.guard ? { commandGuard: { settingsPath: P.guardSettings } } : {}),
        ...(p.skills ? { skillPlugins: [P.skills] } : {}),
        ...rules,
        accountConnectors: p.connectors.length > 0,
        ...(askEach ? { askEach: true } : {}),
      });
    }
    case "codex":
      // The model, effort, resume and fork travel over app-server's protocol, not the command line.
      return c.createCodexAppServerCommand(launch("codex"), { workspacePath: P.workspace, sandbox: p.sandbox, cliVersion: "0.160.0" });
    case "cursor":
      return c.createCursorPrintCommand(launch("cursor-agent"), {
        ...common,
        platform: "darwin",
        ...(p.skills ? { skillPlugin: P.skills } : {}),
      });
    case "copilot":
      if (p.mode === "approve-each") {
        return c.createCopilotAcpCommand(launch("copilot"), {
          workspacePath: P.workspace,
          ...(p.model === null ? {} : { model: p.model }),
          ...(p.effort === null ? {} : { effort: p.effort }),
          ...(p.guard ? { guardPlugin: P.guardPlugin } : {}),
          ...(p.skills ? { skillPlugin: P.skills } : {}),
        });
      }
      return c.createCopilotPromptCommand(launch("copilot"), {
        ...common,
        prompt: "Say hi.",
        ...(p.resume === null ? { sessionId: "00000000-0000-4000-8000-000000000000" } : {}),
        ...(p.guard ? { guardPlugin: P.guardPlugin } : {}),
        ...(p.browser ? { browserMcp } : {}),
        ...(p.skills ? { skillPlugin: P.skills } : {}),
      });
    case "antigravity":
      return c.createAgyPrintCommand(launch("agy"), { ...common, ...(p.skills ? { skillWorkspace: P.skills } : {}) });
    case "muse":
      return c.createMuseExecCommand(launch("muse"), { ...common, prompt: "Say hi." });
    case "opencode":
      // OpenCode 1 through its local server, as Electron's host runs it; the model and session travel over HTTP.
      return c.createOpenCodeServeCommand(launch("opencode"), {
        workspacePath: P.workspace,
        ...(p.mode === "approve-each" ? {} : { sandbox: p.sandbox }),
        ...(p.references ? { referenceFolders: [P.reference] } : {}),
        ...(p.guard ? { openCodeGuardPlugin: P.openCodeGuard } : {}),
        ...(p.browser ? { browserMcp } : {}),
        ...(p.skills ? { skillsDir: P.skills } : {}),
      });
    default:
      throw new Error(`no builder for ${p.runtime}`);
  }
}

function grid() {
  const cases = [];
  for (const runtime of ["claude", "codex", "cursor", "copilot", "antigravity", "muse", "opencode"]) {
    for (const mode of Object.keys(MODES)) {
      for (const variant of VARIANTS) {
        const p = point(runtime, mode, variant);
        const id = `${runtime}/${mode}/${variant}`;
        const offered = modeRunsOn(mode, runtime, "darwin");
        try {
          const spec = electronCommand(p);
          const env = spec.env ?? {};
          cases.push({ id, ...p, offered, electron: { args: spec.args, stdin: spec.stdin, env: Object.fromEntries(Object.keys(env).sort().map((k) => [k, env[k]])) } });
        } catch (error) {
          cases.push({ id, ...p, offered, electron: { refused: String(error?.message ?? error) } });
        }
      }
    }
  }
  return cases;
}

const commit = execFileSync("git", ["-C", electron, "rev-parse", "--short=8", "HEAD"], { encoding: "utf8" }).trim();
const result = { electronCommit: commit, generatedBy: "scripts/command-grid.mjs", cases: grid() };

if (compare !== undefined) {
  const old = JSON.parse(readFileSync(compare, "utf8"));
  const before = new Map(old.cases.map((k) => [k.id, JSON.stringify(k.electron)]));
  let changed = 0;
  for (const k of result.cases) {
    const was = before.get(k.id);
    if (was === undefined) console.log(`new case: ${k.id}`);
    else if (was !== JSON.stringify(k.electron)) {
      changed += 1;
      console.log(`changed: ${k.id}\n  ${old.electronCommit}: ${was}\n  ${commit}: ${JSON.stringify(k.electron)}`);
    }
  }
  console.log(`${changed} of ${result.cases.length} cases changed between Electron ${old.electronCommit} and ${commit}.`);
} else {
  writeFileSync(out, JSON.stringify(result, null, 1) + "\n");
  const refused = result.cases.filter((k) => k.electron.refused !== undefined).length;
  console.log(`${result.cases.length} cases from Electron ${commit} (${refused} refused by Electron itself) -> ${out}`);
}
