import { describe, expect, it } from "vitest";

import { OPENCODE_SEALED_CONFIG, createOpenCodeServeCommand, openCodeModeEnv } from "../src/commands.js";

/*
 * 0.824, Grok's review #4: Scout's look read other people's words -- pull request titles, check output, comments --
 * on an ordinary Ask turn, which on OpenCode reads files, fetches web pages and has Locust's git tool. A sealed run
 * reads only what its prompt gives it.
 */
const EXECUTABLE = { executablePath: "/usr/bin/opencode", prefixArgs: [] as readonly string[] };
const everything = {
  workspacePath: "/work",
  gitReads: { command: "locust", args: ["git-reads.mjs"], env: {}, instructions: "/profile/git.md" },
  browserMcp: { configPath: "/profile/browser.json", command: "locust", args: ["browser.mjs"], env: {} },
  skillsDir: "/profile/run-skills/abc/skills",
  referenceFolders: ["/elsewhere"],
};

describe("a sealed OpenCode run uses no tool", () => {
  it("asks for every tool, so a run with nobody to ask refuses each one, and reaches no folder outside its own", () => {
    const env = openCodeModeEnv({ ...everything, sandbox: "read-only", sealed: true });
    const config = JSON.parse(env.OPENCODE_CONFIG_CONTENT!) as Record<string, unknown>;
    expect(config.permission).toEqual({ "*": "ask", external_directory: "deny" });
    // No git tool, no browser, no skills, no reference folder: nothing beside the prompt.
    expect(config).not.toHaveProperty("mcp");
    expect(config).not.toHaveProperty("skills");
    expect(config).not.toHaveProperty("instructions");
    expect(env.OPENCODE_PURE).toBe("1");
  });

  it("keeps every tool listed, the shell among them, because the free tier refuses a run whose tools are gone", () => {
    const { permission } = JSON.parse(OPENCODE_SEALED_CONFIG) as { permission: Record<string, unknown> };
    expect(Object.values(permission).filter((rule) => rule === "deny")).toEqual(["deny"]);
    expect(permission.external_directory).toBe("deny");
  });

  it("is what a served run is told, and seals only a read-only run", () => {
    const served = createOpenCodeServeCommand(EXECUTABLE, { ...everything, sandbox: "read-only", sealed: true });
    expect(JSON.parse(served.env!.OPENCODE_CONFIG_CONTENT!).permission).toEqual({ "*": "ask", external_directory: "deny" });
    // An ordinary Ask run keeps its reading tools and Locust's git tool, as before.
    const ask = JSON.parse(openCodeModeEnv({ ...everything, sandbox: "read-only" }).OPENCODE_CONFIG_CONTENT!) as { permission: Record<string, unknown>; mcp?: Record<string, unknown> };
    expect(ask.permission.webfetch).toBe("allow");
    expect(ask.mcp).toHaveProperty("locust_git");
    // A mode that may edit is never sealed.
    const edit = JSON.parse(openCodeModeEnv({ ...everything, sandbox: "workspace-write", sealed: true }).OPENCODE_CONFIG_CONTENT!) as { permission: Record<string, unknown> };
    expect(edit.permission).not.toEqual({ "*": "ask", external_directory: "deny" });
  });
});
