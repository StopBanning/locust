import { createHash, randomUUID } from 'node:crypto'
import { mkdir, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'

import type {
  SkillLibraryFile,
  SkillLibraryInstallRequest,
  SkillLibraryKept,
  SkillLibraryPreview,
  SkillLibrarySkill,
  SkillLibrarySource
} from '../shared/ipc.js'
import { replaceFile } from './replace-file.js'

/**
 * SKILLS FROM A GITHUB REPOSITORY (0.710).
 *
 * A person names a public repository; this lists the skill folders in it (a
 * folder holding a SKILL.md, Claude Code's own rule) with every file each one
 * carries, and marks the files a run could execute. The person keeps the ones
 * they pick. What is kept is the one commit the list was read from: a branch
 * that moves afterwards changes nothing until they look again and keep again.
 *
 * WHY THIS CARE. A skill is instructions a teammate follows, and often scripts
 * it runs -- in Auto, without asking. So nothing is fetched beyond the list
 * until the person has seen it; every file is downloaded at the pinned commit
 * and checked against the object id GitHub's own tree gives it (a git blob
 * hash), so what lands is byte for byte what the list described; links and
 * submodules are never followed; and a skill keeps the same size bounds a
 * folder's own skills have (claude-skills.ts).
 *
 * Reads only: two GitHub API calls to look (the repository, its tree at the
 * commit) plus each SKILL.md for its description, and raw files to keep.
 * Public repositories only -- no sign-in is asked for or sent.
 *
 * Kept skills live in the profile, `<root>/skills/<name>/`, one flat folder
 * that claude-skills.ts hands to Claude Code runs as the `library` plugin;
 * `<root>/library.json` records which repository and commit each came from.
 */

const API = 'https://api.github.com'
const RAW = 'https://raw.githubusercontent.com'
const SKILL_FILE = 'SKILL.md'
/** The same bounds a folder's own skills have (claude-skills.ts). */
// Bundled canvas-design fonts exceed 5 MB; 10 MB admits them with a bounded payload.
const MOST_BYTES_A_SKILL = 10 * 1024 * 1024
const MOST_BYTES_ALL = 40 * 1024 * 1024
const MOST_SKILLS = 200
const MOST_FILES_A_SKILL = 500
const SKILL_NAME = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/
const OWNER = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,38})$/
const REPO = /^[A-Za-z0-9._-]{1,100}$/
const SHA = /^[0-9a-f]{40}$/
/**
 * A file a run could execute: by its extension, or because git records it
 * executable (mode 100755). Shown to the person before anything is kept.
 */
