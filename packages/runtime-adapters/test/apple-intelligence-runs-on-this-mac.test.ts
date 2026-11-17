import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  APPLE_MODEL,
  APPLE_RUNTIME,
  appleBrief,
  appleDiscoveryRecord,
  appleFailureWords,
  appleRefusesSandbox,
  appleRequestLine,
  appleUnavailableWords,
  createAppleEventNormalizer,
  createAppleModelCommand,
  readAppleProbe,
} from "../src/apple-model.js";
import type { NormalizedRuntimeEvent } from "../src/codex-events.js";
import { createNodeRuntimeProcessRunner, type RuntimeProcessCompletion } from "../src/process-runner.js";
import type { ExecutableLaunch } from "../src/types.js";

/*
 * APPLE INTELLIGENCE, ON THIS MAC (0.878): the same cases the Rust app's apple_tests.rs holds, for the Electron app.
 * A synthetic helper (fixtures/apple/apple-model.py, the Rust app's own fixture) stands in for the Swift one; the last
 * test asks the real model and runs only with LOCUST_REAL_APPLE=1.
 */

const NOW = "2026-11-17T18:00:00.000Z";
const FAKE = fileURLToPath(new URL("./fixtures/apple/apple-model.py", import.meta.url));

function completion(overrides: Partial<RuntimeProcessCompletion> = {}): RuntimeProcessCompletion {
  return {
    exitCode: 0,
    signal: null,
    stderr: "",
    stderrTruncated: false,
    recordCount: 0,
    cancelled: false,
    forcedTerminationAttempted: false,
    terminationUnconfirmed: false,
    inputDeliveryFailed: false,
    outputLimitExceeded: false,
    oversizedRecordsDropped: 0,
    startedAt: NOW,
    finishedAt: NOW,
    ...overrides,
  };
}

const normalizer = () => createAppleEventNormalizer({ runId: "run_apple", missionId: "mission_apple", now: () => new Date(NOW) });

function lines(said: readonly unknown[], finish: Partial<RuntimeProcessCompletion> = {}): NormalizedRuntimeEvent[] {
  const apple = normalizer();
  const events: NormalizedRuntimeEvent[] = [];
  said.forEach((line, index) => events.push(...apple.accept({ sequence: index + 1, raw: JSON.stringify(line) })));
  events.push(...apple.finish(completion(finish)));
  return events;
}

const kinds = (events: readonly NormalizedRuntimeEvent[]): string[] => events.map((event) => event.type);
const payload = (event: NormalizedRuntimeEvent | undefined): Record<string, unknown> => (event?.payload ?? {}) as Record<string, unknown>;
const failure = (events: readonly NormalizedRuntimeEvent[]): string => String(payload(events.find((event) => event.type === "run.failed")).message);

function python(): string {
  return execFileSync("/bin/sh", ["-c", "command -v python3"], { encoding: "utf8" }).trim();
}

const fakeHelper = (answer: string): ExecutableLaunch => ({
  commandName: "locust-apple-model",
  discoveredPath: python(),
  executablePath: python(),
  prefixArgs: [FAKE, answer],
  kind: "native",
});

/** Runs a helper for one answer, as the host does, and reads every line it wrote. */
async function runHelper(
  helper: ExecutableLaunch,
  prompt: string,
  options: { readonly stopAtFirstWords?: boolean } = {},
): Promise<NormalizedRuntimeEvent[]> {
  const folder = mkdtempSync(join(tmpdir(), "locust-apple-test-"));
  const spec = createAppleModelCommand(helper, { workspacePath: folder, sandbox: "read-only" });
  const controller = new AbortController();
  const run = createNodeRuntimeProcessRunner({ cancellationGraceMs: 500 }).start(spec, prompt, { signal: controller.signal });
  const apple = normalizer();
  const events: NormalizedRuntimeEvent[] = [];
  for await (const record of run.records) {
    const read = apple.accept(record);
    events.push(...read);
    if (options.stopAtFirstWords === true && read.some((event) => event.type === "message.delta")) controller.abort();
  }
  events.push(...apple.finish(await run.completion));
  return events;
}

