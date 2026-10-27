import { randomUUID } from 'node:crypto'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { createServer } from 'node:http'
import type { Server } from 'node:http'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import type { MissionApprovalAnswer, MissionApprovalRequest } from '../shared/ipc.js'
import { relativeToFolder } from '../shared/approval-patch.js'
import { OUTSIDE_NOT_UNDOABLE, outsideFolder } from '../shared/approval-data.js'
import { wholeDetail, wholeInput } from '../shared/approval-detail.js'
import { deniedSaying } from './approval-channel.js'

/**
 * Locust as Claude Code's permission host.
 *
 * Claude Code asks before using a connector tool, and a printed run has
 * nowhere to put that question -- so every call was refused, and 0.61.0
 * papered over it with a blanket allow rule per server. That works and it
 * is what Colin asked for, but it is not how any other client behaves:
 * everywhere else the model asks, the person answers, and "always" is
 * remembered.
 *
 * `--permission-prompt-tool` is the way to give a printed run somewhere to
 * ask. MEASURED 2026-09-10 with a throwaway server: the CLI calls the named
 * MCP tool with `{tool_name, input, tool_use_id}` and waits;
 * `{"behavior":"allow"}` lets the call through, `{"behavior":"deny",
 * "message"}` reaches the model verbatim. It works in `--print` under
 * `--restricted` in the strictest permission mode. It is connectors only --
 * Bash and Edit are granted by `--tools` and never route through it.
 *
 * The shape: a tiny stdio MCP server ships beside the app
 * (resources/locust-permission-bridge.mjs). Each run gets an `mcp.json`
 * naming it, with a loopback URL and a token minted for that run in its
 * environment. The bridge forwards each question here over HTTP; this file
 * turns it into the approval card the app already has, waits for the
 * person, and answers. A token that is not one this process minted is
 * refused before anything else in the request is read.
 */

export const PERMISSION_TOOL_NAME = 'mcp__locust__approve'

export interface PermissionHost {
  /** Bring the listener up. Idempotent. */
  start(): Promise<void>
  /** The port the listener took. 0 until started. */
  readonly port: number
  /**
   * Prepare one run: mint its token, write its mcp.json, remember it. The
   * config path goes on the command line; nothing else leaves this process.
   */
  register(run: { readonly runId: string; readonly missionId: string; readonly cwd: string | null; readonly beforeApproval?: () => Promise<void> }): Promise<{
    readonly configPath: string
    readonly toolName: string
  }>
  /** A person's answer to a card. False when the id names nothing pending. */
  decide(answer: MissionApprovalAnswer): boolean
  /** The run is over: refuse anything still waiting, forget its token, remove its config. */
  release(runId: string): Promise<void>
  dispose(): Promise<void>
}

export type BridgeAnswer = { readonly behavior: 'allow'; readonly updatedInput: unknown } | { readonly behavior: 'deny'; readonly message: string }

interface Registered {
  readonly runId: string
  readonly missionId: string
  readonly cwd: string | null
  readonly configDir: string
  readonly beforeApproval?: () => Promise<void>
}

interface Pending {
  readonly runId: string
  readonly missionId: string
  readonly input: unknown
  readonly resolve: (answer: BridgeAnswer) => void
}

/** `mcp__claude_ai_Robinhood__get_accounts` -> `mcp__claude_ai_Robinhood__` */
export function serverPrefixOf(toolName: string): string {
  const match = /^(mcp__[A-Za-z0-9_]+?__)/.exec(toolName)
  return match?.[1] ?? toolName
}

/** The sentence on the card, from the tool name the way the activity rows say it. */
export function connectorSummary(toolName: string): string {
  const match = /^mcp__([A-Za-z0-9_]+?)__(.+)$/.exec(toolName)
  if (match === null) return `Use ${toolName}`
  const server = match[1]!.replace(/^claude_ai_/, '').replace(/_/g, ' ')
  return `Use ${match[2]!} on ${server}`
}

/**
 * The mcp.json one run is launched with.
 *
 * Exported so the shape is testable without a listener: the CLI reads this
 * file, and a field misnamed here is a bridge that never starts and a run
 * whose every connector call is silently refused.
 */
