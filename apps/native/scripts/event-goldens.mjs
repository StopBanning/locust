#!/usr/bin/env node
// Event goldens: each recorded agent stream in Electron's runtime-adapters fixtures, read by Electron's own reader.
// The Rust readers are checked against these (core/src/runtime/event_goldens_tests.rs).
//
//   node scripts/event-goldens.mjs [--electron <checkout>] [--out parity/events.json]
//
// Each entry keeps the stream's lines, so the check needs nothing outside this folder. The fixtures are already in
// the public repository and hold no personal data. Codex app-server and ACP transcripts are two-way conversations,
// not a stream a reader takes line by line; they are left for a later round and listed as such.

import { execFileSync } from "node:child_process";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const argv = process.argv.slice(2);
const flag = (name, fallback) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : fallback);
const electron = resolve(flag("--electron", process.env.LOCUST_ELECTRON_CHECKOUT ?? join(here, "..", "..", "..")));
const out = flag("--out", join(here, "..", "parity", "events.json"));
const dist = join(electron, "packages/runtime-adapters/dist");
const load = async (name) => import(pathToFileURL(join(dist, name)).href);

const FIXED = new Date("2026-01-01T00:00:00.000Z");
const context = { runId: "run_1", now: () => FIXED };
const READERS = {
  claude: ["claude", (await load("claude-events.js")).createClaudeEventNormalizer],
  cursor: ["cursor", (await load("cursor-events.js")).createCursorEventNormalizer],
  copilot: ["copilot", (await load("copilot-events.js")).createCopilotEventNormalizer],
  "agy-2026-10-02": ["agy", (await load("agy-events.js")).createAgyEventNormalizer],
  antigravity: ["antigravity", (await load("antigravity-events.js")).createAntigravityEventNormalizer],
  muse: ["muse", (await load("muse-events.js")).createMuseEventNormalizer],
  opencode: ["opencode", (await load("opencode-events.js")).createOpenCodeEventNormalizer],
};
const LATER = { codex: "app-server transcript (two-way)", acp: "ACP transcript (two-way)" };

const fixtures = join(electron, "packages/runtime-adapters/test/fixtures");
const streams = [];
const later = [];
for (const dir of readdirSync(fixtures, { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name).sort()) {
  for (const file of readdirSync(join(fixtures, dir)).filter((f) => f.endsWith(".jsonl")).sort()) {
    if (LATER[dir] !== undefined || (dir === "opencode" && file === "serve-events.jsonl")) {
      later.push({ fixture: `${dir}/${file}`, why: LATER[dir] ?? "OpenCode server events (HTTP stream)" });
      continue;
    }
    const [reader, create] = READERS[dir];
    const lines = readFileSync(join(fixtures, dir, file), "utf8").split(/\r?\n/).filter((l) => l.trim() !== "");
    const normalizer = create({ ...context });
    const events = [];
    const keep = (list) => {
      for (const e of list) events.push({ type: e.type, payload: e.payload, ...(e.runtimeThreadId === undefined ? {} : { thread: e.runtimeThreadId }) });
    };
    try {
      lines.forEach((raw, i) => keep(normalizer.accept({ sequence: i + 1, raw })));
      if (typeof normalizer.finish === "function") {
        keep(normalizer.finish({ exitCode: 0, signal: null, stderr: "", stderrTruncated: false, recordCount: lines.length, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false }));
      }
      streams.push({ fixture: `${dir}/${file}`, reader, lines, events });
    } catch (error) {
      streams.push({ fixture: `${dir}/${file}`, reader, lines, refused: String(error?.message ?? error) });
    }
  }
}

const commit = execFileSync("git", ["-C", electron, "rev-parse", "--short=8", "HEAD"], { encoding: "utf8" }).trim();
writeFileSync(out, JSON.stringify({ electronCommit: commit, generatedBy: "scripts/event-goldens.mjs", now: FIXED.toISOString(), streams, later }, null, 1) + "\n");
console.log(`${streams.length} streams read by Electron ${commit}; ${later.length} left for later -> ${out}`);
