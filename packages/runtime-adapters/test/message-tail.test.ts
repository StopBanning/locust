import { describe, expect, it } from "vitest";

import { boundedAnswerText, boundedMessageText } from "../src/codex-events.js";

/**
 * The cap keeps the END, because that is where the protocol blocks live.
 *
 * Reported 2026-09-08 by a Cursor teammate reading this source from inside
 * Locust: every teammate block -- share, memory, ask, room task -- is taught to
 * sit at the end of the reply ("end your reply with exactly this block and
 * nothing after it"), and the cap kept the head. A long answer therefore had
 * its block cut off here, and the run posted nothing to the workroom while the
 * thread showed a complete, healthy turn.
 */
const SHARE = '<locust-share to="Gem">Watch the to= attribute.</locust-share>';

describe("bounding a very long message", () => {
  it("leaves a message under the cap exactly as it is", () => {
    expect(boundedMessageText("short")).toBe("short");
    const exact = "x".repeat(16_384);
    expect(boundedMessageText(exact)).toBe(exact);
  });

  it("KEEPS a trailing protocol block on an answer far over the cap", () => {
    // THE test. This is the one that was failing in production.
    const long = `${"x".repeat(80_000)}
${SHARE}`;
    const bounded = boundedMessageText(long);
    expect(bounded).toContain(SHARE);
  });

  it("keeps the beginning too, so the answer still reads as an answer", () => {
    const long = `The short version is yes.${"x".repeat(80_000)}
${SHARE}`;
    const bounded = boundedMessageText(long);
    expect(bounded.startsWith("The short version is yes.")).toBe(true);
    expect(bounded).toContain(SHARE);
  });

  it("says where it cut, rather than splicing two halves silently", () => {
    const bounded = boundedMessageText("x".repeat(80_000));
    expect(bounded).toMatch(/truncated/);
  });

  it("still respects the ledger cap it exists to enforce", () => {
    // The control. A function that simply stopped truncating would pass every
    // test above and break the thing this cap is for.
    for (const size of [16_385, 40_000, 500_000]) {
      expect(boundedMessageText("x".repeat(size)).length).toBeLessThanOrEqual(16_384);
    }
  });

  it("keeps a long ANSWER whole: 60,000 characters, no middle cut (0.869)", () => {
    // Colin, 2026-11-07: a Codex report read over Locust's MCP server arrived
    // with the middle cut out at 16,384. Answers have the ledger's larger limit.
    const report = Array.from({ length: 600 }, (_, i) => `## Section ${String(i)}\n${"word ".repeat(18)}`).join("\n");
    expect(report.length).toBeGreaterThan(60_000);
    expect(boundedAnswerText(report)).toBe(report);
    // The control: everything else keeps the old bound.
    expect(boundedMessageText(report).length).toBeLessThanOrEqual(16_384);
  });

  it("still bounds an answer past ITS limit, keeping the trailing block", () => {
    const huge = `The short version is yes.${"x".repeat(300_000)}\n${SHARE}`;
    const bounded = boundedAnswerText(huge);
    expect(bounded.length).toBeLessThanOrEqual(131_072);
    expect(bounded.startsWith("The short version is yes.")).toBe(true);
    expect(bounded).toContain(SHARE);
  });
});
