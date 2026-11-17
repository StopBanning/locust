import {
  boundedAnswerText,
  evidenceFor,
  isObject,
  malformedEvidence,
  processEvidence,
  requireContextText,
  stringValue,
} from "./codex-events.js";
import type {
  CodexEventEvidence,
  NormalizedRuntimeEvent,
  NormalizedRuntimeEventType,
  NormalizedRuntimePayloadMap,
} from "./codex-events.js";
import type { RuntimeJsonlRecord, RuntimeProcessCompletion } from "./process-runner.js";
import type { ExecutableLaunch, MissionSandbox, RuntimeCommandSpec, RuntimeDiscovery } from "./types.js";

/**
 * APPLE INTELLIGENCE, ON THIS MAC (0.878). Colin, passing on a friend's question: "could you add Siri AI to Locust?"
 * Siri itself takes no messages from another app, but the model behind Apple Intelligence does: since macOS 26 any app
 * may ask it through Foundation Models, on the Mac itself, free, with no account and nothing sent away.
 *
 * The framework is Swift's alone, so a small helper the Mac app carries (`locust-apple-model`, from
 * `apps/desktop/native/apple-model/main.swift`) asks it: one JSON request on stdin, the answer as JSON lines on stdout.
 *
 * MEASURED 17 Nov 2026 on macOS 27.2.1, and the reason for everything below:
 * - The model reads 8,192 tokens at a time (`SystemLanguageModel.contextSize`). OpenCode sends a chat-only model
 *   ~13,000 tokens of instructions and tools on every turn, so "Your own models" could never carry it: this is a
 *   runtime of its own, and it only talks. No tools; Ask and Plan.
 * - Locust's usual brief made it copy `<locust-ask>` templates into its answers, so it is told less (`appleBrief`).
 * - It keeps no session: no thread id, so every follow-up starts cold and is told what was said.
 * - `streamResponse` yields the whole answer so far each time; the helper sends only what changed.
 *
 * The Rust app has the same runtime (`apps/native/core/src/runtime/apple.rs`), word for word.
 */

export const APPLE_RUNTIME = "apple" as const;
export const APPLE_DISPLAY_NAME = "Apple Intelligence";
export const APPLE_MODEL = "apple-on-device";
export const APPLE_MODEL_NAME = "On-device model";
/** The helper's file name, beside the app's other resources on a Mac. */
export const APPLE_HELPER_NAME = "locust-apple-model";

/** What a person reads when the model is not there, by the reason Foundation Models gives. */
export function appleUnavailableWords(reason: string | undefined): string {
  switch (reason) {
    case "appleIntelligenceNotEnabled":
      return "Apple Intelligence is turned off on this Mac. Turn it on in System Settings, under Apple Intelligence & Siri.";
    case "modelNotReady":
      return "Apple's on-device model is still downloading. Try again once it is ready.";
    case "deviceNotEligible":
      return "This Mac cannot run Apple Intelligence.";
    default:
      return "Apple's on-device model is not available on this Mac.";
  }
}

/** What a person reads when the model would not or could not answer, by the kind the helper names. */
export function appleFailureWords(kind: string, reason?: string): string {
  switch (kind) {
    case "contextSizeExceeded":
      return "This is more than Apple's on-device model can read at once (about 8,000 tokens, with what was said before). Start a new conversation, or carry on with another teammate.";
    case "guardrailViolation":
      return "Apple's on-device model would not answer this: its safety rules stopped it.";
    case "refusal":
      return "Apple's on-device model declined to answer this.";
    case "unsupportedLanguageOrLocale":
      return "Apple's on-device model does not read this language yet.";
    case "rateLimited":
      return "Apple's on-device model is busy. Try again in a moment.";
    case "assetsUnavailable":
      return appleUnavailableWords("modelNotReady");
    case "unavailable":
      return appleUnavailableWords(reason);
    default:
      return "Apple's on-device model could not answer.";
  }
}

/**
 * What Apple's model is told before the conversation. Not Locust's usual brief: its rules for the tags an agent writes
 * (`<locust-ask>`, `<locust-memory>`, `<locust-next>`, shares, files) and its folder and roster sections cost much of the
 * 8,000 tokens this model reads, and a model this small copies the tags into its answers as words (measured 17 Nov:
 * asked what a git branch is, it answered and then wrote out a `<locust-ask>` with the template's own placeholders).
 * So: who it is, what it cannot do, the teammate's own instructions, then what was said.
 */
