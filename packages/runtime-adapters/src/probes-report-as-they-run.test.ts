import { describe, expect, it, vi } from "vitest";

import { discoverInstalledRuntimes } from "./discovery.js";

/**
 * Discovery reports as it happens, one probe at a time.
 *
 * **`probe.started` fires before the subprocess is spawned.** The whole
 * value of the boot screen is the gap between issuing a command and its
 * answer. If both events arrive together there is nothing to show during the
 * wait, and the screen can only say "checking…" and then jump — which is
 * exactly what it did before this.
 *
 * **Probes START one at a time, in order.** Raced, every row appears at once,
 * every elapsed counter ticks together, and a log meant to be read is noise.
 *
 * They are no longer strictly SEQUENTIAL, and the reason is measured. The
 * handoff asked for sequential on the stated assumption that "the total cost
 * is a few hundred ms". On Colin's own machine, where the runtimes are
 * actually installed, `cursor-agent` ran past six seconds — and strictly
 * sequential makes the total the SUM, so the slowest runtime became
 * everybody else's wait on every launch. He felt it immediately: *"maybe its
 * taking so long because its not realizing the runtimes are connected?"*
 *
 * The stagger keeps what sequential was FOR (rows arriving one at a time, in
 * order) and drops what it cost (the sum).
 */

/**
 * A locator that finds nothing -- the shape of a machine with no runtimes
 * installed, which is the case the stagger must not tax.
 *
 * Worth knowing when reading these: `discoverOne` returns HERE, before the
 * runner is ever called. A "slow runner" with this locator is not slow at
 * all, and a test written that way measures nothing -- which is exactly
 * what happened on the first attempt at the timing tests below.
 */
const locator = { find: async () => undefined };

/** A locator that DOES find something, so the runner is actually reached. */
const foundLocator = { find: async () => ({ executablePath: "C:/fake/bin.exe", prefixArgs: [] }) };

const slowRunner = (ms: number) =>
  ({
    run: async () => {
      await new Promise((resolve) => setTimeout(resolve, ms));
      return { stdout: "", stderr: "", exitCode: 0 };
    },
  }) as never;

const instantRunner = { run: async () => ({ stdout: "", stderr: "", exitCode: 0 }) } as never;