const hasPython = (() => {
  try {
    return python().length > 0;
  } catch {
    return false;
  }
})();

describe("Apple Intelligence's lines", () => {
  it("an answer streams as it comes, then is kept whole, and the turn completes", () => {
    const events = lines([{ type: "delta", text: "Red, " }, { type: "delta", text: "blue." }, { type: "done", text: "Red, blue." }]);
    expect(kinds(events)).toEqual(["step.started", "message.delta", "message.delta", "message.delta", "step.completed", "run.completed"]);
    expect(payload(events[1])).toMatchObject({ operation: "append", text: "Red, " });
    expect(payload(events[2])).toMatchObject({ text: "blue." });
    expect(payload(events[3])).toMatchObject({ operation: "replace", text: "Red, blue.", final: true });
    // It keeps no session: every follow-up starts cold and is told what was said.
    expect(events.every((event) => event.runtimeThreadId === undefined)).toBe(true);
    expect(events.every((event) => event.sourceAdapter === APPLE_RUNTIME)).toBe(true);
  });

  it("an answer that rewrites its earlier words replaces them", () => {
    const events = lines([{ type: "delta", text: "Red, blue" }, { type: "replace", text: "Red, yellow, blue." }, { type: "done", text: "" }]);
    expect(payload(events[2])).toMatchObject({ operation: "replace", final: false });
    // A done with no words of its own keeps what was said.
    expect(payload(events[3])).toMatchObject({ text: "Red, yellow, blue." });
    expect(events.at(-1)?.type).toBe("run.completed");
  });

  it("says in words why the model did not answer", () => {
    const cases: readonly [string, string | undefined, string][] = [
      ["contextSizeExceeded", undefined, "more than Apple's on-device model can read at once"],
      ["guardrailViolation", undefined, "its safety rules stopped it"],
      ["refusal", undefined, "declined to answer this"],
      ["unsupportedLanguageOrLocale", undefined, "does not read this language yet"],
      ["rateLimited", undefined, "is busy"],
      ["assetsUnavailable", undefined, "still downloading"],
      ["unavailable", "appleIntelligenceNotEnabled", "Apple Intelligence is turned off on this Mac"],
      ["whatever", undefined, "could not answer"],
    ];
    for (const [kind, reason, words] of cases) {
      const said = failure(lines([{ type: "error", kind, message: "synthetic", ...(reason === undefined ? {} : { reason }) }], { exitCode: 1 }));
      expect(said, kind).toContain(words);
      expect(said, kind).toBe(appleFailureWords(kind, reason));
    }
  });

  it("a helper that ends without answering is a failure, never a finished turn", () => {
    expect(failure(lines([], { exitCode: 3 }))).toBe("Apple's on-device model stopped before it answered.");
    expect(failure(lines([{ type: "done", text: "ok" }], { exitCode: 5 }))).toContain("exited with code 5");
  });

  it("a stopped turn is cancelled, once, and reads nothing after", () => {
    const apple = normalizer();
    apple.accept({ sequence: 1, raw: JSON.stringify({ type: "delta", text: "Think" }) });
    expect(kinds(apple.finish(completion({ cancelled: true, exitCode: null })))).toEqual(["run.cancelled"]);
    expect(apple.finish(completion({ cancelled: true }))).toEqual([]);
  });

  it("a line it cannot read is said, not dropped", () => {
    const apple = normalizer();
    const said = apple.accept({ sequence: 1, raw: "not json" });
    expect(said.map((event) => payload(event).code)).toEqual(["apple.malformed_record"]);
  });
});

