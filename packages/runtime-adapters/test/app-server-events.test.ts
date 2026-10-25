import { describe, expect, it } from "vitest";

import {
  APP_SERVER_COMPACTED,
  APP_SERVER_COMPACTED_ON_REQUEST,
  CODEX_THREAD_IN_USE_SENTENCE,
  createAppServerEventNormalizer,
  limitFromSnapshot,
  usageWindowFromSnapshot,
  toolCommandOf,
  toolNameOf,
} from "../src/app-server-events.js";
import type { AppServerNotification } from "../src/app-server.js";

const NOW = "2026-09-01T10:00:00.000Z";

function normalizer() {
  return createAppServerEventNormalizer({
    runId: "run_1",
    missionId: "mission_1",
    runtime: "codex",
    now: () => new Date(NOW),
  });
}

function note(method: string, params: unknown): AppServerNotification {
  return { method, params: params as never };
}

describe("run lifecycle", () => {
  it("takes the runtime thread id from thread/started without emitting an event", () => {
    const app = normalizer();
    const events = app.accept(note("thread/started", { thread: { id: "th_1" } }));
    expect(events).toEqual([]);
    expect(app.runtimeThreadId).toBe("th_1");
  });

  it("starts on turn/started and completes on turn/completed", () => {
    const app = normalizer();
    app.accept(note("thread/started", { thread: { id: "th_1" } }));
    const started = app.accept(note("turn/started", { threadId: "th_1", turn: {} }));
    const completed = app.accept(note("turn/completed", { threadId: "th_1", turn: {} }));
    expect(started.map((event) => event.type)).toEqual(["run.started"]);
    expect(completed.map((event) => event.type)).toEqual(["run.completed"]);
    expect(app.finalized).toBe(true);
  });

  it("goes quiet once finalized", () => {
    const app = normalizer();
    app.accept(note("turn/completed", { threadId: "t", turn: {} }));
    expect(app.accept(note("item/agentMessage/delta", { itemId: "a", delta: "x" }))).toEqual([]);
    expect(app.finish("cancelled")).toEqual([]);
  });

  it("reports a lost connection as failed rather than completed", () => {
    // A turn that never said it finished did not finish. Reporting success on
    // a dropped connection is how a truncated run becomes a completed one.
    const terminal = normalizer().finish("transport-lost");
    expect(terminal[0]?.type).toBe("run.failed");
    expect(terminal[0]?.type === "run.failed" && terminal[0].payload.runtimeTerminal).toBe("missing");
  });

  it("reports cancellation as cancelled", () => {
    const terminal = normalizer().finish("cancelled");
    expect(terminal[0]?.type).toBe("run.cancelled");
  });
});

describe("assistant text", () => {
  it("appends deltas and replaces on the completed item", () => {
    const app = normalizer();
    const deltas = ["Hel", "lo"].flatMap((delta) =>
      app.accept(note("item/agentMessage/delta", { itemId: "msg_1", delta, threadId: "t", turnId: "u" })),
    );
    expect(deltas.every((event) => event.type === "message.delta")).toBe(true);
    for (const event of deltas) {
      expect(event.type === "message.delta" && event.payload.operation).toBe("append");
    }

    // The completed item carries the WHOLE message. Appending it would double
    // every answer -- the same trap every streaming provider sets.
    const [complete] = app.accept(
      note("item/completed", {
        threadId: "t",
        turnId: "u",
        completedAtMs: 1,
        item: { id: "msg_1", type: "agentMessage", text: "Hello" },
      }),
    );
    expect(complete?.type === "message.delta" && complete.payload.operation).toBe("replace");
    expect(complete?.type === "message.delta" && complete.payload.final).toBe(true);
    expect(complete?.type === "message.delta" && complete.payload.text).toBe("Hello");
  });

  it("ignores a delta with no text or no item", () => {
    const app = normalizer();
    expect(app.accept(note("item/agentMessage/delta", { itemId: "a" }))).toEqual([]);
    expect(app.accept(note("item/agentMessage/delta", { delta: "x" }))).toEqual([]);
  });
});

