import { describe, expect, it } from "vitest";

import { ACP_DECLINED, ACP_PROMPT_RESULT, ACP_SESSION, createAcpEventNormalizer } from "../src/acp-events.js";
import { acpCapabilitiesOf, startAcpRun } from "../src/acp-run.js";
import type { AcpCapabilities } from "../src/acp-run.js";
import { COPILOT_ACP_SESSION, createCopilotAcpCommand } from "../src/commands.js";
import type { ExecutableLaunch } from "../src/commands.js";
import type { AcpPermissionAnswer, AcpPermissionRequest, AcpRun, AcpRunOptions } from "../src/acp-run.js";
import type { RuntimeJsonlRecord } from "../src/process-runner.js";

/**
 * ONE ACP TURN, AGAINST AN AGENT THAT FOLLOWS A SCRIPT (0.377).
 *
 * The fake speaks what the real ones were measured speaking (fixtures/acp/):
 * one JSON-RPC message a line, its own request ids for what it asks, and a
 * prompt answered only once the turn is over.
 */
type Message = { jsonrpc?: string; id?: number; method?: string; params?: Record<string, unknown>; result?: Record<string, unknown>; error?: { code: number; message: string } };

interface Agent {
  readonly sent: Message[];
  say(message: Message): void;
  answer(to: Message, result: Record<string, unknown>): void;
  exit(): void;
}

const AGENT_MODE = "https://agentclientprotocol.com/protocol/session-modes#agent";
const AUTOPILOT = "https://agentclientprotocol.com/protocol/session-modes#autopilot";
const SESSION = {
  sessionId: "s1",
  modes: { currentModeId: AGENT_MODE, availableModes: [{ id: AGENT_MODE, name: "Agent" }, { id: AUTOPILOT, name: "Autopilot" }] },
  configOptions: [{ type: "select", id: "allow_all", currentValue: "off", options: [{ value: "on" }, { value: "off" }] }],
};
const OPTIONS = [
  { optionId: "yes", kind: "allow_once", name: "Allow once" },
  { optionId: "yes-forever", kind: "allow_always", name: "Always allow" },
  { optionId: "no", kind: "reject_once", name: "Deny" },
];

function run(
  script: (message: Message, agent: Agent) => void,
  options: Partial<AcpRunOptions> = {},
): { readonly acp: AcpRun; readonly agent: Agent; readonly spawned: Array<{ args: readonly string[]; cwd: string | undefined }> } {
  let deliver: (chunk: string) => void = () => undefined;
  let exited: () => void = () => undefined;
  const spawned: Array<{ args: readonly string[]; cwd: string | undefined }> = [];
  const agent: Agent = {
    sent: [],
    say: (message) => queueMicrotask(() => deliver(`${JSON.stringify({ jsonrpc: "2.0", ...message })}\n`)),
    answer: (to, result) => agent.say({ id: to.id!, result }),
    exit: () => exited(),
  };
  const acp = startAcpRun({
    spawn: (_executablePath, args, _env, cwd) => {
      spawned.push({ args, cwd });
      return {
        write: (line) => {
          const message = JSON.parse(line) as Message;
          agent.sent.push(message);
          script(message, agent);
        },
        kill: () => undefined,
        onData: (listener) => {
          deliver = listener;
        },
        onExit: (listener) => {
          exited = listener;
        },
      };
    },
    command: { executablePath: "copilot", args: ["--acp"], cwd: "C:\\work\\pebble", stdin: "protocol", sandbox: "workspace-write" } as AcpRunOptions["command"],
    prompt: "Change the port to 3001.",
    modeId: AGENT_MODE,
    requiredConfig: { allow_all: "off" },
    ...options,
  });
  return { acp, agent, spawned };
}