describe("whether Apple's model is there", () => {
  it("reads the probe, and takes anything else as not there", () => {
    expect(readAppleProbe('{"available":true,"contextSize":8192}\n')).toEqual({ available: true, context: 8192 });
    expect(readAppleProbe('{"available":false,"reason":"modelNotReady"}')).toEqual({ available: false, reason: "modelNotReady" });
    expect(readAppleProbe("dyld: Library not loaded: FoundationModels")).toEqual({ available: false, reason: "unknown" });
    expect(readAppleProbe("")).toEqual({ available: false, reason: "unknown" });
  });

  it("says what to do for each reason it is not there", () => {
    expect(appleUnavailableWords("appleIntelligenceNotEnabled")).toContain("System Settings, under Apple Intelligence & Siri");
    expect(appleUnavailableWords("modelNotReady")).toContain("still downloading");
    expect(appleUnavailableWords("deviceNotEligible")).toBe("This Mac cannot run Apple Intelligence.");
    expect(appleUnavailableWords("unknown")).toBe("Apple's on-device model is not available on this Mac.");
  });

  it("has a row among the agents, ready or saying why not, and none where it cannot be asked", () => {
    const helper = fakeHelper("echo");
    const ready = appleDiscoveryRecord({ available: true, context: 8192 }, helper);
    expect(ready).toMatchObject({ id: "apple", displayName: "Apple Intelligence", availability: "available", readiness: "ready", optional: true });
    const off = appleDiscoveryRecord({ available: false, reason: "appleIntelligenceNotEnabled" }, helper);
    expect(off?.readiness).toBe("unsupported");
    expect(off?.diagnostics).toEqual([{ code: "version-unsupported", severity: "info", message: appleUnavailableWords("appleIntelligenceNotEnabled") }]);
    expect(appleDiscoveryRecord({ available: false, reason: "unknown" }, helper)).toBeUndefined();
    expect(appleDiscoveryRecord({ available: true, context: 8192 }, undefined)).toBeUndefined();
  });
});

describe("Apple's model as a run", () => {
  it("runs in Ask only, because it has no tools", () => {
    // Plan runs wherever Ask does: both are read-only.
    expect(appleRefusesSandbox("read-only")).toBeUndefined();
    expect(appleRefusesSandbox(undefined)).toBeUndefined();
    for (const sandbox of ["workspace-write", "danger-full-access"] as const) {
      expect(appleRefusesSandbox(sandbox)).toContain("Ask mode only");
      expect(() => createAppleModelCommand(fakeHelper("echo"), { workspacePath: tmpdir(), sandbox })).toThrow(/Ask mode only/);
    }
  });

  it("sends the prompt as one request line on its input, and the spec says so", () => {
    const spec = createAppleModelCommand(fakeHelper("echo"), { workspacePath: tmpdir(), sandbox: "read-only" });
    expect(spec).toMatchObject({ runtime: "apple", stdin: "apple-json", stdout: "jsonl", sandbox: "read-only" });
    expect(appleRequestLine('What is "rebase"?')).toBe('{"prompt":"What is \\"rebase\\"?"}\n');
  });

  it("its brief names the teammate, says it only talks, and keeps the person's instructions", () => {
    const said = appleBrief("Pippin", "Researcher", "  Answer in French.  ", "What is a tag?");
    expect(said.startsWith("You are Pippin, a teammate in Locust (Researcher). You run on Apple's on-device model")).toBe(true);
    expect(said).toContain("cannot read or change files, run commands, or open web pages");
    expect(said).toContain("Your instructions from the person:\nAnswer in French.");
    expect(said.endsWith("\n\nWhat is a tag?")).toBe(true);
    expect(said).not.toContain("<locust-");
    expect(appleBrief("Pippin", " ", undefined, "Hi")).not.toContain("Your instructions");
    expect(appleBrief("Pippin", " ", undefined, "Hi").startsWith("You are Pippin, a teammate in Locust. ")).toBe(true);
  });

  it.runIf(hasPython)("sends the prompt on its input and reads the answer back whole", async () => {
    const events = await runHelper(fakeHelper("echo"), "What is a rebase?");
    expect(events.at(-1)?.type).toBe("run.completed");
    const final = events.filter((event) => event.type === "message.delta").at(-1);
    expect(payload(final)).toMatchObject({ text: "You said: What is a rebase?", final: true });
  });

  it.runIf(hasPython)("a refusal from the helper ends the turn with its words", async () => {
    const events = await runHelper(fakeHelper("error:contextSizeExceeded"), "A very long brief");
    expect(failure(events)).toBe(appleFailureWords("contextSizeExceeded"));
  });

  it.runIf(hasPython)("a helper that says nothing and exits is a failure", async () => {
    const events = await runHelper(fakeHelper("silent"), "Hello");
    expect(failure(events)).toBe("Apple's on-device model stopped before it answered.");
  });

  it.runIf(hasPython)("stopping it mid-answer cancels the turn", async () => {
    const events = await runHelper(fakeHelper("slow"), "Take your time", { stopAtFirstWords: true });
    expect(events.at(-1)?.type).toBe("run.cancelled");
  }, 20_000);
});