describe("tools", () => {
  function startItem(item: Record<string, unknown>) {
    return note("item/started", { threadId: "t", turnId: "u", startedAtMs: 1, item });
  }
  function completeItem(item: Record<string, unknown>) {
    return note("item/completed", { threadId: "t", turnId: "u", completedAtMs: 2, item });
  }

  it("opens and settles a command execution", () => {
    const app = normalizer();
    const [started] = app.accept(
      startItem({ id: "exec_1", type: "commandExecution", command: "pnpm test" }),
    );
    expect(started?.type).toBe("tool.started");
    expect(started?.type === "tool.started" && started.payload.name).toBe("shell");
    expect(started?.type === "tool.started" && started.payload.command).toBe("pnpm test");

    const [done] = app.accept(
      completeItem({ id: "exec_1", type: "commandExecution", command: "pnpm test", exitCode: 0, status: "completed" }),
    );
    expect(done?.type).toBe("tool.completed");
  });

  // M2 (the code review): a command or patch the person DENIED in
  // Approve-each ends with Codex's status "declined" and was recorded as
  // completed -- with its diff attached -- as though it had run.
  it("records a declined command or patch as failed, not completed", () => {
    for (const type of ["commandExecution", "fileChange"]) {
      const app = normalizer();
      app.accept(startItem({ id: "item_1", type, command: "rm -rf build" }));
      const [done] = app.accept(completeItem({ id: "item_1", type, status: "declined" }));
      expect(done?.type, type).toBe("tool.failed");
      expect(done?.type === "tool.failed" && done.payload.status).toBe("declined");
    }
  });

  it("treats a non-zero exit as a failure even when the status says otherwise", () => {
    const app = normalizer();
    app.accept(startItem({ id: "exec_1", type: "commandExecution", command: "false" }));
    const [done] = app.accept(
      completeItem({ id: "exec_1", type: "commandExecution", exitCode: 1, status: "completed" }),
    );
    expect(done?.type).toBe("tool.failed");
  });

  it("names an MCP tool by server and tool", () => {
    expect(toolNameOf({ type: "mcpToolCall", server: "gmail", tool: "send" })).toBe("gmail.send");
    expect(toolNameOf({ type: "fileChange" })).toBe("apply_patch");
    expect(toolNameOf({ type: "webSearch" })).toBe("web_search");
  });

  it("describes a file change by its count rather than its diff", () => {
    // The diff can be enormous and this lands in a durable ledger.
    expect(toolCommandOf({ type: "fileChange", changes: [1, 2, 3] })).toBe("3 files");
  });

  it("treats an unknown non-tool item as a step, not a tool", () => {
    const app = normalizer();
    const [event] = app.accept(startItem({ id: "r_1", type: "reasoning" }));
    expect(event?.type).toBe("step.started");
  });
});

describe("rate limits", () => {
  it("says nothing while usage is comfortable", () => {
    // The server pushes this unprompted and usually while everything is fine.
    expect(limitFromSnapshot({ weekly: { usedPercent: 31 } })).toBeUndefined();
    expect(limitFromSnapshot({})).toBeUndefined();
    expect(limitFromSnapshot(null)).toBeUndefined();
  });

  it("warns near the limit and reports exhaustion at it", () => {
    expect(limitFromSnapshot({ weekly: { usedPercent: 92 } })?.kind).toBe("temporary-rate-limit");
    expect(limitFromSnapshot({ weekly: { usedPercent: 100 } })?.kind).toBe("quota-exhausted");
  });

  it("reports the worst window, not the first", () => {
    const limit = limitFromSnapshot({ fiveHour: { usedPercent: 10 }, weekly: { usedPercent: 95 } })
    expect(limit?.message).toContain("weekly");
  });

  it("accepts a fractional utilization as well as a percent", () => {
    expect(limitFromSnapshot({ weekly: { utilization: 0.95 } })?.kind).toBe("temporary-rate-limit");
  });

  it("announces a limit once rather than on every push", () => {
    const app = normalizer();
    const limits = (events: readonly { readonly type: string }[]) => events.filter((event) => event.type === "route.limit_detected");
    const first = app.accept(note("account/rateLimits/updated", { rateLimits: { weekly: { usedPercent: 95 } } }));
    const second = app.accept(note("account/rateLimits/updated", { rateLimits: { weekly: { usedPercent: 96 } } }));
    expect(limits(first)).toHaveLength(1);
    // A transcript that repeats the same warning is one nobody reads. (The
    // reading itself changed, 95 to 96, so that is kept -- as a reading.)
    expect(limits(second)).toEqual([]);
  });
});

