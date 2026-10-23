import { createHash } from 'node:crypto'
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises'
import { dirname } from 'node:path'

import type { Letter } from '../../../relay/src/protocol.js'
import { MAX_TOLD, newAwayKey, openAwayLetter, sealAwayLetter, type AwayLetter } from '../shared/away-letter.js'
import type { FolderPullRequest } from '../shared/pull-request.js'
import type { ScoutAwayStatus } from '../shared/scout-away.js'
import { randomText, sha256 } from './relay-crypto.js'
import { replaceFile } from './replace-file.js'
import { AWAY_WORKFLOW_NAME, type AwayConfig } from './scout-away-look.js'

/**
 * SCOUT WHILE YOUR COMPUTER IS OFF (0.829, "Away from your desk", step 2). Turned on in Settings, Locust adds Scout's
 * away look to the repository of the window's folder by a pull request the person merges: a scheduled workflow and
 * the one file it runs (scout-away-look.ts). The workflow reads with its own read-only token, so no token of the
 * person's leaves this PC; what it needs -- the relay's room, a token the relay lets in, the key letters are sealed
 * with, the person's login, time zone and Scout's hours -- goes in the repository's LOCUST_SCOUT secret.
 *
 * While the PC is on, Scout looks from here and leaves a letter saying what it told, so the away look that follows
 * does not tell it again; the workflow sees the PC on and does nothing more. When the PC is back, the away looks'
 * letters are read: what they found goes on the Board, told already, and into What Scout saw. A paired phone is
 * given the key over its sealed line, to show the letters itself while the PC is off.
 *
 * It needs the phone's way from anywhere on: the letters wait in that room on the relay.
 */
export const SCOUT_AWAY_FILE = 'scout-away.json'
/** The most Scout's first look after a launch waits for the line to the relay, to read the letters first. */
const CONNECT_WAIT_MS = 10_000
export const AWAY_SECRET = 'LOCUST_SCOUT'
export const AWAY_BRANCH = 'locust/scout-away'
export const AWAY_WORKFLOW_PATH = '.github/workflows/locust-scout.yml'
export const AWAY_SCRIPT_PATH = '.github/locust/scout-away.mjs'

/** The workflow, every hour on a public repository (free) and every two on a private one (about 360 minutes a month). */
export function awayWorkflow(every: 1 | 2): string {
  return `# Scout's away look, from Locust (https://github.com/StopBanning/locust).
# While your computer is off, Scout looks at this repository's pull requests, reviews, checks and
# issues for you, and leaves what it found at Locust's relay, sealed with a key only your devices
# have, so your phone can show it. With Locust open on your computer, it does nothing more.
# It reads with this workflow's own read-only token. To stop it, turn it off in Locust, or delete
# this file, .github/locust/scout-away.mjs and the LOCUST_SCOUT secret.
name: ${AWAY_WORKFLOW_NAME}
on:
  schedule:
    - cron: '23 ${every === 1 ? '*' : '*/2'} * * *'
  workflow_dispatch:
permissions:
  contents: read
  pull-requests: read
  issues: read
  checks: read
  statuses: read
  actions: read
concurrency:
  group: locust-scout
  cancel-in-progress: true
jobs:
  look:
    runs-on: ubuntu-latest
    timeout-minutes: 5
    steps:
      - name: Look for you
        env:
          LOCUST_SCOUT: \${{ secrets.LOCUST_SCOUT }}
          GH_TOKEN: \${{ github.token }}
          GH_REPO: \${{ github.repository }}
        run: |
          gh api "repos/$GH_REPO/contents/${AWAY_SCRIPT_PATH}?ref=$GITHUB_SHA" -H 'Accept: application/vnd.github.raw' > "$RUNNER_TEMP/scout-away.mjs"
          node "$RUNNER_TEMP/scout-away.mjs"
`
}