export function appleBrief(name: string, role: string, instructions: string | undefined, words: string): string {
  const titled = role.trim().length === 0 ? "" : ` (${role.trim()})`;
  let out =
    `You are ${name}, a teammate in Locust${titled}. You run on Apple's on-device model, on this Mac. You can only talk: you ` +
    "cannot read or change files, run commands, or open web pages, so never say you did. Answer plainly and briefly, " +
    "in the person's language.";
  const own = instructions?.trim();
  if (own !== undefined && own.length > 0) out += `\n\nYour instructions from the person:\n${own}`;
  return `${out}\n\n${words}`;
}

/** The one line the helper reads on stdin: the whole prompt Locust composed, brief and what was said included. */
export function appleRequestLine(prompt: string): string {
  return `${JSON.stringify({ prompt })}\n`;
}

/** What the helper's `--probe` said. */
export type AppleProbe =
  | { readonly available: true; readonly context: number }
  | { readonly available: false; readonly reason: string };

/**
 * Reads a `--probe` answer. Anything else -- a helper that would not start, on a Mac too old for the framework, says
 * nothing JSON -- is unavailable for a reason nobody knows.
 */
export function readAppleProbe(stdout: string): AppleProbe {
  for (const line of stdout.split(/\r?\n/)) {
    let value: unknown;
    try {
      value = JSON.parse(line) as unknown;
    } catch {
      continue;
    }
    if (!isObject(value)) continue;
    if (value.available === true) return { available: true, context: typeof value.contextSize === "number" ? value.contextSize : 0 };
    return { available: false, reason: stringValue(value.reason) ?? "unknown" };
  }
  return { available: false, reason: "unknown" };
}

/**
 * Apple Intelligence as discovery reports a runtime: ready when the model answers; installed but unavailable, with the
 * Mac's own reason in words, when the Mac says why it does not (turned off, still downloading, a Mac that cannot run
 * it); and nothing at all where there is no helper to ask or it could not say -- no row is better than a broken one.
 */
export function appleDiscoveryRecord(probe: AppleProbe, helper: ExecutableLaunch | undefined): RuntimeDiscovery | undefined {
  if (helper === undefined) return undefined;
  if (!probe.available && probe.reason === "unknown") return undefined;
  const base = {
    id: APPLE_RUNTIME,
    kind: "agent-runtime" as const,
    displayName: APPLE_DISPLAY_NAME,
    optional: true,
    availability: "available" as const,
    executable: helper,
    supportedFeatures: [],
    requiredFeatures: [],
  };
  if (probe.available) return { ...base, readiness: "ready", diagnostics: [] };
  return {
    ...base,
    readiness: "unsupported",
    diagnostics: [{ code: "version-unsupported", severity: "info", message: appleUnavailableWords(probe.reason) }],
  };
}

/** The modes this runtime runs in: it has no tools, so only a run that changes nothing (Ask, and Plan). */
export function appleRefusesSandbox(sandbox: MissionSandbox | undefined): string | undefined {
  return sandbox === undefined || sandbox === "read-only"
    ? undefined
    : "Apple's on-device model only talks: it cannot read or change files, so it runs in Ask mode only.";
}

/** The helper, run for one answer: the request goes on stdin (`apple-json`), the answer comes back as JSON lines. */
export function createAppleModelCommand(
  executable: ExecutableLaunch,
  options: { readonly workspacePath: string; readonly sandbox?: MissionSandbox },
): RuntimeCommandSpec {
  const refused = appleRefusesSandbox(options.sandbox);
  if (refused !== undefined) throw new Error(refused);
  if (executable.executablePath.trim().length === 0) throw new Error("Executable path must be non-empty");
  if (options.workspacePath.trim().length === 0) throw new Error("Workspace path must be non-empty");
  return {
    runtime: APPLE_RUNTIME,
    executablePath: executable.executablePath,
    args: [...executable.prefixArgs],
    cwd: options.workspacePath,
    stdin: "apple-json",
    stdout: "jsonl",
    sandbox: "read-only",
  };
}

export interface AppleInvocationContext {
  /** Product-owned run identifier. */
  readonly runId: string;
  readonly missionId?: string;
  readonly requestedRouteId?: string;
  readonly resolvedRouteId?: string;
  /** Test seam and host clock. */
  readonly now?: () => Date;
}

export interface AppleEventNormalizer {
  /** Always undefined: the model keeps no session, so every follow-up starts cold. */
  readonly runtimeThreadId: undefined;
  readonly finalized: boolean;
  accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[];
  finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[];
}

