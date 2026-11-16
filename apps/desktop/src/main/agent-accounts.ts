import { randomUUID } from 'node:crypto'
import { lstat, mkdir, readFile, realpath, rename, rm, writeFile } from 'node:fs/promises'
import { join, resolve, dirname } from 'node:path'
import type { ExecutableLaunch } from '@teammate/runtime-adapters'
import { isAccountId, supportsAccounts, type AgentAccount, type AgentAccountsState, type AccountAgent } from '../shared/agent-accounts.js'

/** Only names and opaque ids live here. Credential files belong exclusively to the agent's CLI. */
export function createAgentAccounts(options: {
  readonly directory: string
  readonly login: (runtime: AccountAgent, home: string, env: Readonly<Record<string, string>>, done: (ok: boolean) => void) => Promise<() => void | Promise<void>>
  readonly refusal?: () => string | undefined
  readonly inUse?: (runtime: AccountAgent, account: string) => boolean | Promise<boolean>
  readonly changed?: () => void
}) {
  const root = resolve(options.directory, 'agent-accounts')
  const file = join(root, 'accounts.json')
  const pending = new Map<string, { cancel: () => void | Promise<void> }>()
  const removing = new Set<string>()
  let queue = Promise.resolve()
  const serial = <T>(work: () => Promise<T>): Promise<T> => {
    const next = queue.then(work)
    queue = next.then(() => undefined, () => undefined)
    return next
  }
  const load = async (): Promise<AgentAccount[]> => {
    try {
      const value: unknown = JSON.parse(await readFile(file, 'utf8'))
      if (!Array.isArray(value) || !value.every(a => a && isAccountId(a.id) && supportsAccounts(a.runtime) && typeof a.name === 'string' && ['new', 'waiting', 'ready', 'failed'].includes(a.state) && (a.forNew === undefined || a.forNew === true))) throw new Error('Invalid account list.')
      return value.map(a => a.state === 'waiting' && !pending.has(a.id) ? { ...a, state: 'failed' as const } : a)
    } catch (cause) { if ((cause as NodeJS.ErrnoException).code === 'ENOENT') return []; throw new Error('The saved account list could not be read. No account was changed.') }
  }
  const save = async (accounts: readonly AgentAccount[]) => {
    await mkdir(root, { recursive: true })
    await writeFile(`${file}.tmp`, JSON.stringify(accounts), { encoding: 'utf8', mode: 0o600 })
    await rename(`${file}.tmp`, file)
    options.changed?.()
  }
  const homeFor = (a: AgentAccount) => join(root, a.id)
  /** The list with new conversations of `runtime` on `id` alone, or on none (This Mac's sign-in). */
  const withForNew = (accounts: readonly AgentAccount[], runtime: AccountAgent, id: string | undefined): AgentAccount[] =>
    accounts.map(({ forNew: _was, ...x }) => (x.runtime === runtime && x.id === id ? { ...x, forNew: true as const } : x.runtime === runtime ? x : _was === true ? { ...x, forNew: true as const } : x))
  // Fail closed on junctions and symlinks, including the containing directory.
  const ownedHome = async (a: AgentAccount) => {
    const home = homeFor(a)
    if (dirname(home) !== root || (await lstat(root)).isSymbolicLink() || (await lstat(home)).isSymbolicLink()
      || resolve(await realpath(root)) !== root || resolve(await realpath(home)) !== home) throw new Error('The account folder has moved. Nothing was removed or started.')
    return home
  }
  const allowed = () => { const why = options.refusal?.(); if (why !== undefined) throw new Error(why) }
  const nameOf = (value: unknown) => {
    if (typeof value !== 'string' || !value.trim() || value.trim().length > 80 || /[\x00-\x1f\x7f]/.test(value)) throw new Error('Give the account a name of up to 80 characters.')
    return value.trim()
  }
  const envFor = (runtime: AccountAgent, home: string): Readonly<Record<string, string>> => runtime === 'claude'
    ? { CLAUDE_CONFIG_DIR: home, ANTHROPIC_API_KEY: '', ANTHROPIC_AUTH_TOKEN: '', CLAUDE_CODE_OAUTH_TOKEN: '' }
    : { CODEX_HOME: home, CODEX_SQLITE_HOME: home, OPENAI_API_KEY: '', CODEX_API_KEY: '', OPENAI_ACCESS_TOKEN: '', CODEX_ACCESS_TOKEN: '' }
  const get = async (id: unknown) => {
    if (!isAccountId(id)) throw new Error('Choose an account from Settings. Nothing was started.')
    const a = (await load()).find(a => a.id === id)
    if (!a) throw new Error('That account was removed. Choose another account in Settings. Nothing was started.')
    return a
  }
  return {
    list: () => serial(load),
    add: (runtime: unknown, name: unknown) => serial(async () => {
      allowed()
      if (typeof runtime !== 'string' || !supportsAccounts(runtime)) throw new Error('This agent does not support another account.')
      const accounts = await load()
      const a: AgentAccount = { id: `account_${randomUUID().replaceAll('-', '')}`, runtime, name: nameOf(name), state: 'new' }
      await mkdir(root, { recursive: true })
      if ((await lstat(root)).isSymbolicLink() || resolve(await realpath(root)) !== root) throw new Error('The account folder has moved. Nothing was added.')
      await mkdir(homeFor(a), { mode: 0o700 })
      await save([...accounts, a])
      return a
    }),
    rename: (id: unknown, name: unknown) => serial(async () => {
      allowed(); const a = await get(id); const next = nameOf(name)
      await save((await load()).map(x => x.id === a.id ? { ...x, name: next } : x))
    }),
    remove: (id: unknown) => serial(async () => {
      const a = await get(id)
      removing.add(a.id)
      try {
        if (pending.has(a.id) || (await options.inUse?.(a.runtime, a.id))) throw new Error('This agent is busy. Stop its runs and sign-in before removing the account.')
        const home = await ownedHome(a)
        // Persist removal before deleting. A failed delete leaves an orphan, never a usable dangling account.
        await save((await load()).filter(x => x.id !== a.id))
        await rm(home, { recursive: true, force: true })
      } finally { removing.delete(a.id) }
    }),
    login: (id: unknown) => serial(async () => {
      allowed(); const a = await get(id)
      if (pending.has(a.id)) return
      if ((await options.inUse?.(a.runtime, a.id))) throw new Error('This agent is busy. Finish its runs before signing in again.')
      const home = await ownedHome(a)
      const job: { cancel: () => void | Promise<void> } = { cancel: () => undefined }
      pending.set(a.id, job)
      await save((await load()).map(x => x.id === a.id ? { ...x, state: 'waiting' } : x))
      try {
        const cancel = await options.login(a.runtime, home, envFor(a.runtime, home), ok => {
          void serial(async () => {
            if (pending.get(a.id) !== job) return
            pending.delete(a.id)
            const signed = (await load()).map(x => x.id === a.id ? { ...x, state: ok ? 'ready' as const : 'failed' as const } : x)
            // Just signed in, and no other account has new conversations: they move to it (0.876).
            const taken = signed.some(x => x.runtime === a.runtime && x.forNew === true)
            await save(ok && !taken ? withForNew(signed, a.runtime, a.id) : signed)
          }).catch(() => undefined)
        })
        job.cancel = cancel
      } catch {
        pending.delete(a.id)
        await save((await load()).map(x => x.id === a.id ? { ...x, state: 'failed' } : x))
        throw new Error('The agent could not open its sign-in. Your other accounts are unchanged.')
      }
    }),
    cancel: (id: unknown) => serial(async () => {
      const a = await get(id); await pending.get(a.id)?.cancel(); pending.delete(a.id)
      await save((await load()).map(x => x.id === a.id && x.state === 'waiting' ? { ...x, state: 'failed' } : x))
    }),
    /** Puts new conversations of `runtime` on an added, signed-in account, or back on This Mac's sign-in. */
    useForNew: (runtime: unknown, id: unknown) => serial(async () => {
      allowed()
      if (typeof runtime !== 'string' || !supportsAccounts(runtime)) throw new Error('This agent does not support another account.')
      if (id === undefined) { await save(withForNew(await load(), runtime, undefined)); return }
      const a = await get(id)
      if (a.runtime !== runtime) throw new Error('That account belongs to another agent. New conversations are unchanged.')
      if (a.state !== 'ready') throw new Error(`Sign in to ${a.name} first. New conversations are unchanged.`)
      await save(withForNew(await load(), runtime, a.id))
    }),
    /** The account new conversations of `runtime` run on: an added one that is signed in, else none (This Mac's sign-in). */
    forNew: (runtime: string): Promise<string | undefined> => serial(async () => (await load()).find(a => a.runtime === runtime && a.forNew === true && a.state === 'ready')?.id),
    async launch(runtime: string, account: string | undefined, launch: ExecutableLaunch): Promise<ExecutableLaunch> {
      if (account === undefined) return launch
      allowed(); const a = await get(account)
      if (a.runtime !== runtime) throw new Error('That account belongs to another agent. Nothing was started.')
      const home = await ownedHome(a)
      if (removing.has(a.id) || pending.has(a.id)) throw new Error('This account is being removed or signed in. Finish that first. Nothing was started.')
      return { ...launch, env: { ...launch.env, ...envFor(a.runtime, home) } }
    },
    async state(): Promise<AgentAccountsState> { return { ok: true, accounts: (await serial(load)).map(a => ({ ...a, home: homeFor(a) })) } },
    dispose() { for (const job of pending.values()) void job.cancel(); pending.clear() }
  }
}