/** The usual agent: starts, and hands each prompt to `turn`. */
function agentWith(turn: (prompt: Message, agent: Agent) => void, extra: (message: Message, agent: Agent) => boolean = () => false) {
  return (message: Message, agent: Agent): void => {
    if (extra(message, agent)) return;
    if (message.method === "initialize") agent.answer(message, { protocolVersion: 1, agentCapabilities: { loadSession: true } });
    else if (message.method === "session/new") agent.answer(message, SESSION);
    else if (message.method === "session/prompt") turn(message, agent);
  };
}

const update = (update: Record<string, unknown>): Message => ({ method: "session/update", params: { sessionId: "s1", update } });
const ended = (agent: Agent, prompt: Message, stopReason = "end_turn"): void => agent.answer(prompt, { stopReason, usage: { inputTokens: 10, outputTokens: 2 } });

async function recordsOf(acp: AcpRun): Promise<RuntimeJsonlRecord[]> {
  await acp.completion;
  const out: RuntimeJsonlRecord[] = [];
  for await (const record of acp.records) out.push(record);
  return out;
}
const methods = (records: readonly RuntimeJsonlRecord[]): string[] => records.map((record) => (JSON.parse(record.raw) as { method: string }).method);
const said = (records: readonly RuntimeJsonlRecord[]): unknown[] => {
  const normalizer = createAcpEventNormalizer({ runId: "run_1", runtime: "copilot" });
  return records.flatMap((record) => normalizer.accept(record)).filter((event) => event.type === "message.delta" && (event.payload as { final: boolean }).final).map((event) => (event.payload as { text: string }).text);
};