const RUNS = /\.(?:sh|bash|zsh|fish|ps1|psm1|bat|cmd|py|pyw|js|mjs|cjs|ts|mts|cts|rb|pl|php|lua|exe|dll|so|dylib|jar|vbs|wsf|applescript|scpt|command)$/i
/** Characters and names Windows cannot hold in a path; a repository made on Linux can carry them. */
const WINDOWS_BAD = /[<>:"\\|?*\u0000-\u001f]|[. ]$/
const WINDOWS_RESERVED = /^(?:con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\..*)?$/i
const TIMEOUT_MS = 20_000
const AT_ONCE = 6
const GITHUB_HOSTS = new Set(['api.github.com', 'raw.githubusercontent.com'])
const MOST_REDIRECTS = 3

export type Fetcher = (
  url: string,
  init?: { readonly headers?: Record<string, string>; readonly signal?: AbortSignal; readonly redirect?: 'manual' }
) => Promise<Response>

export class SkillLibraryError extends Error {}

export interface RepositoryLink {
  readonly owner: string
  readonly repo: string
  readonly ref?: string
  /** A folder inside the repository the link pointed at: only skills under it are listed. */
  readonly path?: string
}

/**
 * `owner/repo`, `github.com/owner/repo`, or a link GitHub's own page gives:
 * `https://github.com/owner/repo/tree/<branch>/<folder>`. A branch name with a
 * slash in it is read as branch + folder, as GitHub's page does when it can.
 */
export function parseRepositoryLink(text: string): RepositoryLink | undefined {
  let rest = text.trim().replace(/[?#].*$/, '').replace(/\/+$/, '')
  rest = rest.replace(/^(?:https?:\/\/)?(?:www\.)?github\.com\//i, '')
  if (/^[a-z][a-z0-9+.-]*:/i.test(rest)) return undefined
  const parts = rest.split('/')
  const owner = parts[0]
  const repo = parts[1]?.replace(/\.git$/i, '')
  if (owner === undefined || repo === undefined || !OWNER.test(owner) || !REPO.test(repo) || repo === '.' || repo === '..') return undefined
  if (parts.length === 2) return { owner, repo }
  if (parts[2] !== 'tree' || parts[3] === undefined || parts[3] === '') return undefined
  const ref = decodeURIComponent(parts[3])
  const folder = parts.slice(4).map((part) => decodeURIComponent(part))
  if (folder.some((part) => part === '' || part === '.' || part === '..')) return undefined
  return { owner, repo, ref, ...(folder.length === 0 ? {} : { path: folder.join('/') }) }
}

interface TreeEntry {
  readonly path: string
  readonly mode: string
  readonly type: string
  readonly sha: string
  readonly size?: number
}

interface FoundSkill extends SkillLibrarySkill {
  readonly entries: readonly TreeEntry[]
}

function megabytes(bytes: number): string {
  return `${String(Math.round(bytes / (1024 * 1024)))} MB`
}

/** Only agent skill roots collapse. Other same-named folders remain real clashes. */
const AGENT_SKILLS = /(^|\/)(?:\.agent|\.agents|\.claude|\.codex|\.cursor|\.opencode|\.github|\.devin|\.dsh|\.gemini|\.grok|\.hermes|\.kiro|\.omp|\.pi|\.qoder|\.rovodev|\.trae-cn|\.trae|\.veto|\.vibe)\/skills\//
const copyRank = (folder: string): number => !AGENT_SKILLS.test(folder) ? 0 : /(^|\/)\.claude\/skills\//.test(folder) ? 1 : /(^|\/)\.agents\/skills\//.test(folder) ? 2 : 3

/** The skill folders a commit's tree holds, each with its files, bounded and checked. */
export function skillsInTree(tree: readonly TreeEntry[], repo: string, under?: string): readonly FoundSkill[] {
  const inside = (path: string, folder: string): boolean => folder === '' || path.startsWith(`${folder}/`)
  const folders = tree
    .filter((entry) => entry.type === 'blob' && (entry.path === SKILL_FILE || entry.path.endsWith(`/${SKILL_FILE}`)))
    .map((entry) => entry.path.slice(0, Math.max(0, entry.path.length - SKILL_FILE.length - 1)))
    .filter((folder) => under === undefined || folder === under || folder.startsWith(`${under}/`))
    .filter((folder) => !folder.split('/').includes('.git'))
    .sort((a, b) => a.localeCompare(b))
  // A SKILL.md inside another skill's folder is part of that skill, not one of its own.
  const outermost = folders.filter((folder) => !folders.some((other) => other !== folder && inside(folder, other) && other.length < folder.length))
  const copies = new Map<string, string>()
  for (const folder of outermost) {
    const identity = folder.replace(AGENT_SKILLS, '$1skills/')
    const previous = copies.get(identity)
    if (previous === undefined || copyRank(folder) < copyRank(previous)) copies.set(identity, folder)
  }
  const unique = [...copies.values()].sort((a, b) => a.localeCompare(b))
  const found: FoundSkill[] = []
  const named = new Map<string, string>()
  for (const folder of unique.slice(0, MOST_SKILLS)) {
    const name = folder === '' ? repo : folder.slice(folder.lastIndexOf('/') + 1)
    // Links and submodules are never carried: a kept skill holds only real files.
    const entries = tree.filter((entry) => entry.type === 'blob' && entry.mode !== '120000' && inside(entry.path, folder))
    const files: SkillLibraryFile[] = entries.map((entry) => {
      const path = folder === '' ? entry.path : entry.path.slice(folder.length + 1)
      return { path, bytes: entry.size ?? 0, runs: entry.mode === '100755' || RUNS.test(path) }
    })
    const bytes = files.reduce((sum, file) => sum + file.bytes, 0)
    const badPath = files.find((file) => file.path.split('/').some((part) => part === '' || part === '.' || part === '..' || WINDOWS_BAD.test(part) || WINDOWS_RESERVED.test(part)))
    const twin = named.get(name.toLowerCase())
    const held = !SKILL_NAME.test(name)
      ? `Its folder's name, "${name}", is not one Claude Code takes for a skill.`
      : twin !== undefined
        ? `Another folder here has the same name (${twin || '(top)'}); only the first is offered.`
        : bytes > MOST_BYTES_A_SKILL
          ? `Larger than ${megabytes(MOST_BYTES_A_SKILL)} (${megabytes(bytes)}): a skill is instructions, not a payload.`
          : files.length > MOST_FILES_A_SKILL
            ? `More than ${String(MOST_FILES_A_SKILL)} files.`
            : badPath !== undefined
              ? `It holds a file Windows cannot keep: ${badPath.path}.`
              : undefined
    if (twin === undefined) named.set(name.toLowerCase(), folder)
    found.push({ name, folder, bytes, files, entries, ...(held === undefined ? {} : { held }) })
  }
  return found
}

/** The `description:` of a SKILL.md's front matter, one line, bounded. */
export function skillDescription(text: string): string | undefined {
  const front = /^\uFEFF?---\r?\n([\s\S]*?)\r?\n---/.exec(text)?.[1]
  const line = front === undefined ? undefined : /^description:[ \t]*(.+)$/m.exec(front)?.[1]
  if (line === undefined) return undefined
  const value = line.trim().replace(/^(["'])(.*)\1$/, '$2').trim()
  if (value === '' || value === '|' || value === '>') return undefined
  return value.length > 300 ? `${value.slice(0, 299)}…` : value
}

/** Git's own id for a file's bytes: what a tree entry's `sha` is. */
export function gitBlobId(bytes: Uint8Array): string {
  return createHash('sha1').update(`blob ${String(bytes.length)}\0`).update(bytes).digest('hex')
}

/*
 * A failure stops new work and is reported only once the work already started
 * has ended. It used to reject at once, so install's cleanup removed the
 * staging folder while other lanes were still writing into it, and a late
 * write made it again: a refused install left a `.staging-*` folder behind
 * (the 0.711 ship's unit run, skills-kept-from-github).
 */
async function atMostAtOnce<T, R>(items: readonly T[], work: (item: T) => Promise<R>): Promise<R[]> {
  const results: R[] = new Array<R>(items.length)
  let next = 0
  let failed: { readonly error: unknown } | undefined
  const lane = async (): Promise<void> => {
    while (next < items.length && failed === undefined) {
      const at = next
      next += 1
      try {
        results[at] = await work(items[at] as T)
      } catch (error) {
        failed ??= { error }
      }
    }
  }
  await Promise.all(Array.from({ length: Math.min(AT_ONCE, items.length) }, () => lane()))
  if (failed !== undefined) throw failed.error
  return results
}

interface LibraryRecord {
  readonly version: 1
  readonly sources: readonly SkillLibrarySource[]
}

export interface SkillLibrary {
  preview(link: string): Promise<SkillLibraryPreview>
  install(request: SkillLibraryInstallRequest): Promise<readonly SkillLibrarySource[]>
  list(): Promise<readonly SkillLibrarySource[]>
  remove(source: string): Promise<readonly SkillLibrarySource[]>
  /** The one folder of kept skills, `<root>/skills`: what a Claude run is handed as the `library` plugin. */
  readonly skillsFolder: string
}

export function createSkillLibrary(options: { readonly root: string; readonly fetch?: Fetcher; readonly now?: () => Date }): SkillLibrary {
  const fetcher: Fetcher = options.fetch ?? ((url, init) => fetch(url, init))
  const now = options.now ?? (() => new Date())
  const skillsFolder = join(options.root, 'skills')
  const recordFile = join(options.root, 'library.json')
  // One change at a time: two installs at once must not interleave their swaps.
  let queue: Promise<unknown> = Promise.resolve()
  const serially = <T>(work: () => Promise<T>): Promise<T> => {
    const run = queue.then(work, work)
    queue = run.catch(() => undefined)
    return run
  }

  const request = async (first: string, what: string): Promise<Response> => {
    let url = first
    let answer: Response | undefined
    // A redirect is followed only to GitHub's own two hosts (a renamed repository answers 301), never elsewhere.
    for (let hop = 0; hop <= MOST_REDIRECTS; hop += 1) {
      try {
        answer = await fetcher(url, {
          headers: { 'User-Agent': 'Locust', Accept: url.startsWith(API) ? 'application/vnd.github+json' : '*/*' },
          signal: AbortSignal.timeout(TIMEOUT_MS),
          redirect: 'manual'
        })
      } catch {
        throw new SkillLibraryError('Locust could not reach GitHub. Check the connection and try again.')
      }
      if (answer.status < 300 || answer.status >= 400) break
      const next = answer.headers.get('location')
      const resolved = next === null ? undefined : (() => { try { return new URL(next, url) } catch { return undefined } })()
      if (resolved === undefined || resolved.protocol !== 'https:' || !GITHUB_HOSTS.has(resolved.host)) {
        throw new SkillLibraryError('GitHub sent Locust somewhere other than GitHub. Nothing was read from there.')
      }
      url = resolved.href
      if (hop === MOST_REDIRECTS) throw new SkillLibraryError('GitHub kept redirecting. Try again in a minute.')
    }
    if (answer === undefined) throw new SkillLibraryError('Locust could not reach GitHub. Check the connection and try again.')
    if (answer.ok) return answer
    if ((answer.status === 403 || answer.status === 429) && answer.headers.get('x-ratelimit-remaining') === '0') {
      const reset = Number(answer.headers.get('x-ratelimit-reset'))
      const until = Number.isFinite(reset) && reset > 0
        ? ` until ${new Date(reset * 1000).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })}`
        : ' for now'
      throw new SkillLibraryError(`GitHub's limit for looking without signing in is used up${until}. Try again then.`)
    }
    if (answer.status === 404) throw new SkillLibraryError(what)
    throw new SkillLibraryError(`GitHub answered ${String(answer.status)} when Locust asked for ${url.startsWith(RAW) ? 'a file' : 'the repository'}. Try again in a minute.`)
  }
  const json = async (url: string, what: string): Promise<unknown> => {
    try {
      return await (await request(url, what)).json()
    } catch (error) {
      if (error instanceof SkillLibraryError) throw error
      throw new SkillLibraryError('GitHub sent something Locust could not read. Try again in a minute.')
    }
  }

  const treeAt = async (owner: string, repo: string, sha: string): Promise<{ readonly tree: readonly TreeEntry[]; readonly partial: boolean }> => {
    const body = (await json(`${API}/repos/${owner}/${repo}/git/trees/${sha}?recursive=1`, `GitHub has no commit ${sha.slice(0, 7)} in ${owner}/${repo}.`)) as {
      readonly tree?: unknown
      readonly truncated?: unknown
    }
    const tree = Array.isArray(body.tree)
      ? (body.tree as unknown[]).filter((entry): entry is TreeEntry => {
          const e = entry as Partial<TreeEntry>
          return typeof e.path === 'string' && typeof e.mode === 'string' && typeof e.type === 'string' && typeof e.sha === 'string'
        })
      : []
    return { tree, partial: body.truncated === true }
  }

  const readRecord = async (): Promise<LibraryRecord> => {
    try {
      const parsed = JSON.parse(await readFile(recordFile, 'utf8')) as Partial<LibraryRecord>
      const sources = Array.isArray(parsed.sources)
        ? parsed.sources.filter((source): source is SkillLibrarySource =>
            typeof source?.source === 'string' && typeof source.sha === 'string' && Array.isArray(source.skills))
        : []
      return { version: 1, sources }
    } catch {
      return { version: 1, sources: [] }
    }
  }
  const writeRecord = async (record: LibraryRecord): Promise<void> => {
    await mkdir(options.root, { recursive: true })
    const temporary = `${recordFile}.${randomUUID()}.tmp`
    await writeFile(temporary, `${JSON.stringify(record, null, 2)}\n`, 'utf8')
    await replaceFile(temporary, recordFile)
  }

  const splitSource = (source: string): { readonly owner: string; readonly repo: string } => {
    const [owner, repo, extra] = source.split('/')
    if (owner === undefined || repo === undefined || extra !== undefined || !OWNER.test(owner) || !REPO.test(repo)) {
      throw new SkillLibraryError('That is not a repository Locust can read.')
    }
    return { owner, repo }
  }

  const sameSource = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase()

  return {
    skillsFolder,

    async preview(link) {
      const parsed = parseRepositoryLink(link)
      if (parsed === undefined) {
        throw new SkillLibraryError('Paste a GitHub repository: owner/repo, or its github.com link.')
      }
      const missing = `No public repository at ${parsed.owner}/${parsed.repo}. Locust reads public repositories only.`
      const about = (await json(`${API}/repos/${parsed.owner}/${parsed.repo}`, missing)) as {
        readonly full_name?: unknown
        readonly default_branch?: unknown
        readonly private?: unknown
      }
      if (about.private === true) throw new SkillLibraryError(missing)
      const source = typeof about.full_name === 'string' && /^[^/]+\/[^/]+$/.test(about.full_name) ? about.full_name : `${parsed.owner}/${parsed.repo}`
      const { owner, repo } = splitSource(source)
      const ref = parsed.ref ?? (typeof about.default_branch === 'string' ? about.default_branch : 'HEAD')
      const commit = (await json(
        `${API}/repos/${owner}/${repo}/commits/${encodeURIComponent(ref)}`,
        `${source} has no branch, tag or commit called "${ref}".`
      )) as { readonly sha?: unknown }
      if (typeof commit.sha !== 'string' || !SHA.test(commit.sha)) throw new SkillLibraryError('GitHub sent something Locust could not read. Try again in a minute.')
      const sha = commit.sha
      const { tree, partial } = await treeAt(owner, repo, sha)
      const found = skillsInTree(tree, repo, parsed.path)
      if (found.length === 0) {
        throw new SkillLibraryError(
          parsed.path === undefined
            ? `${source} has no skills: no folder in it holds a SKILL.md.${partial ? ' GitHub listed only part of it; link a folder inside it instead.' : ''}`
            : `${source} has no skills under ${parsed.path}: no folder there holds a SKILL.md.`
        )
      }
      const record = await readRecord()
      const descriptions = await atMostAtOnce(found, async (skill) => {
        const at = skill.entries.find((entry) => entry.path === (skill.folder === '' ? SKILL_FILE : `${skill.folder}/${SKILL_FILE}`))
        if (at === undefined || (at.size ?? 0) > 256 * 1024) return undefined
        try {
          return skillDescription(await (await request(rawUrl(owner, repo, sha, at.path), '')).text())
        } catch {
          return undefined
        }
      })
      const skills: SkillLibrarySkill[] = found.map((skill, index) => {
        const keeper = record.sources.find((kept) => kept.skills.some((one) => one.name.toLowerCase() === skill.name.toLowerCase()))
        const keptHere = keeper !== undefined && sameSource(keeper.source, source)
        const held = skill.held ?? (keeper !== undefined && !keptHere ? `A skill called ${skill.name} is kept from ${keeper.source}. Remove that one to keep this.` : undefined)
        const description = descriptions[index]
        return {
          name: skill.name,
          folder: skill.folder,
          bytes: skill.bytes,
          files: skill.files,
          ...(description === undefined ? {} : { description }),
          ...(held === undefined ? {} : { held }),
          ...(keptHere ? { keptAt: keeper.sha } : {})
        }
      })
      return { source, ref, sha, skills, partial, ...(parsed.path === undefined ? {} : { path: parsed.path }) }
    },

    install(asked) {
      return serially(async () => {
        const { owner, repo } = splitSource(String(asked?.source ?? ''))
        const source = `${owner}/${repo}`
        const sha = String(asked?.sha ?? '')
        const ref = typeof asked?.ref === 'string' && asked.ref.length <= 255 ? asked.ref : sha
        if (!SHA.test(sha)) throw new SkillLibraryError('Look at the repository again, then keep.')
        const names = Array.isArray(asked?.names) ? [...new Set(asked.names.map(String))] : []
        if (names.length === 0 || names.length > MOST_SKILLS || names.some((name) => !SKILL_NAME.test(name))) {
          throw new SkillLibraryError('Pick at least one skill to keep.')
        }
        // The tree again, at the same commit: a commit never changes, so this is the list the person saw.
        const { tree } = await treeAt(owner, repo, sha)
        // The folder the link named, as the preview listed it: over the whole tree, a same-named folder elsewhere
        // was kept instead, and a root SKILL.md hid every folder's (2026-10-10 sweep).
        const under = typeof asked?.path === 'string' && asked.path.length > 0 && asked.path.length <= 1000 && !asked.path.split('/').includes('..') ? asked.path : undefined
        const found = skillsInTree(tree, repo, under)
        const record = await readRecord()
        const others = record.sources.filter((kept) => !sameSource(kept.source, source))
        const chosen = names.map((name) => {
          const skill = found.find((one) => one.name === name && one.held === undefined)
          if (skill === undefined) throw new SkillLibraryError(`${name} is not a skill Locust can keep from that commit.`)
          const keeper = others.find((kept) => kept.skills.some((one) => one.name.toLowerCase() === name.toLowerCase()))
          if (keeper !== undefined) throw new SkillLibraryError(`A skill called ${name} is kept from ${keeper.source}. Remove that one to keep this.`)
          return skill
        })
        const othersBytes = others.flatMap((kept) => kept.skills).reduce((sum, skill) => sum + skill.bytes, 0)
        const bytes = chosen.reduce((sum, skill) => sum + skill.bytes, 0)
        if (othersBytes + bytes > MOST_BYTES_ALL) {
          throw new SkillLibraryError(`That would keep more than ${megabytes(MOST_BYTES_ALL)} of skills in all. Keep fewer, or remove a repository first.`)
        }

        const staging = join(options.root, `.staging-${randomUUID()}`)
        try {
          for (const skill of chosen) {
            await atMostAtOnce(skill.entries, async (entry) => {
              const inSkill = skill.folder === '' ? entry.path : entry.path.slice(skill.folder.length + 1)
              const answer = await request(rawUrl(owner, repo, sha, entry.path), `${entry.path} is missing from ${source} at ${sha.slice(0, 7)}.`)
              const body = new Uint8Array(await answer.arrayBuffer())
              if (gitBlobId(body) !== entry.sha) {
                throw new SkillLibraryError(`${skill.name}/${inSkill} did not arrive as GitHub lists it. Nothing was kept; try again.`)
              }
              const target = join(staging, skill.name, ...inSkill.split('/'))
              await mkdir(join(target, '..'), { recursive: true })
              await writeFile(target, body)
            })
          }
          // Every file arrived and checked: only now does anything kept change.
          await mkdir(skillsFolder, { recursive: true })
          const before = record.sources.find((kept) => sameSource(kept.source, source))
          for (const skill of before?.skills ?? []) await rm(join(skillsFolder, skill.name), { recursive: true, force: true })
          for (const skill of chosen) {
            await rm(join(skillsFolder, skill.name), { recursive: true, force: true })
            await rename(join(staging, skill.name), join(skillsFolder, skill.name))
          }
          const kept: SkillLibrarySource = {
            source,
            ref,
            sha,
            keptAt: now().toISOString(),
            skills: chosen.map((skill): SkillLibraryKept => ({
              name: skill.name,
              files: skill.files.length,
              bytes: skill.bytes,
              runs: skill.files.filter((file) => file.runs).map((file) => file.path)
            }))
          }
          const sources = [...others, kept].sort((a, b) => a.source.localeCompare(b.source))
          await writeRecord({ version: 1, sources })
          return sources
        } finally {
          await rm(staging, { recursive: true, force: true }).catch(() => undefined)
        }
      })
    },

    async list() {
      return (await readRecord()).sources
    },

    remove(source) {
      return serially(async () => {
        const record = await readRecord()
        const gone = record.sources.find((kept) => sameSource(kept.source, String(source)))
        if (gone === undefined) return record.sources
        for (const skill of gone.skills) {
          if (SKILL_NAME.test(skill.name)) await rm(join(skillsFolder, skill.name), { recursive: true, force: true })
        }
        const sources = record.sources.filter((kept) => kept !== gone)
        await writeRecord({ version: 1, sources })
        return sources
      })
    }
  }
}

function rawUrl(owner: string, repo: string, sha: string, path: string): string {
  return `${RAW}/${owner}/${repo}/${sha}/${path.split('/').map((part) => encodeURIComponent(part)).join('/')}`
}