export function bridgeConfig(options: {
  readonly bridgePath: string
  readonly url: string
  readonly token: string
  readonly node: string
}): string {
  return JSON.stringify(
    {
      mcpServers: {
        locust: {
          command: options.node,
          args: [options.bridgePath],
          // `ELECTRON_RUN_AS_NODE` makes Electron's own binary behave as plain
          // node, so the bridge runs on a runtime that is GUARANTEED to be on
          // this machine -- Claude Code no longer needs node on PATH and this
          // must not depend on it either.
          env: { ELECTRON_RUN_AS_NODE: '1', LOCUST_PERMISSION_URL: options.url, LOCUST_PERMISSION_TOKEN: options.token }
        }
      }
    },
    null,
    2
  )
}

const deny = (message: string): BridgeAnswer => ({ behavior: 'deny', message })

/**
 * What a bridged request is, in the card's words.
 *
 * The bridge was built for connectors, and every request through it was
 * drawn as one. But in Edit mode Claude Code routes its OWN tools here too:
 * a drive of the check-after-edits feature (2026-09-25) had Bash asking to
 * run `node check.js`, and the card said the input went "to the service the
 * connector reaches" and that Locust "cannot undo what happens there" -- about
 * a command on this machine. A built-in tool is now the card it is.
 */
export function builtInOrConnector(
  toolName: string,
  input: unknown,
  cwd: string
): {
  readonly kind: 'command' | 'file-change' | 'connector'
  readonly summary: string
  readonly detail: string
  readonly reversibleSays?: string
  readonly dataSentSays?: string
} {
  const fields = typeof input === 'object' && input !== null ? (input as Record<string, unknown>) : {}
  const text = (value: unknown): string => (typeof value === 'string' ? value.replace(/\s+/g, ' ').trim().slice(0, 600) : '')
  /*
   * THE WEB (0.711). Claude Code's own search and fetch, asked about outside
   * Auto. Drawn as what leaves this machine -- a connector's kind -- in words
   * of their own: the search's words go to the search, and a page's whole
   * address goes to that page's server, which is where a folder's hidden
   * instruction would put what it wanted out.
   */
  if (toolName === 'WebSearch') {
    return {
      kind: 'connector',
      summary: 'Search the web',
      detail: text(fields.query),
      dataSentSays: 'The search words above, to Claude Code\'s web search.',
      reversibleSays: 'Nothing is changed by a search.'
    }
  }
  if (toolName === 'WebFetch') {
    const url = text(fields.url)
    let host = ''
    try {
      host = new URL(url).host
    } catch {
      host = ''
    }
    return {
      kind: 'connector',
      summary: host.length > 0 ? `Open a page on ${host}` : 'Open a web page',
      detail: [url, text(fields.prompt)].filter((line) => line.length > 0).join('\n'),
      dataSentSays: 'The whole address above, to that site. Read it: anything written into an address is sent with it.',
      reversibleSays: 'Opening a page changes nothing here. A site can act on an address it is sent.'
    }
  }
  if (toolName === 'Bash') {
    // Whole, line breaks kept: a heredoc or a long command is read as written (R14).
    const command = typeof fields.command === 'string' ? wholeDetail(fields.command) : ''
    return { kind: 'command', summary: command.length > 0 ? 'Run a command' : 'Run a command it did not describe', detail: command }
  }
  if (toolName === 'Edit' || toolName === 'Write' || toolName === 'MultiEdit' || toolName === 'NotebookEdit') {
    const file = text(fields.file_path) || text(fields.notebook_path)
    // Outside the folder, said as such (QA-2026-09-29 round 2, R15): a write
    // to /etc/cron.d read "Change 1 file ... Yes for tracked files", in the
    // words of one inside it, when no version control of the project holds it.
    if (file.length > 0 && outsideFolder(file, cwd)) {
      return { kind: 'file-change', summary: 'Change 1 file outside your project folder', detail: file, reversibleSays: OUTSIDE_NOT_UNDOABLE }
    }
    return { kind: 'file-change', summary: 'Change 1 file', detail: file.length > 0 ? relativeToFolder(file, cwd) : '' }
  }
  if (!toolName.startsWith('mcp__')) {
    return { kind: 'command', summary: `Use ${toolName}`, detail: wholeInput(input) }
  }
  // The input the connector would be called with, WHOLE and one field to a
  // line (R14): it is the one thing a person can judge a connector call by,
  // and a recipient written last was past the old 600-character cut.
  return { kind: 'connector', summary: connectorSummary(toolName), detail: wholeInput(input) }
}