/*
 * THE REAL MODEL. Asks this Mac's own Apple Intelligence through the helper the desktop build compiles
 * (apps/desktop/out/native/locust-apple-model, scripts/build-mac-apple-model.cjs). Run with LOCUST_REAL_APPLE=1; needs
 * Apple Intelligence turned on. Under load (other builds) one answer took 16 s here, so the limits are generous.
 */
const REAL_HELPER = fileURLToPath(new URL("../../../apps/desktop/out/native/locust-apple-model", import.meta.url));

describe.runIf(process.env.LOCUST_REAL_APPLE === "1")("the real Apple model (LOCUST_REAL_APPLE=1)", () => {
  it("real_apple: answers one line through the helper, as a teammate briefed by Locust", async () => {
    expect(existsSync(REAL_HELPER), `build the helper first: ${REAL_HELPER}`).toBe(true);
    const probe = readAppleProbe(execFileSync(REAL_HELPER, ["--probe"], { encoding: "utf8", timeout: 10_000 }));
    expect(probe.available, JSON.stringify(probe)).toBe(true);
    const helper: ExecutableLaunch = { commandName: "locust-apple-model", discoveredPath: REAL_HELPER, executablePath: REAL_HELPER, prefixArgs: [], kind: "native" };
    const events = await runHelper(helper, appleBrief("Pippin", "Researcher", undefined, "Reply with exactly the word: ok"));
    expect(events.at(-1)?.type, JSON.stringify(payload(events.at(-1)))).toBe("run.completed");
    const said = String(payload(events.filter((event) => event.type === "message.delta").at(-1)).text).toLowerCase();
    expect(said).toContain("ok");
    expect(said).not.toContain("<locust-");
  }, 240_000);

  it("real_apple: a follow-up told what was said fits and answers", async () => {
    const helper: ExecutableLaunch = { commandName: "locust-apple-model", discoveredPath: REAL_HELPER, executablePath: REAL_HELPER, prefixArgs: [], kind: "native" };
    const first = await runHelper(helper, appleBrief("Pippin", "Researcher", undefined, "In one sentence, what is a git branch?"));
    expect(first.at(-1)?.type).toBe("run.completed");
    const answered = String(payload(first.filter((event) => event.type === "message.delta").at(-1)).text);
    const followUp = `This conversation has earlier turns, but not in your session: it could not be resumed. What was said, oldest first:\n\n- Asked: "In one sentence, what is a git branch?" -- answered: "${answered}"\n\nThe person now asks:\n\nAnd in one sentence, how is that different from a tag?`;
    const second = await runHelper(helper, appleBrief("Pippin", "Researcher", undefined, followUp));
    expect(second.at(-1)?.type, JSON.stringify(payload(second.at(-1)))).toBe("run.completed");
    expect(APPLE_MODEL).toBe("apple-on-device");
  }, 480_000);
});
