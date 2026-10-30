// Where a harness is allowed to put a scratch workspace.
//
// Import this FIRST, for its side effect:
//
//   import './scratch-root.mjs'      // before anything calls tmpdir()
//
// MEASURED 2026-09-07. Cursor could not read or edit ANY file in a scratch
// workspace, and said so itself: "Every file-editing path was blocked:
// StrReplace / Write / Delete -> Read permission denied ... The file is
// ignored by `.cursorignore`". Same command, same model, minutes apart:
//
//   C:\Users\<user>\AppData\Local\Temp\...   every path blocked, 102s, no edit
//   C:\Users\<user>\Documents\...            edited correctly, 8.7s
//
// The cause is not Locust and not Cursor. `~/.cursorignore` on this machine
// carries `AppData/`, added deliberately on 2026-09-06 to stop Cursor
// snapshot-indexing multi-GB agent transcripts (~14 GB/hr). Every harness here
// builds its workspace with `mkdtemp(tmpdir())`, and on Windows tmpdir() IS
// inside AppData -- so every Cursor run was being handed a folder its own
// config told it to ignore.
//
// That is worth stating plainly because of what it cost: `diff-smoke` failed
// with ten red checks that all looked like a diff-rendering defect, and the
// runtime had simply never been allowed to open the file. A harness that tests
// the machine's configuration instead of the product reports a defect that
// does not exist -- and the reverse is worse, because a run that cannot touch
// anything also cannot fail the way a real one would.
//
// The fix belongs here rather than in that file: the ignore was added for a
// real reason and should stay. tmpdir() reads TEMP at call time, so pointing
// it somewhere outside AppData fixes every `mkdtemp(tmpdir())` in the tree at
// once, without editing any of them.
//
// Codex, Claude Code and OpenCode all work fine under AppData; only Cursor
// reads `.cursorignore`. This applies to all of them anyway, because a scratch
// root that depends on which runtime you picked is the kind of thing that is
// true until someone changes a route.

import { mkdirSync, readFileSync, readdirSync, rmSync, statSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join } from 'node:path'

/**
 * Outside AppData, outside the repository, and with no instruction file above
 * it.
 *
 * Not inside the repo: several harnesses `git init` their workspace, and a git
 * repository nested inside this one is a trap for anything that walks the
 * tree. `LOCUST_SCRATCH` overrides it for a machine laid out differently.
 *
 * Not under Documents either (2026-10-04). It was `~/Documents/locust-scratch`,
 * and Documents holds Colin's own CLAUDE.md (his business brief): Claude Code
 * loads every CLAUDE.md in the folders above the one it works in, so every
 * live Claude run a drive made in its own folder had that brief in its
 * context. Arena round 2's first Fable run did -- its session file lists it as
 * a Project instruction -- which broke the round's rule of no CLAUDE.md in or
 * above a model's folder. The home folder has none above it.
 */
export const SCRATCH_ROOT = process.env.LOCUST_SCRATCH ?? join(homedir(), 'locust-scratch')

/** Where the scratch root was until 2026-10-04: still pruned, so it empties on its own. */
export const LEGACY_SCRATCH_ROOT = join(homedir(), 'Documents', 'locust-scratch')

mkdirSync(SCRATCH_ROOT, { recursive: true })

// tmpdir() consults these every call, so this reaches code that was written
// against tmpdir() long before this file existed.
process.env.TEMP = SCRATCH_ROOT
process.env.TMP = SCRATCH_ROOT
process.env.TMPDIR = SCRATCH_ROOT

/**
 * Which `.cursorignore` rule hides this path, if one does.
 *
 * The warning below is the whole point of this export. Ten red checks in
 * diff-smoke described a diff that had not been rendered, and every one of
 * them was a runtime that had been refused the file -- the report named the
 * symptom furthest from the cause. A harness that can say "the runtime was
 * not allowed to look at this folder" costs one line and replaces an
 * afternoon.
 *
 * Directory rules only (`AppData/`, `node_modules/`). That covers what these
 * files actually carry and stays honest about what it does not check: a glob
 * over file names is not read here, so a quiet return is "no directory rule
 * matched", never "Cursor can definitely see this".
 */