/**
 * WHAT "ALWAYS" LETS THROUGH, SAID (QA-2026-09-29 round 2, R35).
 *
 * Always was remembered per CONNECTOR: pressed on "Use list_issues on
 * github", it let the next call of the reply -- delete_file on the same
 * connector -- through with no card. It is remembered per TOOL now, as
 * Claude Code's own "don't ask again" is, and the card says what it covers.
 */
export function alwaysKeyOf(toolName: string): string {
  return toolName
}

export function alwaysCoversFor(toolName: string): string {
  if (toolName === 'Bash') return 'every command it runs'
  const match = /^mcp__([A-Za-z0-9_]+?)__(.+)$/.exec(toolName)
  if (match === null) return `every use of ${toolName}`
  const server = match[1]!.replace(/^claude_ai_/, '').replace(/_/g, ' ')
  return `${match[2]!} on ${server} again, and no other tool`
}

export function createPermissionHost(options: {
  /** Absolute path of the bridge script the CLI will spawn. */
  readonly bridgePath: string
  /** The node the bridge runs on. Electron's own binary can stand in when `ELECTRON_RUN_AS_NODE` is set for it. */
  readonly node: string
  /** Raise a card. The host answers when `decide` is called with its id. */
  readonly emitApproval: (request: MissionApprovalRequest) => void
  /**
   * Take a card down: the bridge stopped waiting for it (0.837). After ten minutes the bridge tells Claude Code "nobody
   * answered in time", and Claude Code asks again, so the old card waited on nothing: two of one card on screen (Colin's
   * frame, 2026-10-26), and pressing the old one did nothing.
   */
  readonly withdrawApproval?: (approvalId: string) => void
  readonly createId?: () => string
  readonly now?: () => Date
  /** A fixed port for tests; 0 lets the OS choose. */
  readonly port?: number
}): PermissionHost {
  const createId = options.createId ?? randomUUID
  const now = options.now ?? (() => new Date())
  const tokens = new Map<string, Registered>()
  const byRun = new Map<string, string>()
  const pending = new Map<string, Pending>()
  let server: Server | undefined
  let starting: Promise<void> | undefined
  let port = 0

  const answerRequest = async (body: unknown, raised: (approvalId: string) => void = () => undefined): Promise<BridgeAnswer> => {
    const asked = typeof body === 'object' && body !== null ? (body as Record<string, unknown>) : {}
    // The token FIRST. Anything on this machine can reach a loopback port;
    // only a process handed a token by this one is a run of ours.
    const registered = typeof asked.token === 'string' ? tokens.get(asked.token) : undefined
    if (registered === undefined) return deny('this request did not come from a Locust run.')
    try {
      await registered.beforeApproval?.()
    } catch {
      return deny('The run could not write its pending text, so this approval was refused.')
    }
    // A run may end while its last text is being fsynced. It no longer asks.
    if (tokens.get(asked.token as string) !== registered) return deny('The run ended before the approval could be shown.')
    const toolName = typeof asked.toolName === 'string' && asked.toolName.length > 0 ? asked.toolName : 'a tool'
    const input = asked.input ?? {}
    // Which call is asking, as Claude Code names it: the thread finds the
    // call's row by it, and a helper's call names its helper on the card.
    const toolUseId = typeof asked.toolUseId === 'string' && /^[A-Za-z0-9_-]{1,128}$/.test(asked.toolUseId) ? asked.toolUseId : undefined
    /*
     * EVERY REQUEST IS RAISED (0.616, shared/who-decides.ts). This host used
     * to answer a tool it had been told "always" about by itself, before the
     * saved rules were read: an Always on one Bash card let every later
     * command through, a rule saying no and a command that stops every
     * python.exe included. It remembers nothing now; the key below is what an
     * Always would cover, and the main process decides with it.
     */
    const approvalId = createId()
    const decided = new Promise<BridgeAnswer>((resolve) => {
      pending.set(approvalId, { runId: registered.runId, missionId: registered.missionId, input, resolve })
    })
    raised(approvalId)
    options.emitApproval({
      approvalId,
      runId: registered.runId,
      missionId: registered.missionId,
      ...builtInOrConnector(toolName, input, registered.cwd ?? ''),
      alwaysCovers: alwaysCoversFor(toolName),
      alwaysKey: `claude:${alwaysKeyOf(toolName)}`,
      runtime: 'claude',
      cwd: registered.cwd,
      requestedAt: now().toISOString(),
      ...(toolUseId === undefined ? {} : { toolUseId })
    })
    return decided
  }

  const host: PermissionHost = {
    get port() {
      return port
    },

    async start() {
      if (server !== undefined) return
      // One start, however many ask: two runs registering at once each
      // started a listener, and the first was never closed (a B4 lead).
      if (starting !== undefined) return starting
      starting = (async (): Promise<void> => {
      const listener = createServer((req, res) => {
        if (req.method !== 'POST' || req.url !== '/approve') {
          res.writeHead(404).end()
          return
        }
        const chunks: Buffer[] = []
        let size = 0
        req.on('data', (chunk: Buffer) => {
          size += chunk.length
          // A permission question is a tool name and an input. Anything
          // larger than this is not one.
          if (size <= 256 * 1024) chunks.push(chunk)
        })
        req.on('end', () => {
          let body: unknown
          try {
            body = JSON.parse(Buffer.concat(chunks).toString('utf8'))
          } catch {
            body = {}
          }
          let card: string | undefined
          void answerRequest(body, (approvalId) => { card = approvalId }).then((answer) => {
            if (!res.destroyed) res.writeHead(200, { 'content-type': 'application/json' }).end(JSON.stringify(answer))
          })
          // The bridge let go before anyone answered (its ten minutes ran out, or the run's process ended): the
          // card waits on nothing now, so it comes down, and the question it asks again is the one card.
          res.on('close', () => {
            if (res.writableEnded || card === undefined) return
            const waiting = pending.get(card)
            if (waiting === undefined) return
            pending.delete(card)
            waiting.resolve(deny('Nobody answered in time.'))
            options.withdrawApproval?.(card)
          })
        })
      })
      await new Promise<void>((resolve, reject) => {
        listener.once('error', reject)
        // Loopback only. Never 0.0.0.0: this is a door into approving things.
        listener.listen(options.port ?? 0, '127.0.0.1', () => resolve())
      })
      const address = listener.address()
      port = typeof address === 'object' && address !== null ? address.port : 0
      server = listener
      })().finally(() => {
        starting = undefined
      })
      return starting
    },

    async register(run) {
      if (server === undefined) await host.start()
      const token = createId()
      const configDir = await mkdtemp(join(tmpdir(), 'locust-permission-'))
      const configPath = join(configDir, 'mcp.json')
      await writeFile(
        configPath,
        bridgeConfig({
          bridgePath: options.bridgePath,
          url: `http://127.0.0.1:${String(port)}/approve`,
          token,
          node: options.node
        }),
        'utf8'
      )
      tokens.set(token, { runId: run.runId, missionId: run.missionId, cwd: run.cwd, configDir, ...(run.beforeApproval === undefined ? {} : { beforeApproval: run.beforeApproval }) })
      byRun.set(run.runId, token)
      return { configPath, toolName: PERMISSION_TOOL_NAME }
    },

    decide(answer) {
      const waiting = pending.get(answer.approvalId)
      if (waiting === undefined) return false
      pending.delete(answer.approvalId)
      if (!('decision' in answer)) {
        // A question's answer reaching a permission: refused, and said.
        waiting.resolve(deny('Locust did not send that answer: it belongs to a question, and the runtime was waiting on a permission.'))
        return true
      }
      if (answer.decision === 'deny') {
        // Claude reads the denial's message as the tool's result, so a reason
        // the person gave rides it (0.374).
        waiting.resolve(deny(answer.reason === undefined ? 'Denied in Locust.' : `Denied in Locust. ${deniedSaying(answer.reason)}`))
        return true
      }
      // An Always is remembered by the main process, under this request's
      // `alwaysKey` (0.616); to Claude Code it is this call allowed.
      waiting.resolve({ behavior: 'allow', updatedInput: waiting.input })
      return true
    },

    async release(runId) {
      const token = byRun.get(runId)
      byRun.delete(runId)
      for (const [id, waiting] of pending) {
        if (waiting.runId === runId) {
          pending.delete(id)
          waiting.resolve(deny('The run ended before anyone answered.'))
        }
      }
      if (token === undefined) return
      const registered = tokens.get(token)
      tokens.delete(token)
      if (registered !== undefined) await rm(registered.configDir, { recursive: true, force: true }).catch(() => undefined)
    },

    async dispose() {
      for (const runId of [...byRun.keys()]) await host.release(runId)
      const listener = server
      server = undefined
      if (listener !== undefined) await new Promise<void>((resolve) => listener.close(() => resolve()))
    }
  }
  return host
}