describe("an ACP run", () => {
  it("offers the agent nothing, starts in the folder it names, and records the turn", async () => {
    const { acp, agent, spawned } = run(agentWith((prompt, agent) => {
      agent.say(update({ sessionUpdate: "agent_message_chunk", content: { type: "text", text: "Done." } }));
      ended(agent, prompt);
    }));
    const records = await recordsOf(acp);
    expect(agent.sent.find((message) => message.method === "initialize")?.params).toEqual({
      protocolVersion: 1,
      clientCapabilities: { fs: { readTextFile: false, writeTextFile: false }, terminal: false },
    });
    // The process is started IN the folder as well as told it.
    expect(spawned[0]?.cwd).toBe("C:\\work\\pebble");
    expect(agent.sent.find((message) => message.method === "session/new")?.params).toEqual({ cwd: "C:\\work\\pebble", mcpServers: [] });
    expect(methods(records)).toEqual([ACP_SESSION, "session/update", ACP_PROMPT_RESULT]);
    expect(said(records)).toEqual(["Done."]);
    await expect(acp.completion).resolves.toMatchObject({ exitCode: 0, cancelled: false });
  });

  it("never records the machine's own commands, or anything said for another session", async () => {
    const { acp } = run(agentWith((prompt, agent) => {
      agent.say(update({ sessionUpdate: "available_commands_update", availableCommands: [{ name: "my-private-skill", description: "the operator's own" }] }));
      agent.say({ method: "session/update", params: { sessionId: "s2", update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "not this run" } } } });
      agent.say(update({ sessionUpdate: "agent_message_chunk", content: { type: "text", text: "Done." } }));
      ended(agent, prompt);
    }));
    const records = await recordsOf(acp);
    const kept = records.map((record) => record.raw).join("\n");
    expect(kept).not.toContain("my-private-skill");
    expect(kept).not.toContain("not this run");
    expect(said(records)).toEqual(["Done."]);
  });

  it("continues a session by loading it, and keeps none of what the load replays", async () => {
    const { acp, agent } = run(
      agentWith((prompt, agent) => {
        agent.say(update({ sessionUpdate: "agent_message_chunk", content: { type: "text", text: "ACP-OK-7 PLUM-3" } }));
        ended(agent, prompt);
      }, (message, agent) => {
        if (message.method !== "session/load") return false;
        // As Copilot does: the whole conversation first, then the answer.
        agent.say(update({ sessionUpdate: "user_message_chunk", content: { type: "text", text: "Remember the word PLUM-3." } }));
        agent.say(update({ sessionUpdate: "agent_message_chunk", content: { type: "text", text: "OK" } }));
        agent.answer(message, { modes: SESSION.modes, configOptions: SESSION.configOptions });
        return true;
      }),
      { resumeSessionId: "s1" },
    );
    const records = await recordsOf(acp);
    expect(agent.sent.find((message) => message.method === "session/load")?.params).toEqual({ sessionId: "s1", cwd: "C:\\work\\pebble", mcpServers: [] });
    expect(said(records)).toEqual(["ACP-OK-7 PLUM-3"]);
    expect(records.map((record) => record.raw).join("")).not.toContain("PLUM-3.");
  });

  // Since 0.616 the run remembers nothing: an Always was answered here, before
  // the saved rules were read. Every request goes to the host, which keeps the
  // Always and decides after the rules (apps/desktop shared/who-decides.ts).
  it("answers by the KIND of option, never says 'always' to the agent, and asks the host about every request", async () => {
    const asked: AcpPermissionRequest[] = [];
    const ask = (id: number, command: string): Message => ({
      id,
      method: "session/request_permission",
      params: { sessionId: "s1", toolCall: { toolCallId: `t${String(id)}`, title: "Run the tests", kind: "execute", rawInput: { command } }, options: OPTIONS },
    });
    const { acp, agent } = run(
      agentWith((prompt, agent) => {
        agent.say(ask(100, "npm test"));
        agent.say(ask(101, "npm test"));
        agent.say(ask(102, "rm -rf build"));
        // The turn ends once all three have been answered.
        const waitForAnswers = (): void => {
          const answered = agent.sent.filter((message) => message.id !== undefined && message.id >= 100 && message.result !== undefined);
          if (answered.length === 3) ended(agent, prompt);
          else setTimeout(waitForAnswers, 1);
        };
        waitForAnswers();
      }),
      {
        onPermission: async (request): Promise<AcpPermissionAnswer> => {
          asked.push(request);
          return request.command === "npm test" ? "allow_always" : "reject_once";
        },
      },
    );
    const records = await recordsOf(acp);
    const answers = new Map(agent.sent.filter((message) => message.id !== undefined && message.id >= 100).map((message) => [message.id, message.result]));
    expect(answers.get(100)).toEqual({ outcome: { outcome: "selected", optionId: "yes" } });
    // The second, identical, is put to the host too -- once the first was answered, so a
    // different command may be asked in between -- and answered "once" again.
    expect(answers.get(101)).toEqual({ outcome: { outcome: "selected", optionId: "yes" } });
    expect(asked.map((request) => request.command).sort()).toEqual(["npm test", "npm test", "rm -rf build"]);
    expect(answers.get(102)).toEqual({ outcome: { outcome: "selected", optionId: "no" } });
    expect(JSON.stringify([...answers.values()])).not.toContain("yes-forever");
    // The refusal is on the record, for the normalizer to call it declined.
    expect(records.filter((record) => methods([record])[0] === ACP_DECLINED).map((record) => record.raw)).toEqual([JSON.stringify({ method: ACP_DECLINED, params: { toolCallId: "t102" } })]);
  });

  it("writes a refusal down BEFORE the answer goes back, so it comes before the failed call it explains", async () => {
    const { acp } = run(
      agentWith((prompt, agent) => {
        agent.say(update({ sessionUpdate: "tool_call", toolCallId: "t1", kind: "execute", status: "pending", rawInput: { command: "echo hi" } }));
        agent.say({ id: 7, method: "session/request_permission", params: { sessionId: "s1", toolCall: { toolCallId: "t1", kind: "execute", rawInput: { command: "echo hi" } }, options: OPTIONS } });
      }, (message, agent) => {
        if (message.id !== 7 || message.result === undefined) return false;
        agent.say(update({ sessionUpdate: "tool_call_update", toolCallId: "t1", status: "failed", rawOutput: { message: "The user rejected this tool call." } }));
        agent.say({ id: 3, result: { stopReason: "end_turn" } });
        return true;
      }),
      { onPermission: async () => "reject_once" },
    );
    const records = await recordsOf(acp);
    const order = records.map((record) => JSON.parse(record.raw) as { method: string; params: { update?: { status?: string } } });
    const declinedAt = order.findIndex((record) => record.method === ACP_DECLINED);
    const failedAt = order.findIndex((record) => record.params.update?.status === "failed");
    expect(declinedAt).toBeGreaterThanOrEqual(0);
    expect(declinedAt).toBeLessThan(failedAt);
  });

  it("a turn the agent ends 'cancelled' on the host's own refusal finished, declined -- it was not stopped (OpenCode 2.1.0)", async () => {
    const { acp } = run(
      agentWith((prompt, agent) => {
        agent.say(update({ sessionUpdate: "tool_call", toolCallId: "t1", kind: "execute", status: "pending", rawInput: { command: "echo hi" } }));
        agent.say({ id: 7, method: "session/request_permission", params: { sessionId: "s1", toolCall: { toolCallId: "t1", kind: "execute", rawInput: { command: "echo hi" } }, options: OPTIONS } });
      }, (message, agent) => {
        if (message.id !== 7 || message.result === undefined) return false;
        agent.say(update({ sessionUpdate: "tool_call_update", toolCallId: "t1", status: "failed", rawOutput: { error: "The user declined this tool call" } }));
        agent.say({ id: 3, result: { stopReason: "cancelled" } });
        return true;
      }),
      { onPermission: async () => "reject_once" },
    );
    await expect(acp.completion).resolves.toMatchObject({ cancelled: false, exitCode: 0 });
    const records = await recordsOf(acp);
    const normalizer = createAcpEventNormalizer({ runId: "run_1", runtime: "opencode" });
    records.forEach((record) => normalizer.accept(record));
    const types = normalizer.finish(await acp.completion).map((event) => event.type);
    expect(types).not.toContain("run.cancelled");
    expect(types).toContain("run.completed");
  });

  it("a turn the agent ends 'cancelled' with nothing refused still reads as stopped (control)", async () => {
    const { acp } = run(agentWith((prompt, agent) => ended(agent, prompt, "cancelled")));
    const records = await recordsOf(acp);
    const normalizer = createAcpEventNormalizer({ runId: "run_1", runtime: "opencode" });
    records.forEach((record) => normalizer.accept(record));
    expect(normalizer.finish(await acp.completion).map((event) => event.type)).toContain("run.cancelled");
  });

  it("refuses anything else the agent asks -- files, a terminal -- as a method it was never offered", async () => {
    const { acp, agent } = run(agentWith((prompt, agent) => {
      agent.say({ id: 50, method: "fs/read_text_file", params: { sessionId: "s1", path: "C:\\Users\\person\\.ssh\\id_rsa" } });
      agent.say({ id: 51, method: "terminal/create", params: { sessionId: "s1", command: "whoami" } });
      setTimeout(() => ended(agent, prompt), 5);
    }));
    await acp.completion;
    for (const id of [50, 51]) {
      expect(agent.sent.find((message) => message.id === id)?.error?.code).toBe(-32601);
    }
  });

  it("on Stop, cancels the turn the protocol's way and answers the open question 'cancelled'", async () => {
    const controller = new AbortController();
    let prompt: Message | undefined;
    const { acp, agent } = run(
      agentWith((message, agent) => {
        prompt = message;
        agent.say({ id: 9, method: "session/request_permission", params: { sessionId: "s1", toolCall: { toolCallId: "t9", kind: "execute", rawInput: { command: "npm publish" } }, options: OPTIONS } });
      }, (message, agent) => {
        // The prompt ends once the open question has been answered, as the spec has it.
        if (message.id === 9 && message.result !== undefined) {
          ended(agent, prompt!, "cancelled");
          return true;
        }
        return message.method === "session/cancel";
      }),
      {
        signal: controller.signal,
        onPermission: () => {
          // The card is up, and the person presses Stop instead of answering.
          setTimeout(() => controller.abort(), 1);
          return new Promise<AcpPermissionAnswer>(() => undefined);
        },
      },
    );
    await expect(acp.completion).resolves.toMatchObject({ cancelled: true, exitCode: null });
    expect(agent.sent.some((message) => message.method === "session/cancel" && message.params?.sessionId === "s1")).toBe(true);
    expect(agent.sent.find((message) => message.id === 9)?.result).toEqual({ outcome: { outcome: "cancelled" } });
  });

  it("ends a stopped turn anyway when the agent never says it stopped", async () => {
    const controller = new AbortController();
    const { acp } = run(agentWith(() => setTimeout(() => controller.abort(), 1)), { signal: controller.signal, cancelGraceMs: 10 });
    await expect(acp.completion).resolves.toMatchObject({ cancelled: true });
  });

  it("puts a session back into the mode that asks, and will not run one that cannot be", async () => {
    const autopilot = { ...SESSION, modes: { ...SESSION.modes, currentModeId: AUTOPILOT } };
    const switched = run(agentWith((prompt, agent) => ended(agent, prompt), (message, agent) => {
      if (message.method === "session/new") agent.answer(message, autopilot);
      else if (message.method === "session/set_mode") agent.answer(message, {});
      else return false;
      return true;
    }));
    await switched.acp.completion;
    const setMode = switched.agent.sent.findIndex((message) => message.method === "session/set_mode");
    expect(switched.agent.sent[setMode]?.params).toEqual({ sessionId: "s1", modeId: AGENT_MODE });
    expect(setMode).toBeLessThan(switched.agent.sent.findIndex((message) => message.method === "session/prompt"));

    const noAgentMode = { ...SESSION, modes: { currentModeId: AUTOPILOT, availableModes: [{ id: AUTOPILOT, name: "Autopilot" }] } };
    const refused = run(agentWith(() => undefined, (message, agent) => {
      if (message.method !== "session/new") return false;
      agent.answer(message, noAgentMode);
      return true;
    }));
    await expect(refused.acp.completion).resolves.toMatchObject({ exitCode: null, stderr: expect.stringContaining("did not offer the mode") as unknown });
    expect(refused.agent.sent.some((message) => message.method === "session/prompt")).toBe(false);
  });

  it("turns allow-all off before the prompt, and will not run an agent that keeps it on", async () => {
    const allowAll = (value: string) => [{ ...SESSION.configOptions[0], currentValue: value }];
    const fixed = run(agentWith((prompt, agent) => ended(agent, prompt), (message, agent) => {
      if (message.method === "session/new") agent.answer(message, { ...SESSION, configOptions: allowAll("on") });
      else if (message.method === "session/set_config_option") agent.answer(message, { configOptions: allowAll("off") });
      else return false;
      return true;
    }));
    await expect(fixed.acp.completion).resolves.toMatchObject({ exitCode: 0 });
    expect(fixed.agent.sent.find((message) => message.method === "session/set_config_option")?.params).toEqual({ sessionId: "s1", configId: "allow_all", value: "off" });

    const stubborn = run(agentWith(() => undefined, (message, agent) => {
      if (message.method === "session/new") agent.answer(message, { ...SESSION, configOptions: allowAll("on") });
      else if (message.method === "session/set_config_option") agent.answer(message, { configOptions: allowAll("on") });
      else return false;
      return true;
    }));
    await expect(stubborn.acp.completion).resolves.toMatchObject({ exitCode: null, stderr: expect.stringContaining("allow_all") as unknown });
    expect(stubborn.agent.sent.some((message) => message.method === "session/prompt")).toBe(false);
  });

  it("sends what it was handed during a turn as the next prompt, once that turn ends", async () => {
    let acpRun: AcpRun | undefined;
    const prompts: string[] = [];
    const { acp } = run(agentWith((prompt, agent) => {
      const said = ((prompt.params?.prompt as Array<{ text: string }>)[0]!).text;
      prompts.push(said);
      if (prompts.length === 1) {
        // The person denies a step and says why while the turn is running.
        void acpRun!.steer("I declined that. Use npm test instead.").then(() => ended(agent, prompt));
        return;
      }
      agent.say(update({ sessionUpdate: "agent_message_chunk", content: { type: "text", text: "Running npm test." } }));
      ended(agent, prompt);
    }));
    acpRun = acp;
    const records = await recordsOf(acp);
    expect(prompts).toEqual(["Change the port to 3001.", "I declined that. Use npm test instead."]);
    expect(methods(records).filter((method) => method === ACP_PROMPT_RESULT)).toHaveLength(2);
    expect(said(records)).toEqual(["Running npm test."]);
    // Once it has ended there is no next prompt to carry anything.
    await expect(acp.steer("too late")).resolves.toBe(false);
  });

  it("still sends what it was handed when the turn stops at a limit (2026-10-10 sweep)", async () => {
    let acpRun: AcpRun | undefined;
    const prompts: string[] = [];
    const { acp } = run(agentWith((prompt, agent) => {
      prompts.push(((prompt.params?.prompt as Array<{ text: string }>)[0]!).text);
      if (prompts.length === 1) {
        void acpRun!.steer("Then do the rest.").then(() => ended(agent, prompt, "max_turn_requests"));
        return;
      }
      ended(agent, prompt);
    }));
    acpRun = acp;
    await recordsOf(acp);
    expect(prompts).toEqual(["Change the port to 3001.", "Then do the rest."]);
  });

  it("an agent that dies mid-turn leaves a run that did not finish, saying so", async () => {
    const { acp } = run(agentWith((_prompt, agent) => setTimeout(() => agent.exit(), 1)));
    await expect(acp.completion).resolves.toMatchObject({ exitCode: null, cancelled: false, stderr: "The agent exited before its turn finished." });
  });

  it("says so in words when the agent cannot continue an earlier session, rather than asking it anyway", async () => {
    const { acp, agent } = run((message, agent) => {
      if (message.method === "initialize") agent.answer(message, { protocolVersion: 1, agentCapabilities: { loadSession: false } });
      else if (message.method !== undefined) agent.say({ id: message.id!, error: { code: -32601, message: "Method not found" } });
    }, { resumeSessionId: "s1" });
    await expect(acp.completion).resolves.toMatchObject({ exitCode: null, stderr: "The agent cannot continue an earlier session." });
    expect(agent.sent.map((message) => message.method)).toEqual(["initialize"]);
  });

  it("will not speak to an agent on another version of the protocol", async () => {
    const { acp, agent } = run((message, agent) => {
      if (message.method === "initialize") agent.answer(message, { protocolVersion: 2 });
    });
    await expect(acp.completion).resolves.toMatchObject({ exitCode: null, stderr: expect.stringContaining("version 2") as unknown });
    expect(agent.sent.map((message) => message.method)).toEqual(["initialize"]);
  });
});