/*
 * THE READING, NOT ONLY THE WARNING (0.388). Claude's windows were kept as a
 * `claude.usage_window` reading on every push; Codex's only as a warning from
 * 90% up, so a usage meter had nothing to show for Codex until it was nearly
 * gone. The shape below is a real snapshot's, read off a Codex rollout on
 * 2026-09-27 (snake_case on disk; the app-server sends camelCase).
 */
describe("usage readings", () => {
  const ROLLOUT = {
    limit_id: "codex",
    limit_name: null,
    primary: { used_percent: 3.0, window_minutes: 300, resets_at: 1790363478 },
    secondary: { used_percent: 0.0, window_minutes: 10080, resets_at: 1790950278 },
    credits: { has_credits: false, unlimited: false, balance: "0" },
    individual_limit: null,
    plan_type: "plus",
  };

  it("reads every window, fullest first, in the words Claude's reading uses", () => {
    expect(usageWindowFromSnapshot(ROLLOUT)).toBe(
      `5-hour window 3% used · resets ${new Date(1790363478 * 1000).toISOString()} · weekly window 0% used · resets ${new Date(1790950278 * 1000).toISOString()}`,
    );
    expect(usageWindowFromSnapshot({ primary: { usedPercent: 34, windowDurationMins: 300 }, secondary: { usedPercent: 71.6, windowDurationMins: 10080 } })).toBe(
      "weekly window 72% used · 5-hour window 34% used",
    );
  });

  it("reads nothing where there are no windows", () => {
    expect(usageWindowFromSnapshot({ credits: { balance: "0" }, plan_type: "plus" })).toBeUndefined();
    expect(usageWindowFromSnapshot(null)).toBeUndefined();
  });

  it("keeps a reading as a codex.usage_window diagnostic, once per change", () => {
    const app = normalizer();
    const readings = (events: readonly { readonly type: string; readonly payload: unknown }[]) =>
      events.filter((event) => event.type === "adapter.diagnostic" && (event.payload as { code: string }).code === "codex.usage_window").map((event) => (event.payload as { message: string }).message);
    const first = app.accept(note("account/rateLimits/updated", { rateLimits: { primary: { usedPercent: 12, windowDurationMins: 300 } } }));
    const same = app.accept(note("account/rateLimits/updated", { rateLimits: { primary: { usedPercent: 12, windowDurationMins: 300 } } }));
    const moved = app.accept(note("account/rateLimits/updated", { rateLimits: { primary: { usedPercent: 13, windowDurationMins: 300 } } }));
    expect(readings(first)).toEqual(["5-hour window 12% used"]);
    expect(same).toEqual([]);
    expect(readings(moved)).toEqual(["5-hour window 13% used"]);
    // A reading is never a limit: nothing near the line was said.
    expect([...first, ...moved].some((event) => event.type === "route.limit_detected")).toBe(false);
  });
});

/*
 * M3 (the code review): on the app-server transport an exhausted quota or a
 * signed-out account ended the run as kind "unknown" with no limit event, so
 * nothing offered another route; and one flag for every limit meant an
 * exhausted snapshot was never said once a 90% warning had been.
 */
