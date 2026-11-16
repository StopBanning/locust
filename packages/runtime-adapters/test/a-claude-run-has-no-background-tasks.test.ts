import { describe, expect, it } from "vitest";

import { CLAUDE_RUN_ENV, createClaudePrintCommand } from "../src/commands.js";

/*
 * A Claude run is cut off when its turn ends, and so is anything it started in the background (see
 * background-work-ends-with-the-run.test.ts). Two Opus runs on 15 Nov 2026, resumed after a run that had background jobs,
 * were ended about a minute in with `aborted_tools`. Locust switches background tasks off for its Claude runs.
 */
const EXECUTABLE = { executablePath: "/Users/someone/.local/bin/claude", prefixArgs: [] as readonly string[] };
const options = { workspacePath: "/Users/someone/work/project", sandbox: "full-access" as const };

describe("a Claude run", () => {
  it("starts with Claude Code's own switch for background tasks turned off", () => {
    expect(CLAUDE_RUN_ENV).toEqual({ CLAUDE_CODE_DISABLE_BACKGROUND_TASKS: "1" });
    const first = createClaudePrintCommand(EXECUTABLE, options);
    expect(first.env?.CLAUDE_CODE_DISABLE_BACKGROUND_TASKS).toBe("1");
  });

  it("has it off when the session is resumed too, and keeps the launch's own variables beside it", () => {
    const resumed = createClaudePrintCommand({ ...EXECUTABLE, env: { CLAUDE_CONFIG_DIR: "/Users/someone/.claude-2" } }, { ...options, resumeThreadId: "8d202ff1-15d2-4aa6-9fdf-f15afcd65e5e" });
    expect(resumed.args).toContain("--resume");
    expect(resumed.env).toEqual({ CLAUDE_CONFIG_DIR: "/Users/someone/.claude-2", CLAUDE_CODE_DISABLE_BACKGROUND_TASKS: "1" });
  });
});