export function cursorIgnoreHit(path, ignoreFile = join(homedir(), '.cursorignore')) {
  let text
  try {
    text = readFileSync(ignoreFile, 'utf8')
  } catch {
    return undefined
  }
  const segments = new Set(path.split(/[\\/]+/).filter(Boolean))
  for (const raw of text.split('\n')) {
    const line = raw.trim()
    if (line.length === 0 || line.startsWith('#') || line.startsWith('!')) continue
    if (!line.endsWith('/')) continue
    const name = line.replace(/^\*\*\//, '').replace(/\/$/, '')
    if (name.length > 0 && segments.has(name)) return { rule: line, file: ignoreFile }
  }
  return undefined
}

const hidden = cursorIgnoreHit(SCRATCH_ROOT)
if (hidden !== undefined) {
  // Not thrown: every drive imports this file, and most of them never touch
  // Cursor. A run that cannot see its own workspace fails loudly enough on
  // its own -- what it has never done is say why.
  console.warn(
    `\n  !! The scratch root is hidden from Cursor.\n` +
      `     ${SCRATCH_ROOT}\n` +
      `     matches ${JSON.stringify(hidden.rule)} in ${hidden.file}\n\n` +
      `     A Cursor run here is refused every read and write before it starts,\n` +
      `     and reports as a runtime that did nothing rather than one that was\n` +
      `     not allowed to. Set LOCUST_SCRATCH to a path outside that rule.\n`
  )
}

/**
 * Throw away scratch left by earlier runs.
 *
 * Moving off tmpdir() lost something that was never noticed: Windows clears
 * its temp folder, and nothing clears this one. Drives delete their profile
 * and deliberately KEEP their workspace, so a folder in the user's Documents
 * would otherwise grow one directory per run forever.
 *
 * Two days, so a run being inspected this morning is still there this
 * afternoon, and only entries this harness made: the prefix is required, the
 * entry must be a directory, and it must sit directly in the root. Anything
 * a person put here by hand is not touched.
 */
function pruneOldScratch(root, days = 2) {
  const cutoff = Date.now() - days * 24 * 60 * 60 * 1000
  let entries
  try {
    entries = readdirSync(root, { withFileTypes: true })
  } catch {
    return
  }
  for (const entry of entries) {
    if (!entry.isDirectory() || !entry.name.startsWith('locust-')) continue
    const path = join(root, entry.name)
    try {
      if (statSync(path).mtimeMs > cutoff) continue
      rmSync(path, { recursive: true, force: true })
    } catch {
      // In use, or gone already. Neither is worth interrupting a run for.
    }
  }
}

pruneOldScratch(SCRATCH_ROOT)
// The old root, by the same rules, until nothing of the harness's is left in it. Windows only: it never existed
// elsewhere, and on a Mac reading ~/Documents can stop on a privacy prompt (the first Mac run, 2026-10-30).
if (process.platform === 'win32' && LEGACY_SCRATCH_ROOT !== SCRATCH_ROOT) pruneOldScratch(LEGACY_SCRATCH_ROOT)

/**
 * The instruction files a runtime would read in a run made under `path`: any
 * CLAUDE.md (Claude Code reads every one from the folder up to the drive's
 * root) or AGENTS.md (Codex and others) in `path` or a folder above it.
 * Exported for the warning below and for a drive to check its own workspace.
 */
export function instructionFilesAbove(path) {
  const found = []
  let at = path
  for (;;) {
    for (const name of ['CLAUDE.md', 'AGENTS.md']) {
      try {
        if (statSync(join(at, name)).isFile()) found.push(join(at, name))
      } catch { /* none here */ }
    }
    const up = dirname(at)
    if (up === at) break
    at = up
  }
  return found
}

const instructions = instructionFilesAbove(SCRATCH_ROOT)
if (instructions.length > 0) {
  console.warn(
    `\n  !! An instruction file sits above the scratch root.\n` +
      `     ${SCRATCH_ROOT}\n` +
      `     ${instructions.join('\n     ')}\n\n` +
      `     Every live run a drive makes in its own folder would read it as the\n` +
      `     project's instructions. Set LOCUST_SCRATCH to a path with none above it.\n`
  )
}
