import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, it, vi } from 'vitest'

import { codexRefusalFor, createCodexAppServerCommand, startCodexAppServerRun } from '@teammate/runtime-adapters'
import type { AppServerRequest, AppServerRunProcess, JsonValue } from '@teammate/runtime-adapters'

import { createApprovalChannel, protocolAnswerFor, protocolDecisionFor } from './approval-channel.js'
import type { MissionApprovalDecision } from '../shared/ipc.js'

/**
 * EVERY "NO" LOCUST SENDS CODEX IS ONE CODEX TAKES (Codex 0.172.0).
 *
 * Locust answered a person's Deny with `"reject"`, a value in no schema of
 * Codex's: the v2 approvals take `decline` or `cancel`. Each decision Locust
 * can send is checked here against Codex's own generated response schemas,
 * checked in under `fixtures/codex-0.172.0/`, so the next Codex that moves
 * fails this file instead of a person's turn.
 *
 * Deny is `decline`, not `cancel`: the card's Deny is "don't do that, carry
 * on", and a reason the person gave reaches a Codex run as the turn's next
 * input (`onDeniedSaying`, a-denial-can-say-why.test.ts), which needs the turn
 * still running. `cancel` would interrupt it.
 */

type Schema = { readonly oneOf: readonly Variant[] }
type Shape = { readonly type?: string; readonly required?: readonly string[]; readonly properties?: Readonly<Record<string, { readonly type?: string }>> }
type Variant = { readonly enum?: readonly string[]; readonly required?: readonly string[]; readonly properties?: Readonly<Record<string, Shape>> }

/** The decision type a response schema names: `properties.decision`'s `$ref`, resolved in its own definitions. */
function decisionSchema(file: string): Schema {
  const path = fileURLToPath(new URL(`./fixtures/codex-0.172.0/${file}.json`, import.meta.url))
  const schema = JSON.parse(readFileSync(path, 'utf8')) as {
    readonly properties: { readonly decision: { readonly $ref: string } }
    readonly definitions: Readonly<Record<string, Schema>>
  }
  const name = schema.properties.decision.$ref.replace('#/definitions/', '')
  const found = schema.definitions[name]
  if (found === undefined) throw new Error(`${file} names ${name}, which it does not define`)
  return found
}

const COMMAND = 'CommandExecutionRequestApprovalResponse'
const FILE_CHANGE = 'FileChangeRequestApprovalResponse'
const LEGACY_EXEC = 'ExecCommandApprovalResponse'
const LEGACY_PATCH = 'ApplyPatchApprovalResponse'
const SCHEMA_FOR: Readonly<Record<string, string>> = {
  'item/commandExecution/requestApproval': COMMAND,
  'item/fileChange/requestApproval': FILE_CHANGE,
  execCommandApproval: LEGACY_EXEC,
  applyPatchApproval: LEGACY_PATCH
}

function typeFits(value: unknown, type: string | undefined): boolean {
  if (type === 'string') return typeof value === 'string'
  if (type === 'array') return Array.isArray(value)
  if (type === 'object') return typeof value === 'object' && value !== null && !Array.isArray(value)
  return true
}

/**
 * Whether `decision` is one of the schema's variants: a string in a variant's
 * `enum`, or an object with exactly the key a variant requires (each says
 * `additionalProperties: false`), carrying that key's required fields.
 */
function fits(decision: unknown, schema: Schema): boolean {
  return schema.oneOf.some((variant) => {
    if (variant.enum !== undefined) return typeof decision === 'string' && variant.enum.includes(decision)
    if (typeof decision !== 'object' || decision === null || Array.isArray(decision) || variant.properties === undefined) return false
    const props = variant.properties
    const given = decision as Record<string, unknown>
    if (!(variant.required ?? []).every((key) => key in given) || !Object.keys(given).every((key) => key in props)) return false
    return Object.entries(given).every(([key, inner]) => {
      const shape = props[key]!
      return typeFits(inner, shape.type) &&
        (shape.required ?? []).every((field) => typeFits((inner as Record<string, unknown>)[field], shape.properties?.[field]?.type) && (inner as Record<string, unknown>)[field] !== undefined)
    })
  })
}

/** A result Locust sends Codex: only `decision`, and a decision the asking method's schema takes. */
function expectCodexTakes(result: JsonValue, file: string): void {
  expect(Object.keys(result as Record<string, unknown>)).toEqual(['decision'])
  const decision = (result as { decision: unknown }).decision
  expect(fits(decision, decisionSchema(file)), `${file} does not take ${JSON.stringify(decision)}`).toBe(true)
}

// Every answer a person can give: a new one fails to type-check here until it is listed.
const EVERY_DECISION = ['approve-once', 'approve-always', 'deny'] as const satisfies readonly MissionApprovalDecision[]
const _listed: Record<MissionApprovalDecision, true> = { 'approve-once': true, 'approve-always': true, deny: true }
void _listed

function channel() {
  const approvals: { approvalId: string }[] = []
  const instance = createApprovalChannel({
    emitApproval: (request) => approvals.push(request),
    emitUpdate: () => undefined,
    createId: (() => { let n = 0; return () => String(++n) })(),
    now: () => new Date('2026-11-27T00:00:00.000Z')
  })
  const handler = instance.requestHandlerFor({ runId: 'run_1', missionId: 'mission_1', cwd: '/work', changesByItem: new Map() })
  const ask = (method: string, params: Record<string, unknown> = {}): Promise<JsonValue> =>
    handler({ id: 1, method, params: { threadId: 't1', itemId: 'i1', cwd: '/work', ...params } } as AppServerRequest)
  return { instance, approvals, ask }
}

