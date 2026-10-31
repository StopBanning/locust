// What may never reach this public repository, checked before it does.
//
//   node _tools/public-guard.mjs --staged             what a commit is about to add (the pre-commit hook)
//   node _tools/public-guard.mjs --message <file>     a commit's message (the commit-msg hook)
//   node _tools/public-guard.mjs --range <a>..<b>     every commit in a range (CI, on each push)
//
// Development moved into this repository on 2026-10-05, with its whole history
// cleaned first. From here a slip is public within seconds, so every commit --
// a person's, a helper agent's -- passes this first. It reuses the public
// export's own definitions (public-export.mjs), so the two cannot disagree:
//
// - a private path (plans, notes, session records, screenshots under docs/):
//   refused, with the rule that matched;
// - the owner's account folder name, and any term of the owner's deny file:
//   refused, with no way past -- the deny file lives outside the repository
//   (LOCUST_DENY_FILE, default ~/Documents/Codex/.locust-release/export-deny.txt);
//   where it is absent (CI) only the account name is checked, and hits name a
//   term's number, never its text;
// - a key-shaped string (sk-, ghp_, AKIA, a PEM block, xox), an email outside
//   the test and product domains, a phone number outside 555: refused, unless
//   the line says `public-guard: allow` -- a made-up fixture says so on its
//   own line, where a reader of the diff sees it.
//
// Only ADDED lines are read: what is already public stays as it is.

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { join } from 'node:path'

import { SCAN_PATTERNS, accountRemains, denyHits, denyTerms, exclusionRule } from './public-export.mjs'

const ALLOW_MARK = 'public-guard: allow'
const WORKFLOWS = '.github/workflows/**'
// Addresses that are product, vendor or made up -- not a person's inbox. Retina image assets (@2x.png) are not emails.
const OK_EMAIL = /@(?:[2-4]x\.png|(?:[a-z0-9-]+\.)*(?:test|example|invalid|local|locust\.lol|example\.com|example\.org|anthropic\.com|cursor\.com|github\.com|users\.noreply\.github\.com|openai\.com|google\.com))$/i
const OK_PHONE = /(?:^|\D)555\D/

function git(args) {
  return execFileSync('git', args, { encoding: 'utf8', maxBuffer: 1 << 28 })
}

export function readDenyTerms(path = process.env.LOCUST_DENY_FILE ?? join(homedir(), 'Documents', 'Codex', '.locust-release', 'export-deny.txt')) {
  return existsSync(path) ? denyTerms(readFileSync(path, 'utf8')) : undefined
}

/** One added line's problems, said without repeating a private term. */
export function lineProblems(line, terms) {
  const found = []
  if (accountRemains(line)) found.push("the owner's account folder name")
  if (terms !== undefined) for (const n of denyHits(line, terms)) found.push(`deny term #${String(n)}`)
  if (found.length > 0 || line.includes(ALLOW_MARK)) return found
  for (const pattern of SCAN_PATTERNS) {
    pattern.re.lastIndex = 0
    for (const match of line.matchAll(pattern.re)) {
      if (pattern.name === 'email' && OK_EMAIL.test(match[0])) continue
      if (pattern.name === 'phone' && OK_PHONE.test(` ${match[0]} `)) continue
      found.push(`${pattern.name}-shaped text (${String(match[0].length)} chars)`)
    }
  }
  return found
}

/** A unified diff's added lines, by file: the guard's whole view of a change. */
export function addedLines(diff) {
  const byFile = new Map()
  let file
  for (const line of diff.split('\n')) {
    if (line.startsWith('+++ ')) {
      file = line.slice(4).replace(/^b\//, '')
      if (file === '/dev/null') file = undefined
      continue
    }
    if (file !== undefined && line.startsWith('+') && !line.startsWith('+++')) {
      if (!byFile.has(file)) byFile.set(file, [])
      byFile.get(file).push(line.slice(1))
    }
  }
  return byFile
}

/** Everything wrong with a change: its paths, then its added lines. */
export function changeProblems(paths, diff, terms) {
  const problems = []
  for (const path of paths) {
    const rule = exclusionRule(path)
    // The export kept the PRIVATE repository's workflows out; here the workflows are this repository's own.
    if (rule !== null && rule !== WORKFLOWS) problems.push(`${path}: a private path (${rule})`)
  }
  for (const [file, lines] of addedLines(diff)) {
    lines.forEach((line, index) => {
      for (const what of lineProblems(line, terms)) problems.push(`${file}: added line ${String(index + 1)}: ${what}`)
    })
  }
  return problems
}

function report(problems, what) {
  if (problems.length === 0) return 0
  console.error(`public-guard: ${what} refused -- this repository is public.`)
  for (const problem of problems.slice(0, 40)) console.error(`  ${problem}`)
  if (problems.length > 40) console.error(`  ... and ${String(problems.length - 40)} more`)
  console.error(`A made-up value in a test may say "${ALLOW_MARK}" on its line. Private notes and records belong in the private repository.`)
  return 1
}

function main(argv) {
  const terms = readDenyTerms()
  if (argv[0] === '--staged') {
    const paths = git(['diff', '--cached', '--name-only', '--diff-filter=ACMR', '-z']).split('\0').filter(Boolean)
    const diff = git(['diff', '--cached', '--no-color', '--no-ext-diff', '-U0', '--diff-filter=ACMR'])
    return report(changeProblems(paths, diff, terms), 'this commit')
  }
  if (argv[0] === '--message' && argv[1] !== undefined) {
    const text = readFileSync(argv[1], 'utf8')
    const problems = text.split('\n').flatMap((line, index) => lineProblems(line, terms).filter((what) => !/-shaped/.test(what)).map((what) => `message line ${String(index + 1)}: ${what}`))
    return report(problems, 'this commit message')
  }
  if (argv[0] === '--range' && argv[1] !== undefined) {
    const problems = []
    for (const commit of git(['rev-list', '--no-merges', argv[1]]).split('\n').filter(Boolean)) {
      const paths = git(['diff-tree', '--no-commit-id', '--name-only', '-r', '--diff-filter=ACMR', '-z', commit]).split('\0').filter(Boolean)
      const diff = git(['show', '--format=', '--no-color', '--no-ext-diff', '-U0', '--diff-filter=ACMR', commit])
      for (const problem of changeProblems(paths, diff, terms)) problems.push(`${commit.slice(0, 8)} ${problem}`)
      const message = git(['log', '-1', '--format=%B', commit])
      message.split('\n').forEach((line, index) => {
        for (const what of lineProblems(line, terms).filter((w) => !/-shaped/.test(w))) problems.push(`${commit.slice(0, 8)} message line ${String(index + 1)}: ${what}`)
      })
    }
    return report(problems, `the range ${argv[1]}`)
  }
  console.error('usage: node _tools/public-guard.mjs --staged | --message <file> | --range <a>..<b>')
  return 2
}

if (process.argv[1] !== undefined && /public-guard\.mjs$/.test(process.argv[1])) process.exit(main(process.argv.slice(2)))