describe("what an ACP agent says it can do (W12)", () => {
  it("reads the protocol's own keys, and only true counts", () => {
    expect(acpCapabilitiesOf({ loadSession: true, promptCapabilities: { image: true, audio: false, embeddedContext: true }, mcpCapabilities: { http: true, sse: "yes" } })).toEqual({
      continuesSessions: true,
      images: true,
      audio: false,
      embeddedContext: true,
      mcpOverHttp: true,
      mcpOverSse: false,
    });
  });

  it("reads what Copilot 1.0.88 really answered (fixtures/acp), its sessionCapabilities left out", async () => {
    const { readFile } = await import("node:fs/promises");
    const lines = (await readFile(new URL("./fixtures/acp/copilot-1.0.88-load-approved.jsonl", import.meta.url), "utf8")).split("\n");
    const line = lines.find((entry) => entry.includes("agentCapabilities"));
    expect(line).toBeDefined();
    const found = (value: unknown): unknown => {
      if (typeof value !== "object" || value === null) return undefined;
      const record = value as Record<string, unknown>;
      if ("agentCapabilities" in record) return record.agentCapabilities;
      for (const inner of Object.values(record)) {
        const deeper = found(inner);
        if (deeper !== undefined) return deeper;
      }
      return undefined;
    };
    expect(acpCapabilitiesOf(found(JSON.parse(line!)))).toEqual({
      continuesSessions: true,
      images: true,
      audio: false,
      embeddedContext: true,
      mcpOverHttp: true,
      mcpOverSse: true,
    });
  });

  it("leaves out keys it does not know, and reads nothing from a malformed answer", () => {
    const read = acpCapabilitiesOf({ loadSession: true, teleport: true, _meta: { anything: true }, promptCapabilities: { video: true } });
    expect(Object.keys(read).sort()).toEqual(["audio", "continuesSessions", "embeddedContext", "images", "mcpOverHttp", "mcpOverSse"]);
    expect(read.continuesSessions).toBe(true);
    for (const malformed of [undefined, null, "all", [true], { promptCapabilities: "image" }]) {
      expect(Object.values(acpCapabilitiesOf(malformed)).every((value) => value === false)).toBe(true);
    }
  });

  it("is handed over once the agent answers on this protocol's version", async () => {
    const seen: AcpCapabilities[] = [];
    const { acp } = run(agentWith((prompt, agent) => ended(agent, prompt)), { onCapabilities: (capabilities) => seen.push(capabilities) });
    await acp.completion;
    expect(seen).toEqual([{ continuesSessions: true, images: false, audio: false, embeddedContext: false, mcpOverHttp: false, mcpOverSse: false }]);
  });

  it("is not handed over from an agent on another version of the protocol", async () => {
    const seen: AcpCapabilities[] = [];
    const { acp } = run((message, agent) => {
      if (message.method === "initialize") agent.answer(message, { protocolVersion: 2, agentCapabilities: { loadSession: true } });
    }, { onCapabilities: (capabilities) => seen.push(capabilities) });
    await acp.completion;
    expect(seen).toEqual([]);
  });

  it("a listener that throws does not stop the turn", async () => {
    const { acp } = run(agentWith((prompt, agent) => ended(agent, prompt)), {
      onCapabilities: () => {
        throw new Error("listener broke");
      },
    });
    await expect(acp.completion).resolves.toMatchObject({ exitCode: 0 });
  });
});

