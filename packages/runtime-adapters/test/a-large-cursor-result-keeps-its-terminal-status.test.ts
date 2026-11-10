import { EventEmitter } from "node:events";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

import { createCursorEventNormalizer, createNodeRuntimeProcessRunner } from "../src/index.js";
import type { SpawnedRuntimeProcess } from "../src/index.js";

async function receive(line: string) {
  const stdout = new EventEmitter();
  const stdin = Object.assign(new EventEmitter(), { write: () => true, end: () => undefined });
  const child = Object.assign(new EventEmitter(), { stdin, stdout, stderr: new EventEmitter(), kill: () => true }) as unknown as SpawnedRuntimeProcess;
  const runner = createNodeRuntimeProcessRunner({ maxRecordBytes: 1_024, spawnProcess: () => child });
  const run = runner.start(
    { runtime: "cursor", executablePath: resolve("fake-cursor"), args: ["-p"], cwd: resolve("fake-workspace"), stdin: "prompt", stdout: "jsonl", oversizedStandIns: true },
    "prompt",
  );
  for (let at = 0; at < line.length; at += 997) stdout.emit("data", line.slice(at, at + 997));
  (child as unknown as EventEmitter).emit("close", 0, null);
  const completion = await run.completion;
  return { completion, raw: run.records.drainAvailable().map((record) => record.raw)[0]! };
}

describe("a large Cursor result keeps its terminal status", () => {
  it("keeps success and failure after a repeated answer larger than the result cap, across chunks and without a newline", async () => {
    for (const is_error of [false, true]) {
      const line = JSON.stringify({ type: "result", result: "x".repeat(9 * 1024 * 1024) + '\n"tail"', is_error, subtype: is_error ? "error" : "success", usage: { inputTokens: 3, outputTokens: 2 } });
      const { raw, completion } = await receive(line);
      expect(raw.length).toBeLessThan(70_000);
      expect(JSON.parse(raw)).toMatchObject({ type: "result", is_error, subtype: is_error ? "error" : "success" });
      expect(completion.oversizedRecordsDropped).toBe(0);
      const cursor = createCursorEventNormalizer({ runId: "r", missionId: "m", cliVersion: "test", now: () => new Date() });
      cursor.accept({ sequence: 1, raw });
      expect(cursor.finish(completion).at(-1)?.type).toBe(is_error ? "run.failed" : "run.completed");
    }
  });
});
