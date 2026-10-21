import { describe, expect, it } from "vitest";

import { createOpenCodeRunCommand, createOpenCodeServeCommand, withOpenCodeProviders } from "../src/index.js";
import type { ExecutableLaunch } from "../src/index.js";

/**
 * YOUR OWN MODEL IS AN OPENCODE PROVIDER (0.357).
 *
 * A company with its own model -- Colin's father's builds one -- should be
 * able to "just insert their model". Any OpenAI-compatible endpoint is
 * declared to OpenCode as a provider in the config the run already carries,
 * the shape measured working through this builder on 2026-09-25. The key goes
 * in the child's environment with that config, and nowhere a record reads.
 */
const executable: ExecutableLaunch = { executablePath: "C:/tools/opencode.exe", prefixArgs: [] };
const acme = { name: "Acme Chat", baseUrl: "https://llm.acme.example/v1", apiKey: "sk-acme-secret", models: ["acme-70b"] };

describe("your own model", () => {
  it("is declared as an openai-compatible provider, beside the run's own permissions", () => {
    const spec = createOpenCodeRunCommand(executable, {
      workspacePath: "C:/work",
      prompt: "hello",
      model: "own-a1b2/acme-70b",
      sandbox: "read-only",
      providers: { "own-a1b2": acme },
    });
    const config = JSON.parse(spec.env?.OPENCODE_CONFIG_CONTENT ?? "{}");
    expect(config.provider["own-a1b2"]).toEqual({
      npm: "@ai-sdk/openai-compatible",
      name: "Acme Chat",
      options: { baseURL: "https://llm.acme.example/v1", apiKey: "sk-acme-secret" },
      models: { "acme-70b": { name: "acme-70b" } },
    });
    // The read-only run's denials are still there: a provider is added, nothing replaced.
    expect(config.permission).toBeDefined();
    expect(spec.args).toContain("own-a1b2/acme-70b");
  });

  it("puts the key in the environment only -- never in the arguments", () => {
    const spec = createOpenCodeRunCommand(executable, {
      workspacePath: "C:/work",
      prompt: "hello",
      model: "own-a1b2/acme-70b",
      sandbox: "workspace-write",
      providers: { "own-a1b2": acme },
    });
    expect(spec.args.join(" ")).not.toContain("sk-acme-secret");
    expect(spec.env?.OPENCODE_CONFIG_CONTENT).toContain("sk-acme-secret");
  });

  it("works for a run that stops to ask, too", () => {
    const spec = createOpenCodeServeCommand(executable, { workspacePath: "C:/work", providers: { "own-a1b2": acme } });
    const config = JSON.parse(spec.env?.OPENCODE_CONFIG_CONTENT ?? "{}");
    expect(config.provider["own-a1b2"].options.baseURL).toBe("https://llm.acme.example/v1");
    expect(config.permission.edit).toBe("ask");
  });

  it("sends a placeholder to an endpoint that asks for no key, which the SDK needs and the server ignores", () => {
    const config = JSON.parse(withOpenCodeProviders("{}", { "own-local": { name: "Local", baseUrl: "http://127.0.0.1:11434/v1", models: ["llama3"] } }) ?? "{}");
    expect(config.provider["own-local"].options.apiKey).toBe("not-needed");
  });

  it("turns every tool off for a model that only chats, and only for it (0.358)", () => {
    // MEASURED 2026-09-26 against an endpoint that refuses tools: declared
    // `tool_call: false` in OpenCode's model config, it was still sent ten;
    // with every tool off for the run it was sent none, and answered.
    const chatOnly = JSON.parse(withOpenCodeProviders("{}", { "own-chat": { ...acme, toolCalls: false } }) ?? "{}");
    expect(chatOnly.tools).toEqual({ "*": false });
    const withTools = JSON.parse(withOpenCodeProviders("{}", { "own-a1b2": acme }) ?? "{}");
    expect(withTools.tools).toBeUndefined();
  });

  it("declares a model's listed price, which OpenCode prices each step with, and none where none is given (0.810)", () => {
    const price = { input: 2, output: 10, cache_read: 0.1, cache_write: 2.5 };
    const priced = JSON.parse(withOpenCodeProviders("{}", { "own-a1b2": { ...acme, prices: { "acme-70b": price } } }) ?? "{}");
    expect(priced.provider["own-a1b2"].models).toEqual({ "acme-70b": { name: "acme-70b", cost: price } });
    const unpriced = JSON.parse(withOpenCodeProviders("{}", { "own-a1b2": acme }) ?? "{}");
    expect(unpriced.provider["own-a1b2"].models).toEqual({ "acme-70b": { name: "acme-70b" } });
  });

  it("changes nothing when there is no model of your own", () => {
    expect(withOpenCodeProviders('{"permission":{}}', undefined)).toBe('{"permission":{}}');
    expect(withOpenCodeProviders(undefined, {})).toBeUndefined();
  });

  it("refuses an id OpenCode could not take, and an address that is not http(s)", () => {
    expect(() => withOpenCodeProviders("{}", { "Bad Id": acme })).toThrow(/provider id/);
    expect(() => withOpenCodeProviders("{}", { "own-x": { ...acme, baseUrl: "file:///etc/passwd" } })).toThrow(/http\(s\)/);
  });
});