describe("limits and sign-in on the app-server transport", () => {
  const failure = (error: Record<string, unknown>) => {
    const app = normalizer();
    return app.accept(note("error", { threadId: "t", turnId: "u", willRetry: false, error }));
  };

  it("says an exhausted quota after a warning was already said", () => {
    const app = normalizer();
    const warned = app.accept(note("account/rateLimits/updated", { rateLimits: { weekly: { usedPercent: 95 } } }));
    const spent = app.accept(note("account/rateLimits/updated", { rateLimits: { weekly: { usedPercent: 100 } } }));
    // Limit events only: each push also carries the reading itself (0.388).
    const kinds = (events: typeof warned) => events.flatMap((event) => (event.type === "route.limit_detected" ? [event.payload.kind] : []));
    expect(kinds(warned)).toEqual(["temporary-rate-limit"]);
    expect(kinds(spent)).toEqual(["quota-exhausted"]);
  });

  it("classifies a usage-limit error from Codex's own error info, and names the limit", () => {
    for (const info of ["usageLimitExceeded", { usageLimitExceeded: {} }]) {
      const events = failure({ message: "You have hit your limit.", codexErrorInfo: info });
      expect(events.map((event) => event.type)).toEqual(["route.limit_detected", "run.failed"]);
      expect(events[0]?.type === "route.limit_detected" && events[0].payload.kind).toBe("quota-exhausted");
      expect(events[1]?.type === "run.failed" && events[1].payload.kind).toBe("quota-exhausted");
    }
  });

  it("classifies a signed-out account from Codex's own error info", () => {
    const events = failure({ message: "401", codexErrorInfo: "unauthorized" });
    expect(events.map((event) => event.type)).toEqual(["run.failed"]);
    expect(events[0]?.type === "run.failed" && events[0].payload.kind).toBe("authentication-failed");
  });

  it("classifies the schema's other named errors", () => {
    const kindOf = (info: string) => {
      const events = failure({ message: "x", codexErrorInfo: info });
      const failed = events.at(-1);
      return failed?.type === "run.failed" ? failed.payload.kind : undefined;
    };
    expect(kindOf("rateLimitExceeded")).toBe("temporary-rate-limit");
    expect(kindOf("cyberPolicy")).toBe("safety-blocked");
    expect(kindOf("misalignmentPolicyViolation")).toBe("safety-blocked");
  });

  it("names a window by the v2 field, windowDurationMins (read from the generated schema)", () => {
    expect(limitFromSnapshot({ primary: { usedPercent: 93, windowDurationMins: 300 } })?.message).not.toMatch(/primary/);
    expect(limitFromSnapshot({ secondary: { usedPercent: 99, windowDurationMins: 10080 } })?.message).not.toMatch(/secondary/);
  });

  it("falls back to the message when there is no error info", () => {
    const events = failure({ message: "You've hit your usage limit. Try again later." });
    expect(events.at(-1)?.type === "run.failed" && events.at(-1)?.payload).toMatchObject({ kind: expect.stringMatching(/quota-exhausted|temporary-rate-limit/) });
  });
});

/*
 * L1 (the code review): a run that failed before or outside its turn -- a
 * refused handshake, a server that exited, a timeout -- lost its reason: the
 * run knew it and every one read "The runtime connection ended before the
 * turn completed."
 */
describe("a run that ends without the server saying so", () => {
  it("says the reason the run knew, and classifies it", () => {
    const app = normalizer();
    const [failed] = app.finish("transport-lost", "Authentication required: run codex login.");
    expect(failed?.type === "run.failed" && failed.payload.message).toBe("Authentication required: run codex login.");
    expect(failed?.type === "run.failed" && failed.payload.kind).toBe("authentication-failed");
  });

  it("says, in a sentence, that Codex has the conversation open elsewhere (0.835)", () => {
    // Codex's own words, as they reached Colin's window: the conflict, with its log and a config warning after it.
    const log = "thread 01a10f66-f0ff-7622-8e68-875be5256541 already has an active writer 2026-10-25T03:32:46.432808Z ERROR codex_app_server: Codex is ignoring 1 unrecognized configuration setting. 2026-10-25T03:32:48.486820Z ERROR codex_core::session::session: failed to initialize thread persistence: thread-store conflict: thread 01a10f66-f0ff-7622-8e68-875be5256541 already has an active writer";
    const [failed] = normalizer().finish("transport-lost", log);
    expect(failed?.type === "run.failed" && failed.payload.message).toBe(CODEX_THREAD_IN_USE_SENTENCE);
    // What Codex said stays, for whoever reads the evidence.
    expect(failed?.type === "run.failed" && failed.payload.process.stderr).toContain("already has an active writer");
    const app = normalizer();
    const events = app.accept(note("error", { threadId: "t", turnId: "u", willRetry: false, error: { message: log } }));
    expect(events.at(-1)?.type === "run.failed" && events.at(-1)?.payload).toMatchObject({ message: CODEX_THREAD_IN_USE_SENTENCE });
  });

  it("still says something when it knew nothing", () => {
    const [failed] = normalizer().finish("transport-lost");
    expect(failed?.type === "run.failed" && failed.payload.message).toMatch(/ended before the turn completed/);
  });
});