describe('the check itself', () => {
  it('passes what Codex 0.172 takes and fails what Locust used to send', () => {
    expect(fits('decline', decisionSchema(COMMAND))).toBe(true)
    expect(fits({ acceptWithExecpolicyAmendment: { execpolicy_amendment: ['ls'] } }, decisionSchema(COMMAND))).toBe(true)
    expect(fits('reject', decisionSchema(COMMAND))).toBe(false)
    expect(fits('reject', decisionSchema(FILE_CHANGE))).toBe(false)
    expect(fits({ acceptWithExecpolicyAmendment: {} }, decisionSchema(COMMAND))).toBe(false)
    expect(fits('abort', decisionSchema(LEGACY_EXEC))).toBe(true)
    expect(fits({ denied: { rejection: 'no' } }, decisionSchema(LEGACY_PATCH))).toBe(true)
    expect(fits('denied', decisionSchema(LEGACY_EXEC))).toBe(false)
    expect(fits({ denied: {} }, decisionSchema(LEGACY_EXEC))).toBe(false)
    expect(fits({ denied: { rejection: 'no' }, extra: 1 }, decisionSchema(LEGACY_EXEC))).toBe(false)
  })
})

describe("a person's answer, as Codex is sent it", () => {
  it('is in both v2 schemas, for every answer', () => {
    for (const file of [COMMAND, FILE_CHANGE]) {
      for (const decision of EVERY_DECISION) {
        expectCodexTakes({ decision: protocolDecisionFor(decision) }, file)
        expectCodexTakes(protocolAnswerFor({ approvalId: 'ap_1', decision }), file)
      }
    }
  })

  it('is decline for Deny, so the turn carries on and a reason can still be steered into it', () => {
    expect(protocolDecisionFor('deny')).toBe('decline')
  })

  it('is what a Deny on the card sends, with a reason or without', async () => {
    for (const [method, file] of [['item/commandExecution/requestApproval', COMMAND], ['item/fileChange/requestApproval', FILE_CHANGE]] as const) {
      for (const reason of [undefined, 'use the build script instead']) {
        const { instance, approvals, ask } = channel()
        const waiting = ask(method, { command: 'rm -rf build', changes: [] })
        await vi.waitFor(() => {
          expect(approvals).toHaveLength(1)
        })
        instance.decide({ approvalId: approvals[0]!.approvalId, decision: 'deny', ...(reason === undefined ? {} : { reason }) })
        const sent = await waiting
        expectCodexTakes(sent, file)
        expect(sent).toEqual({ decision: 'decline' })
      }
    }
  })
})

describe("Locust's own refusal, as Codex is sent it", () => {
  it('is in the schema of the method that asked, v2 and legacy', () => {
    for (const [method, file] of Object.entries(SCHEMA_FOR)) expectCodexTakes(codexRefusalFor(method), file)
  })

  it('refuses a legacy request it never puts on a card in the legacy shape', async () => {
    for (const method of ['execCommandApproval', 'applyPatchApproval']) {
      const { approvals, ask } = channel()
      expectCodexTakes(await ask(method, { command: ['rm', '-rf', 'build'] }), SCHEMA_FOR[method]!)
      expect(approvals).toHaveLength(0)
    }
  })

  it('releases a card a finished run was still asking with a decision Codex takes', async () => {
    const { instance, approvals, ask } = channel()
    const waiting = ask('item/commandExecution/requestApproval', { command: 'x' })
    await vi.waitFor(() => {
      expect(approvals).toHaveLength(1)
    })
    instance.release('run_1')
    expectCodexTakes(await waiting, COMMAND)
  })

  it('is what a run with no one to ask sends on the wire, for every kind of approval', async () => {
    // startCodexAppServerRun's own default, the answer in every mode but Approve each.
    const written: Record<string, unknown>[] = []
    let emit: (chunk: string) => void = () => undefined
    const process: AppServerRunProcess = {
      write: (line) => {
        for (const part of line.split('\n')) if (part.trim().length > 0) written.push(JSON.parse(part) as Record<string, unknown>)
      },
      kill: () => undefined,
      onData: (listener) => {
        emit = listener
      },
      onExit: () => undefined
    }
    startCodexAppServerRun({
      spawn: () => process,
      command: createCodexAppServerCommand({ commandName: 'codex', discoveredPath: '/usr/bin/codex', executablePath: '/usr/bin/codex', prefixArgs: [], kind: 'native' }, { workspacePath: '/work', sandbox: 'workspace-write' }),
      prompt: 'count the files',
      sandbox: 'workspace-write',
      approvalPolicy: 'never'
    })
    const methods = Object.keys(SCHEMA_FOR)
    methods.forEach((method, n) => {
      emit(`${JSON.stringify({ jsonrpc: '2.0', id: 900 + n, method, params: { threadId: 't1', conversationId: 't1', itemId: `i${String(n)}`, callId: `c${String(n)}` } })}\n`)
    })
    await vi.waitFor(() => {
      expect(written.filter((m) => typeof m.id === 'number' && m.id >= 900)).toHaveLength(methods.length)
    })
    methods.forEach((method, n) => {
      const reply = written.find((m) => m.id === 900 + n)!
      expectCodexTakes(reply.result as JsonValue, SCHEMA_FOR[method]!)
    })
  })
})