describe("discovery as it happens", () => {
  it("never reports a probe finished before it reported it started", async () => {
    const order: string[] = [];
    await discoverInstalledRuntimes({
      runner: instantRunner,
      locator: locator as never,
      watch: {
        started: (runtime) => order.push(`started:${runtime.id}`),
        finished: (runtime) => order.push(`finished:${runtime.id}`),
      },
    });

    expect(order.length).toBeGreaterThan(0);
    const startedAt = new Map<string, number>();
    order.forEach((entry, index) => {
      const [phase, id = ""] = entry.split(":");
      if (phase === "started") startedAt.set(id, index);
      else {
        expect(startedAt.has(id), `${entry} arrived with no start`).toBe(true);
        expect(index).toBeGreaterThan(startedAt.get(id) ?? Number.MAX_SAFE_INTEGER);
      }
    });
  });

  it("starts them one at a time, in order, however slow they are", async () => {
    // The log's legibility is about the ORDER things begin in, not about
    // whether they overlap. A slow probe must not hold up the next START.
    const starts: number[] = [];
    await discoverInstalledRuntimes({
      runner: slowRunner(60),
      locator: foundLocator as never,
      staggerMs: 5,
      watch: { started: () => starts.push(Date.now()), finished: () => undefined },
    });

    expect(starts.length).toBeGreaterThan(2);
    for (let index = 1; index < starts.length; index += 1) {
      expect(starts[index]).toBeGreaterThanOrEqual(starts[index - 1] ?? 0);
    }
    // The last start lands long before the sum of the probe times, which is
    // the whole point: a slow runtime delays nobody else's row.
    expect((starts.at(-1) ?? 0) - (starts[0] ?? 0)).toBeLessThan(60 * starts.length);
  });

  it("does not make the total the sum of every probe", async () => {
    vi.useFakeTimers();
    try {
      let completed = false;
      const discovery = discoverInstalledRuntimes({
        runner: slowRunner(50),
        locator: foundLocator as never,
        staggerMs: 2,
        watch: { started: () => undefined, finished: () => undefined },
      }).then(() => { completed = true; });
      // OpenCode checks its version before selecting its protocol's probes.
      // That extra phase still overlaps the other runtimes. Keep the same
      // strict boundary without measuring the test machine's scheduling load.
      await vi.advanceTimersByTimeAsync(249);
      expect(completed).toBe(true);
      await discovery;
    } finally {
      await vi.runAllTimersAsync();
      vi.useRealTimers();
    }
  });

  it("names the binary it is about to run, and its id, which are not the same", async () => {
    /*
     * Cursor's id is `cursor` and its command is `cursor-agent`, and that
     * difference cost a real defect: the screen keyed rows by one and
     * results by the other, so Cursor sat on "still waiting" while the table
     * underneath showed it green. Both travel on the event.
     */
    const seen: { id: string; bin: string; displayName: string }[] = [];
    await discoverInstalledRuntimes({
      runner: instantRunner,
      locator: locator as never,
      watch: { started: (runtime) => seen.push(runtime), finished: () => undefined },
    });

    expect(seen.length).toBeGreaterThan(0);
    for (const runtime of seen) {
      expect(runtime.id.length).toBeGreaterThan(0);
      expect(runtime.bin.length).toBeGreaterThan(0);
      expect(runtime.displayName.length).toBeGreaterThan(0);
    }
    const cursor = seen.find((runtime) => runtime.id === "cursor");
    expect(cursor?.bin).toBe("cursor-agent");
  });

  it("still answers with every runtime when nobody is watching", async () => {
    // The watcher is an observer, never a condition of the sweep.
    const found = await discoverInstalledRuntimes({ runner: instantRunner, locator: locator as never });
    expect(found.length).toBeGreaterThan(0);
  });
});

describe("a machine with nothing installed waits for nothing", () => {
  /*
   * Colin, 2026-09-14: "lets make sure if ian or anyone else gets the app
   * that doesnt have all the runtimes that the loading screen isnt a
   * nuisance."
   *
   * A fixed stagger put a floor under the sweep -- seven definitions at
   * 240ms is about 1.7s before the last one starts -- and that floor was
   * paid by exactly the people who should pay nothing. A probe for a binary
   * that is not on PATH answers in about 10ms because nothing is spawned,
   * so on a bare machine the stagger WAS the whole wait.
   *
   * Each probe now waits for the one before it OR for the stagger,
   * whichever comes first.
   */
  it("chains straight through when every probe is instant", async () => {
    const began = Date.now();
    await discoverInstalledRuntimes({
      runner: instantRunner,
      locator: locator as never,
      staggerMs: 240,
      watch: { started: () => undefined, finished: () => undefined },
    });
    // Seven definitions at a fixed 240ms would be about 1,680ms.
    expect(Date.now() - began).toBeLessThan(400);
  });

  it("still spaces them out when the probes are slow", async () => {
    // The beat only exists where there is a wait to fill.
    const starts: number[] = [];
    await discoverInstalledRuntimes({
      runner: slowRunner(200),
      locator: foundLocator as never,
      staggerMs: 40,
      watch: { started: () => starts.push(Date.now()), finished: () => undefined },
    });
    const gaps = starts.slice(1).map((at, index) => at - (starts[index] ?? 0));
    // Every gap is the stagger, not the probe: the next one does not wait
    // for a slow answer.
    for (const gap of gaps) expect(gap).toBeLessThan(200);
    expect(Math.max(...gaps)).toBeGreaterThan(10);
  });
});