describe("errors and noise", () => {
  it("does not end the run on an error the provider intends to retry", () => {
    const app = normalizer();
    const events = app.accept(
      note("error", { threadId: "t", turnId: "u", willRetry: true, error: { message: "rate limited" } }),
    );
    expect(events[0]?.type).toBe("adapter.diagnostic");
    expect(app.finalized).toBe(false);
  });

  it("ends the run on an error the provider will not retry", () => {
    const app = normalizer();
    const events = app.accept(
      note("error", { threadId: "t", turnId: "u", willRetry: false, error: { message: "no auth" } }),
    );
    expect(events[0]?.type).toBe("run.failed");
    expect(app.finalized).toBe(true);
  });

  it("keeps quiet about the trust bypass Locust's own command guard asks for, and passes every other warning on (0.720)", () => {
    // Word for word as Codex 0.162.1 sent them, 2026-10-10: one of each, every turn.
    const bypass = "`--dangerously-bypass-hook-trust` is enabled. Enabled hooks may run without review for this invocation.";
    const app = normalizer();
    expect(app.accept(note("configWarning", { summary: bypass, details: null }))).toEqual([]);
    expect(app.accept(note("warning", { threadId: "t", message: bypass }))).toEqual([]);
    const other = app.accept(note("configWarning", { summary: "Codex is ignoring 1 unrecognized configuration setting.", details: null }));
    expect(other[0]).toMatchObject({ type: "adapter.diagnostic" });
  });

  it("stays silent on the protocol's many unrelated notifications", () => {
    // MCP startup, remote control and realtime audio all stream through here;
    // a diagnostic per unknown method would bury the actual run.
    const app = normalizer();
    for (const method of [
      "mcpServer/startupStatus/updated",
      "remoteControl/status/changed",
      "thread/tokenUsage/updated",
      "thread/status/changed",
      "fs/changed",
    ]) {
      expect(app.accept(note(method, {}))).toEqual([]);
    }
  });

  it("stamps every event with the mission runtime", () => {
    const app = createAppServerEventNormalizer({ runId: "run_1", runtime: "claude", now: () => new Date(NOW) });
    const events = app.accept(note("turn/started", { threadId: "t", turn: {} }));
    expect(events[0]?.sourceAdapter).toBe("claude");
  });
});

describe("saying which limit, and when it lifts", () => {
  /*
   * VERBATIM from `~/.codex/sessions/.../rollout-2026-09-08T12-26-52-*.jsonl`,
   * captured while Colin's account was near its ceiling. The keys are the
   * snake_case ones Codex writes to disk.
   *
   * This fixture is the whole point. Reporting "primary limit 93% used" told
   * him his Codex quota was nearly gone. It was not: `primary` is the
   * FIVE-HOUR window, `secondary` is the weekly one, and the weekly bucket
   * was at 14%. He pushed back, and he was right.
   */
  const REAL = {
    primary: { used_percent: 93.0, window_minutes: 300, resets_at: 1788892785 },
    secondary: { used_percent: 14.0, window_minutes: 10080, resets_at: 1789479585 },
  };

  it("names the window a person recognises, not the API's own key", () => {
    const limit = limitFromSnapshot(REAL);
    expect(limit?.message).toContain("5-hour");
    expect(limit?.message).not.toContain("primary");
  });

  it("still reports the worst window, which is the one that stops you", () => {
    expect(limitFromSnapshot(REAL)?.message).toContain("93%");
    expect(limitFromSnapshot(REAL)?.kind).toBe("temporary-rate-limit");
  });

  it("says when it lifts, because that is the decision", () => {
    // "Wait an hour" and "stop for the week" are different actions and the
    // percentage alone cannot tell them apart.
    expect(limitFromSnapshot(REAL)?.message).toMatch(/resets \d\d:\d\d/);
  });

  it("reads the camelCase spelling the app-server sends", () => {
    // Two serialisations of one fact: the notification is camelCase, the
    // rollout file on disk is snake_case. A field it cannot see is a field it
    // silently drops.
    const limit = limitFromSnapshot({ primary: { usedPercent: 93, windowMinutes: 300, resetsAt: 1788892785 } });
    expect(limit?.message).toContain("5-hour");
    expect(limit?.message).toMatch(/resets \d\d:\d\d/);
  });

  it("calls the seven-day window weekly", () => {
    expect(limitFromSnapshot({ secondary: { used_percent: 99, window_minutes: 10080 } })?.message)
      .toContain("weekly");
  });

  it("falls back to the raw key when no window is given", () => {
    // Better a label that says little than a window invented from nothing.
    expect(limitFromSnapshot({ primary: { usedPercent: 95 } })?.message).toContain("primary");
  });

  it("says nothing about a reset it was not told", () => {
    expect(limitFromSnapshot({ primary: { usedPercent: 95, windowMinutes: 300 } })?.message)
      .not.toContain("resets");
  });

  it("stays quiet well below the ceiling", () => {
    expect(limitFromSnapshot({ primary: { used_percent: 14, window_minutes: 10080 } })).toBeUndefined();
  });
})

