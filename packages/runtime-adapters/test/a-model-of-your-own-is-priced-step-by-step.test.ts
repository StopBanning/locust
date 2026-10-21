import { describe, expect, it } from "vitest";

import { createOpenCodeEventNormalizer, listedStepCost } from "../src/opencode-events.js";
import type { OpenCodePrice } from "../src/commands.js";
import type { RuntimeProcessCompletion } from "../src/process-runner.js";

/**
 * A MODEL OF YOUR OWN, PRICED STEP BY STEP (0.810).
 *
 * A model of the person's own that its provider lists is priced from that
 * list. OpenCode, told the price in a provider's config, keeps its base and
 * drops its tiers (MEASURED on 1.18.27, probe-opencode-own-price: a Claude
 * Haiku 5.5 step over 100k of context read $0.01505 for $0.07525), so each
 * step is priced here, by OpenCode's own sum for the models it lists.
 */
const HAIKU: OpenCodePrice = { input: 0.1, output: 0.5, cache_read: 0.01, cache_write: 0.125, tiers: [{ input: 0.5, output: 2.5, cache_read: 0.05, cache_write: 0.625, tier: { type: "context", size: 100_000 } }] };
const step = (input: number, extra: Partial<{ output: number; reasoning: number; cacheRead: number; cacheWrite: number }> = {}) => ({ input, output: 0, reasoning: 0, cacheRead: 0, cacheWrite: 0, ...extra });

describe("a step at its listed price", () => {
  it("prices input, output, reasoning and both caches, each at its own rate", () => {
    const flat: OpenCodePrice = { input: 2, output: 10, cache_read: 0.1, cache_write: 2.5 };
    // 1,000 x 2 + 100 x 10 + 50 x 10 (reasoning, at the output price) + 2,000 x 0.1 + 500 x 2.5, a million tokens.
    expect(listedStepCost(flat, step(1000, { output: 100, reasoning: 50, cacheRead: 2000, cacheWrite: 500 }))).toBeCloseTo(0.00495, 10);
    // A price that names no cache rate charges none.
    expect(listedStepCost({ input: 1, output: 2 }, step(1000, { cacheRead: 5000 }))).toBeCloseTo(0.001, 10);
  });

  it("takes the dearer tier once the step's context, caches included, passes its size", () => {
    // 50,000 in and 20,000 from cache: 70k of context, the base.
    expect(listedStepCost(HAIKU, step(50_000, { cacheRead: 20_000 }))).toBeCloseTo((50_000 * 0.1 + 20_000 * 0.01) / 1e6, 10);
    // 90,000 in and 20,000 from cache: 110k of context, over the tier, though neither count is.
    expect(listedStepCost(HAIKU, step(90_000, { cacheRead: 20_000, output: 100 }))).toBeCloseTo((90_000 * 0.5 + 20_000 * 0.05 + 100 * 2.5) / 1e6, 10);
    // At the size exactly, still the base: a tier is for context over it.
    expect(listedStepCost(HAIKU, step(100_000))).toBeCloseTo(0.01, 10);
  });

  it("takes the largest tier passed, and the older over-200k price where a list gives that", () => {
    const tiered: OpenCodePrice = { input: 1, output: 1, tiers: [{ input: 2, output: 2, tier: { type: "context", size: 100_000 } }, { input: 3, output: 3, tier: { type: "context", size: 200_000 } }] };
    expect(listedStepCost(tiered, step(250_000))).toBeCloseTo(0.75, 10);
    expect(listedStepCost(tiered, step(150_000))).toBeCloseTo(0.3, 10);
    const older: OpenCodePrice = { input: 3, output: 15, context_over_200k: { input: 6, output: 22.5 } };
    expect(listedStepCost(older, step(210_000))).toBeCloseTo(1.26, 10);
    expect(listedStepCost(older, step(190_000))).toBeCloseTo(0.57, 10);
  });
});

const NOW = "2026-10-21T12:00:00.000Z";
const done: RuntimeProcessCompletion = { exitCode: 0, signal: null, stderr: "", stderrTruncated: false, recordCount: 3, cancelled: false, forcedTerminationAttempted: false, terminationUnconfirmed: false, inputDeliveryFailed: false, outputLimitExceeded: false, startedAt: NOW, finishedAt: NOW };
const usageOf = (price: OpenCodePrice | undefined) => {
  const opencode = createOpenCodeEventNormalizer({ runId: "run_1", missionId: "mission_1", model: "own-0a1b2c3d/claude-haiku-5-5", now: () => new Date(NOW), ...(price === undefined ? {} : { price }) });
  const records = [
    { type: "step_start", sessionID: "ses_1", part: { type: "step-start" } },
    // OpenCode's own figure for this step: the base only.
    { type: "step_finish", sessionID: "ses_1", part: { type: "step-finish", reason: "tool-calls", cost: 0.012, tokens: { input: 120_000, output: 0, reasoning: 0, cache: { read: 0, write: 0 } } } },
    { type: "step_start", sessionID: "ses_1", part: { type: "step-start" } },
    { type: "step_finish", sessionID: "ses_1", part: { type: "step-finish", reason: "stop", cost: 0.0001, tokens: { input: 1000, output: 100, reasoning: 0, cache: { read: 0, write: 0 } } } },
  ];
  const events = records.flatMap((record, index) => opencode.accept({ sequence: index + 1, raw: JSON.stringify(record) }));
  events.push(...opencode.finish(done));
  return (events.at(-1)?.payload as { usage?: Record<string, number> }).usage;
};

describe("a run of a model of your own", () => {
  it("adds up each step at its listed price, the step over the tier at the tier's", () => {
    // 120,000 x 0.5, then 1,000 x 0.1 + 100 x 0.5: the first step is over Haiku's 100k tier, the second is not.
    expect(usageOf(HAIKU)?.usd).toBeCloseTo(0.06 + 0.00015, 10);
  });

  it("reads OpenCode's own figure where no price is given, as before", () => {
    expect(usageOf(undefined)?.usd).toBeCloseTo(0.0121, 10);
  });
});