/**
 * The helper's lines -> product events. `delta` appends to the answer, `replace` rewrites it, `done` keeps it whole
 * (the final replace) and closes the step; `error` names the kind, said in words when the run ends.
 */
export function createAppleEventNormalizer(context: AppleInvocationContext): AppleEventNormalizer {
  const runId = requireContextText(context.runId, "runId");
  const now = context.now ?? (() => new Date());
  const MESSAGE_ITEM = "msg_0";
  let sequence = 0;
  let finalized = false;
  let stepOpen = false;
  let done = false;
  let said = "";
  let failure: { readonly kind: string; readonly reason?: string } | undefined;

  const emit = <TType extends NormalizedRuntimeEventType>(
    type: TType,
    payload: NormalizedRuntimePayloadMap[TType],
  ): NormalizedRuntimeEvent => {
    sequence += 1;
    return {
      id: `${runId}:apple:${sequence}`,
      runId,
      ...(context.missionId === undefined ? {} : { missionId: context.missionId }),
      sequence,
      occurredAt: now().toISOString(),
      sourceAdapter: APPLE_RUNTIME,
      ...(context.requestedRouteId === undefined ? {} : { requestedRouteId: context.requestedRouteId }),
      ...(context.resolvedRouteId === undefined ? {} : { resolvedRouteId: context.resolvedRouteId }),
      type,
      payload,
    } as NormalizedRuntimeEvent;
  };

  const openStep = (out: NormalizedRuntimeEvent[], evidence: CodexEventEvidence): void => {
    if (stepOpen) return;
    stepOpen = true;
    out.push(emit("step.started", { stepKind: "turn", evidence }));
  };

  return {
    get runtimeThreadId(): undefined {
      return undefined;
    },
    get finalized() {
      return finalized;
    },

    accept(record: RuntimeJsonlRecord): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      let parsed: unknown;
      try {
        parsed = JSON.parse(record.raw) as unknown;
      } catch {
        return [
          emit("adapter.diagnostic", {
            level: "warning",
            code: "apple.malformed_record",
            message: "A line from Apple's on-device model could not be read.",
            terminal: false,
            evidence: malformedEvidence(record),
          }),
        ];
      }
      if (!isObject(parsed)) return [];
      const type = stringValue(parsed.type) ?? "";
      const text = typeof parsed.text === "string" ? parsed.text : "";
      const evidence = evidenceFor(record, parsed, type);
      const out: NormalizedRuntimeEvent[] = [];
      if (type === "delta" && text.length > 0) {
        openStep(out, evidence);
        said += text;
        out.push(emit("message.delta", { itemId: MESSAGE_ITEM, operation: "append", text: boundedAnswerText(text), final: false, evidence }));
      } else if (type === "replace") {
        openStep(out, evidence);
        said = text;
        out.push(emit("message.delta", { itemId: MESSAGE_ITEM, operation: "replace", text: boundedAnswerText(text), final: false, evidence }));
      } else if (type === "done") {
        openStep(out, evidence);
        done = true;
        const whole = text.length === 0 ? said : text;
        if (whole.length > 0) {
          out.push(emit("message.delta", { itemId: MESSAGE_ITEM, operation: "replace", text: boundedAnswerText(whole), final: true, evidence }));
        }
        out.push(emit("step.completed", { stepKind: "turn", evidence }));
        stepOpen = false;
      } else if (type === "error") {
        const reason = stringValue(parsed.reason);
        failure = { kind: stringValue(parsed.kind) ?? "other", ...(reason === undefined ? {} : { reason }) };
      }
      return out;
    },

    finish(completion: RuntimeProcessCompletion): readonly NormalizedRuntimeEvent[] {
      if (finalized) return [];
      finalized = true;
      const process = processEvidence(completion);
      if (completion.cancelled) return [emit("run.cancelled", { process })];
      if (done && failure === undefined && completion.exitCode === 0) return [emit("run.completed", { process })];
      const message = completion.outputLimitExceeded
        ? "Apple's on-device model sent more than Locust could record, so the run was stopped rather than leave a gap in its record."
        : failure !== undefined
          ? appleFailureWords(failure.kind, failure.reason)
          : done
            ? `Apple's on-device model answered, then its helper exited with code ${String(completion.exitCode)}.`
            : "Apple's on-device model stopped before it answered.";
      return [
        emit("run.failed", {
          kind: "process-failed",
          message,
          runtimeTerminal: done ? "completed" : "failed",
          process,
        }),
      ];
    },
  };
}