/*
 * A2.5: Codex 0.156.1 carries a `contextCompaction` item and a
 * `thread/compacted` notification (read in the binary, 2026-09-24; not
 * captured). One compaction is said once, whichever of them arrives, and
 * whether it sends one or both.
 */
describe("Codex compacting its conversation", () => {
  const item = note("item/completed", { item: { type: "contextCompaction", id: "c1" } });
  const notice = note("thread/compacted", { threadId: "th_1" });
  const lines = (events: readonly { type: string; payload: unknown }[]) =>
    events.filter((event) => event.type === "adapter.diagnostic"
      && (event.payload as { code: string }).code === "codex.context_compacted");

  const asked = note("item/completed", { item: { type: "userMessage", id: "u1", content: [] } });

  it("is one line for one compaction, however it is reported", () => {
    for (const order of [[item, notice], [notice, item], [item], [notice]]) {
      const app = normalizer();
      const said = lines([asked, ...order].flatMap((entry) => app.accept(entry)));
      expect(said).toHaveLength(1);
      expect(said[0]?.payload).toMatchObject({ level: "info", message: APP_SERVER_COMPACTED });
    }
  });

  it("says it was asked for when the turn was the compaction alone: /compact (0.428)", () => {
    const app = normalizer();
    const said = lines([item, notice].flatMap((entry) => app.accept(entry)));
    expect(said[0]?.payload).toMatchObject({ message: APP_SERVER_COMPACTED_ON_REQUEST });
  });

  it("is a line for each compaction when there are two", () => {
    const app = normalizer();
    expect(lines([item, notice, item, notice].flatMap((entry) => app.accept(entry)))).toHaveLength(2);
  });

  it("opens no step for the compaction item", () => {
    const app = normalizer();
    expect(app.accept(note("item/started", { item: { type: "contextCompaction", id: "c1" } }))).toEqual([]);
  });
});

describe("a review Codex was asked for (0.428)", () => {
  it("draws only the findings, once: the review-mode bookends are not steps", () => {
    // Measured 2026-09-28, 0.157.1: enteredReviewMode, then exitedReviewMode
    // carrying the findings, then the same findings as an agent message.
    const app = normalizer();
    app.accept(note("thread/started", { thread: { id: "th_1" } }));
    app.accept(note("turn/started", { threadId: "th_1", turn: {} }));
    const bookends = [
      ...app.accept(note("item/started", { threadId: "th_1", item: { type: "enteredReviewMode", id: "r1", review: "current changes" } })),
      ...app.accept(note("item/completed", { threadId: "th_1", item: { type: "enteredReviewMode", id: "r1", review: "current changes" } })),
      ...app.accept(note("item/started", { threadId: "th_1", item: { type: "exitedReviewMode", id: "r2", review: "add subtracts" } })),
      ...app.accept(note("item/completed", { threadId: "th_1", item: { type: "exitedReviewMode", id: "r2", review: "add subtracts" } })),
    ];
    expect(bookends).toEqual([]);
  });
});