interface Saved {
  readonly v: 1
  readonly repo: string
  readonly folder: string
  readonly login: string
  readonly key: string
  readonly token: string
  /** The relay room it was opened in: phones turned off and on again make a new room, and this one is gone. */
  readonly room: string
  readonly every: 1 | 2
  readonly private: boolean
  readonly branch: string
  readonly pr?: { readonly url: string; readonly number: number }
  readonly merged: boolean
  /** The newest letter read here. */
  readonly cursor?: string
  readonly lastLook?: string
  /** What the last letter from here said, by its hash: one is left only when it changes. */
  readonly told?: string
  /** The away look's file as it went in, by its hash. */
  readonly script: string
}

export interface ScoutAwayRelay {
  /** Settled once the phones' settings are read at launch: before then no room says nothing. */
  ready(): Promise<void>
  room(): { readonly relay: string; readonly room: string } | undefined
  connected(): boolean
  awayOpen(hash: string): Promise<boolean>
  awayClose(): Promise<boolean>
  keepAwayAlerts(): Promise<boolean>
  post(box: { readonly n: string; readonly c: string }): Promise<boolean>
  letters(after?: string): Promise<{ readonly items: readonly Letter[]; readonly more: boolean } | undefined>
}

export interface ScoutAwayDeps {
  readonly file: string
  /** The window's folder, when one is chosen. */
  readonly folder: () => string | undefined
  readonly gh: (args: readonly string[], cwd: string) => Promise<string>
  /** The same, with `input` on its stdin: a secret is never put on a command line. */
  readonly ghInput: (args: readonly string[], cwd: string, input: string) => Promise<string>
  readonly relay: () => ScoutAwayRelay | undefined
  readonly scout: {
    /** What Scout has told, as the next away look must know it. */
    told(): Promise<{ readonly prs: readonly FolderPullRequest[]; readonly told: readonly string[]; readonly briefDay?: string }>
    /** The away looks' letters, oldest first. */
    absorb(letters: readonly AwayLetter[]): Promise<void>
    hours(): Promise<{ readonly quiet?: { readonly from: number; readonly to: number }; readonly briefAt?: number }>
  }
  /** The away look's one file (resources/scout-away.mjs). */
  readonly script: () => Promise<string>
  readonly tz?: () => string
  readonly now?: () => Date
  readonly changed?: (status: ScoutAwayStatus) => void
}

const hashOf = (text: string): string => createHash('sha256').update(text, 'utf8').digest('base64url')
const b64 = (text: string): string => Buffer.from(text, 'utf8').toString('base64')
/** gh's own words on a failure, cut to the line that says what went wrong. */
const ghSaid = (error: unknown): string => {
  const text = error instanceof Error ? String((error as Error & { stderr?: unknown }).stderr ?? error.message) : String(error)
  return text.split(/\r?\n/).map((line) => line.trim()).find((line) => line !== '' && !/^Command failed/.test(line))?.slice(0, 200) ?? 'GitHub did not answer.'
}