describe("what Copilot is started with, to ask", () => {
  const copilot: ExecutableLaunch = {
    commandName: "copilot",
    discoveredPath: "C:\\tools\\copilot.cmd",
    executablePath: "C:\\tools\\copilot.cmd",
    prefixArgs: [],
  };

  it("--acp and nothing that stops it asking: no --allow-all-tools, no prompt, no session on the argv", () => {
    const spec = createCopilotAcpCommand(copilot, { workspacePath: "C:\\work" });
    expect(spec.args).toEqual(["--acp", "--excluded-tools=session_store_sql"]);
    expect(spec.stdin).toBe("protocol");
    expect(spec.cwd).toBe("C:\\work");
    expect(createCopilotAcpCommand(copilot, { workspacePath: "C:\\work", model: "gpt-5.6-luna" }).args).toEqual(["--acp", "--excluded-tools=session_store_sql", "--model", "gpt-5.6-luna"]);
    // The effort the person chose rides on this route too (the 2026-10-10 sweep: it never did).
    expect(createCopilotAcpCommand(copilot, { workspacePath: "C:\\work", model: "gpt-5.6-luna", effort: "high" }).args).toEqual(["--acp", "--excluded-tools=session_store_sql", "--model", "gpt-5.6-luna", "--effort", "high"]);
  });

  it("holds the session to the mode that asks, with allow-all off -- by Copilot's own ids", () => {
    expect(COPILOT_ACP_SESSION).toEqual({ modeId: "https://agentclientprotocol.com/protocol/session-modes#agent", requiredConfig: { allow_all: "off" } });
  });
});