export function createScoutAway(deps: ScoutAwayDeps) {
  const now = deps.now ?? (() => new Date())
  const tz = deps.tz ?? (() => Intl.DateTimeFormat().resolvedOptions().timeZone || 'UTC')
  let saved: Saved | undefined
  let loaded = false
  let busy: string | undefined
  let message: string | undefined
  let queue: Promise<unknown> = Promise.resolve()
  let catching: Promise<void> | undefined

  const serial = <T>(work: () => Promise<T>): Promise<T> => {
    const task = queue.then(work)
    queue = task.catch(() => undefined)
    return task
  }

  const load = async (): Promise<Saved | undefined> => {
    if (loaded) return saved
    loaded = true
    try {
      const held = JSON.parse(await readFile(deps.file, 'utf8')) as Saved
      if (held.v === 1 && typeof held.repo === 'string' && typeof held.key === 'string' && typeof held.token === 'string' && typeof held.room === 'string') saved = held
    } catch { /* Missing is off. */ }
    return saved
  }
  const persist = async (next: Saved | undefined): Promise<void> => {
    saved = next
    if (next === undefined) { await rm(deps.file, { force: true, maxRetries: 20, retryDelay: 100 }); return }
    await mkdir(dirname(deps.file), { recursive: true })
    const temp = `${deps.file}.tmp`
    await writeFile(temp, JSON.stringify(next), { mode: 0o600 })
    await replaceFile(temp, deps.file)
  }

  const blocked = (): string | undefined => {
    const relay = deps.relay()
    if (relay?.room() === undefined) return 'Turn on “Your phone, from anywhere” first: Scout leaves what it finds there.'
    if (deps.folder() === undefined) return 'Open a project folder that is on GitHub first.'
    return undefined
  }
  const status = (): ScoutAwayStatus => {
    if (busy !== undefined) return { state: 'busy', doing: busy }
    const held = saved
    const said = message === undefined ? {} : { message }
    if (held === undefined) {
      const why = blocked()
      return { state: 'off', ...(why === undefined ? {} : { blocked: why }), ...said }
    }
    if (!held.merged) return { state: 'waiting', repo: held.repo, pr: held.pr?.url ?? `https://github.com/${held.repo}/pulls`, ...said }
    return { state: 'on', repo: held.repo, every: held.every, private: held.private, ...(held.lastLook === undefined ? {} : { lastLook: held.lastLook }), ...said }
  }
  const tell = (): ScoutAwayStatus => { const now = status(); deps.changed?.(now); return now }

  /** The away look's settings, as the repository's secret holds them. */
  const configOf = async (held: Pick<Saved, 'token' | 'key' | 'login' | 'room'>, relay: string): Promise<AwayConfig> => {
    const hours = await deps.scout.hours().catch(() => ({}) as Awaited<ReturnType<ScoutAwayDeps['scout']['hours']>>)
    return {
      v: 1, relay, room: held.room, token: held.token, key: held.key, login: held.login, tz: tz(),
      ...(hours.quiet === undefined ? {} : { quiet: hours.quiet }),
      ...(hours.briefAt === undefined ? {} : { briefAt: hours.briefAt })
    }
  }

  /** What Scout told, left at the relay for the next away look; only when it changed since the last. */
  const leaveTold = async (force = false): Promise<void> => {
    const held = saved
    const relay = deps.relay()
    if (held === undefined || relay === undefined || !relay.connected()) return
    const told = await deps.scout.told().catch(() => undefined)
    if (told === undefined) return
    const letter: AwayLetter = { v: 1, kind: 'told', at: now().toISOString(), repo: held.repo, read: true, found: [], prs: told.prs, told: told.told.slice(-MAX_TOLD), ...(told.briefDay === undefined ? {} : { briefDay: told.briefDay }) }
    const mark = hashOf(JSON.stringify({ ...letter, at: '' }))
    if (!force && mark === held.told) return
    if (await relay.post(await sealAwayLetter(held.key, letter, held.room, 'pc'))) await persist({ ...held, told: mark }).catch(() => undefined)
  }

  /** The letters left since the last one read here: the away looks' go to Scout, oldest first. */
  const catchUp = async (): Promise<void> => {
    const held = saved
    const relay = deps.relay()
    if (held === undefined || relay === undefined || !relay.connected() || relay.room()?.room !== held.room) return
    let cursor = held.cursor
    const looks: AwayLetter[] = []
    for (let page = 0; page < 10; page += 1) {
      const got = await relay.letters(cursor)
      if (got === undefined) break
      for (const item of got.items) {
        cursor = item.id
        if (item.from !== 'away') continue
        const letter = await openAwayLetter(held.key, item, held.room, 'away')
        if (letter !== undefined && letter.kind === 'look' && letter.repo.toLowerCase() === held.repo.toLowerCase()) looks.push(letter)
      }
      if (!got.more) break
    }
    if (looks.length > 0) await deps.scout.absorb(looks)
    const lastLook = looks.at(-1)?.at ?? held.lastLook
    if (cursor !== held.cursor || lastLook !== held.lastLook) {
      await persist({ ...(saved ?? held), ...(cursor === undefined ? {} : { cursor }), ...(lastLook === undefined ? {} : { lastLook }) }).catch(() => undefined)
      tell()
    }
  }

  /** Whether the look is on the repository's default branch now: the pull request was merged. */
  const mergedYet = async (held: Saved): Promise<boolean | 'closed'> => {
    const folder = deps.folder() ?? held.folder
    try {
      await deps.gh(['api', `repos/${held.repo}/contents/${AWAY_WORKFLOW_PATH}?ref=${encodeURIComponent(held.branch)}`, '--jq', '.sha'], folder)
      return true
    } catch { /* Not there yet. */ }
    if (held.pr === undefined) return false
    try {
      const state = (await deps.gh(['api', `repos/${held.repo}/pulls/${String(held.pr.number)}`, '--jq', '.state'], folder)).trim()
      return state === 'closed' ? 'closed' : false
    } catch {
      return false
    }
  }

  /** Lets the away look go: the relay forgets it, its secret goes, and its workflow is turned off or its pull request closed. */
  const letGo = async (held: Saved, why?: string): Promise<void> => {
    const relay = deps.relay()
    if (relay?.room()?.room === held.room) await relay.awayClose().catch(() => false)
    const folder = deps.folder() ?? held.folder
    await deps.gh(['secret', 'delete', AWAY_SECRET, '--repo', held.repo], folder).catch(() => undefined)
    if (held.merged) await deps.gh(['workflow', 'disable', AWAY_WORKFLOW_NAME, '--repo', held.repo], folder).catch(() => undefined)
    else if (held.pr !== undefined) await deps.gh(['pr', 'close', String(held.pr.number), '--repo', held.repo, '--delete-branch'], folder).catch(() => undefined)
    await persist(undefined)
    message = why
  }

  const turnOn = async (): Promise<void> => {
    const why = blocked()
    if (why !== undefined) { message = why; return }
    const relay = deps.relay()!
    const place = relay.room()!
    const folder = deps.folder()!
    if (!relay.connected()) { message = 'Locust’s relay cannot be reached right now. Try again in a minute.'; return }
    busy = 'Reading the repository…'
    tell()
    let repo: { nameWithOwner: string; visibility: string; defaultBranchRef: { name: string }; viewerPermission: string }
    try {
      repo = JSON.parse(await deps.gh(['repo', 'view', '--json', 'nameWithOwner,visibility,defaultBranchRef,viewerPermission'], folder)) as typeof repo
    } catch {
      message = 'This folder is not on GitHub, or GitHub’s CLI is not signed in here (Settings > Accounts).'
      return
    }
    if (repo.viewerPermission !== 'ADMIN') { message = `Only an owner or admin of ${repo.nameWithOwner} can add the secret the away look needs.`; return }
    const login = (await deps.gh(['api', 'user', '--jq', '.login'], folder).catch(() => '')).trim()
    if (!/^[A-Za-z0-9-]{1,39}$/.test(login)) { message = 'GitHub’s CLI did not say who you are. Sign in again in Settings > Accounts.'; return }
    const branch = repo.defaultBranchRef.name
    const isPrivate = repo.visibility !== 'PUBLIC'
    const every: 1 | 2 = isPrivate ? 2 : 1
    const token = randomText(32)
    const key = newAwayKey()
    const script = await deps.script()
    const workflow = awayWorkflow(every)

    busy = 'Letting the away look into Locust’s relay…'
    tell()
    if (!(await relay.awayOpen(await sha256(token)))) { message = 'Locust’s relay did not take the away look. Try again in a minute.'; return }
    const draft: Saved = { v: 1, repo: repo.nameWithOwner, folder, login, key, token, room: place.room, every, private: isPrivate, branch, merged: false, script: hashOf(script) }
    try {
      busy = 'Adding the LOCUST_SCOUT secret…'
      tell()
      await deps.ghInput(['secret', 'set', AWAY_SECRET, '--repo', repo.nameWithOwner], folder, JSON.stringify(await configOf(draft, place.relay)))

      // Already there, as Locust would put it: nothing to merge.
      const onBranch = async (path: string): Promise<string | undefined> => deps.gh(['api', `repos/${repo.nameWithOwner}/contents/${path}?ref=${encodeURIComponent(branch)}`, '-H', 'Accept: application/vnd.github.raw'], folder).catch(() => undefined)
      const [hadWorkflow, hadScript] = await Promise.all([onBranch(AWAY_WORKFLOW_PATH), onBranch(AWAY_SCRIPT_PATH)])
      if (hadWorkflow?.replace(/\r\n/g, '\n') === workflow && hadScript?.replace(/\r\n/g, '\n') === script) {
        await deps.gh(['workflow', 'enable', AWAY_WORKFLOW_NAME, '--repo', repo.nameWithOwner], folder).catch(() => undefined)
        await persist({ ...draft, merged: true })
      } else {
        busy = 'Opening the pull request…'
        tell()
        const base = JSON.parse(await deps.gh(['api', `repos/${repo.nameWithOwner}/git/commits/${(await deps.gh(['api', `repos/${repo.nameWithOwner}/git/ref/heads/${encodeURIComponent(branch)}`, '--jq', '.object.sha'], folder)).trim()}`], folder)) as { sha: string; tree: { sha: string } }
        const tree = JSON.parse(await deps.ghInput(['api', '-X', 'POST', `repos/${repo.nameWithOwner}/git/trees`, '--input', '-'], folder, JSON.stringify({
          base_tree: base.tree.sha,
          tree: [
            { path: AWAY_WORKFLOW_PATH, mode: '100644', type: 'blob', content: workflow },
            { path: AWAY_SCRIPT_PATH, mode: '100644', type: 'blob', content: script }
          ]
        }))) as { sha: string }
        const commit = JSON.parse(await deps.ghInput(['api', '-X', 'POST', `repos/${repo.nameWithOwner}/git/commits`, '--input', '-'], folder, JSON.stringify({
          message: 'Add Scout\'s away look from Locust\n\nWhile your computer is off, Scout looks at this repository on a schedule, with the workflow\'s own read-only token, and leaves what it found at Locust\'s relay, sealed for your devices.',
          tree: tree.sha,
          parents: [base.sha]
        }))) as { sha: string }
        const ref = `refs/heads/${AWAY_BRANCH}`
        await deps.ghInput(['api', '-X', 'POST', `repos/${repo.nameWithOwner}/git/refs`, '--input', '-'], folder, JSON.stringify({ ref, sha: commit.sha }))
          .catch(() => deps.ghInput(['api', '-X', 'PATCH', `repos/${repo.nameWithOwner}/git/${ref}`, '--input', '-'], folder, JSON.stringify({ sha: commit.sha, force: true })))
        const body = [
          'Locust’s Scout looks at this repository for you while your computer is off, and your phone shows what it found.',
          '',
          `- **What runs:** \`${AWAY_WORKFLOW_PATH}\`, ${every === 1 ? 'every hour' : 'every two hours'}, runs \`${AWAY_SCRIPT_PATH}\`. It reads your pull requests, reviews asked of you, the ${branch} branch’s checks and new issues, with the workflow’s own read-only token.`,
          '- **Where it goes:** to Locust’s relay, sealed with a key only your devices have. With Locust open on your computer, it does nothing more.',
          `- **What it costs:** ${isPrivate ? 'about 360 of the 2,000 free Actions minutes a private repository gets each month' : 'nothing: Actions are free on public repositories'}.`,
          '- **To stop it:** turn it off in Locust, or delete these two files and the `LOCUST_SCOUT` secret.'
        ].join('\n')
        let pr: { url: string; number: number }
        try {
          pr = JSON.parse(await deps.ghInput(['api', '-X', 'POST', `repos/${repo.nameWithOwner}/pulls`, '--input', '-', '--jq', '{url: .html_url, number: .number}'], folder, JSON.stringify({ title: 'Scout’s away look, from Locust', head: AWAY_BRANCH, base: branch, body }))) as typeof pr
        } catch {
          // One is open already, from an earlier try: it has the new commit now.
          pr = (JSON.parse(await deps.gh(['pr', 'list', '--repo', repo.nameWithOwner, '--head', AWAY_BRANCH, '--state', 'open', '--json', 'url,number'], folder)) as { url: string; number: number }[])[0]!
          if (pr === undefined) throw new Error('GitHub did not open the pull request.')
        }
        await persist({ ...draft, pr })
      }
    } catch (error) {
      await relay.awayClose().catch(() => false)
      await deps.gh(['secret', 'delete', AWAY_SECRET, '--repo', repo.nameWithOwner], folder).catch(() => undefined)
      await persist(undefined).catch(() => undefined)
      message = `It could not be set up: ${ghSaid(error)}`
      return
    }
    message = undefined
    await relay.keepAwayAlerts().catch(() => false)
    await leaveTold(true)
  }

  /** Off when the room it was opened in is gone: the phones were turned off and on, and the relay forgot it. */
  const roomGone = async (): Promise<boolean> => {
    const held = saved
    const relay = deps.relay()
    if (held === undefined || relay === undefined) return false
    await relay.ready().catch(() => undefined)
    if (relay.room()?.room === held.room) return false
    await letGo(held, 'Your phones were turned off, so Scout stopped looking while your computer is off. Turn it on again here.').catch(() => undefined)
    return true
  }

  return {
    status: (check = false): Promise<ScoutAwayStatus> => serial(async () => {
      await load()
      await deps.relay()?.ready().catch(() => undefined)
      if (await roomGone()) return tell()
      const held = saved
      if (check && held !== undefined && !held.merged) {
        const merged = await mergedYet(held)
        if (merged === 'closed') await letGo(held, 'The pull request was closed without merging, so Scout does not look while your computer is off.')
        else if (merged) { await persist({ ...held, merged: true }); message = undefined }
        return tell()
      }
      return status()
    }),

    set: (on: unknown): Promise<ScoutAwayStatus> => serial(async () => {
      await load()
      await deps.relay()?.ready().catch(() => undefined)
      if (typeof on !== 'boolean' || busy !== undefined) return status()
      if (on && saved === undefined) {
        try { await turnOn() } finally { busy = undefined }
      } else if (!on && saved !== undefined) {
        busy = 'Turning it off…'
        tell()
        try { await letGo(saved) } finally { busy = undefined }
      }
      return tell()
    }),

    /** The line to the relay is up, or a phone's alerts changed. */
    refresh: (): void => {
      void serial(async () => {
        await load()
        if (saved === undefined || await roomGone()) return
        await deps.relay()?.keepAwayAlerts().catch(() => false)
        await catchUp()
        await leaveTold()
      }).catch(() => undefined)
    },

    /** A letter came while connected: the away look's, read at once. */
    letter: (letter: Letter): void => {
      if (letter.from !== 'away') return
      void serial(catchUp).catch(() => undefined)
    },

    /** Before Scout looks: the letters left while the PC was off are read first, so nothing is told twice. */
    catchUp: (): Promise<void> => {
      catching ??= (async () => {
        if (await load() === undefined) return
        // Just launched, the line to the relay takes a moment: the letters are worth that wait.
        const relay = deps.relay()
        await relay?.ready().catch(() => undefined)
        for (let waited = 0; relay !== undefined && !relay.connected() && waited < CONNECT_WAIT_MS; waited += 250) await new Promise((done) => setTimeout(done, 250))
        await serial(catchUp)
      })().catch(() => undefined).finally(() => { catching = undefined })
      return catching
    },

    /** After Scout looked: what it told, for the next away look. */
    told: (): void => { void serial(async () => { await load(); await leaveTold() }).catch(() => undefined) },

    /** Scout's hours changed: the secret says them again. */
    hoursChanged: (): void => {
      void serial(async () => {
        const held = await load()
        const place = deps.relay()?.room()
        if (held === undefined || place === undefined || place.room !== held.room) return
        await deps.ghInput(['secret', 'set', AWAY_SECRET, '--repo', held.repo], deps.folder() ?? held.folder, JSON.stringify(await configOf(held, place.relay))).catch(() => undefined)
      }).catch(() => undefined)
    },

    /** The phones' way from anywhere, or the window's folder, changed: whether it can be turned on may have too. */
    nudge: (): void => { void serial(async () => { await load(); if (!(await roomGone())) tell() }).catch(() => undefined) },

    /** For a paired phone: the key to open the letters, and the repository they are about. */
    forPhone: async (): Promise<{ readonly key: string; readonly repo: string } | undefined> => {
      const held = await load()
      return held === undefined ? undefined : { key: held.key, repo: held.repo }
    }
  }
}
