// Put Locust's releases on StopBanning/locust's Releases page (0.790), each at the commit it was built from.
//
//   node _tools/github-release.mjs <version>                after publish-dl.sh: the release people download, with
//                                                            the six installers dl.locust.lol serves and their sums
//   node _tools/github-release.mjs --history [--every <s>]  every older build in CHANGELOG.md, its notes only, one
//                                                            every <s> seconds (default 20); run again, it carries on
//
// WHY. GitHub suspended the account that held Locust's releases on 2026-10-16 (publish-downloads.mjs), and since
// then the code repository had no Releases page, which people expect of an open-source project. The page mirrors
// dl.locust.lol: each file is downloaded from there, checked against the SHA-256 that release.json publishes, and
// uploaded unchanged, so a copy from either place is the same file. As publish-release.mjs did, the release is a
// draft until every file is on it at its size, so nobody is ever shown a release with a file missing.
//
// A tag only ever names a commit already on the public main. A commit outside it would carry history that was
// never published: the four tags made before that history was rewritten for publishing (0.643.0 to 0.646.0) are
// the case, and those builds are tagged at the commit that set their version instead. Tags go up in batches of
// more than three, for which GitHub starts no workflow, so the history does not start a run per build.

import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { createWriteStream, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { Readable } from 'node:stream'
import { pipeline } from 'node:stream/promises'
import { REPO, changelogEntries, checksumsFile, releaseFileName, releaseNotes } from './github-release-notes.mjs'

const ROOT = new URL('..', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')
const CHANGELOG = join(ROOT, 'CHANGELOG.md')
const DOWNLOADS = 'https://dl.locust.lol/'
const REMOTE = 'stopbanning'
const CREDENTIAL = ['-c', 'credential.helper=', '-c', 'credential.helper=!gh auth git-credential']

const history = process.argv.includes('--history')
// --without-mac: a build whose Mac job GitHub never ran (0.835) is released with its four Windows and Linux files.
const withoutMac = process.argv.includes('--without-mac')
const version = history ? undefined : process.argv[2]
const everyAt = process.argv.indexOf('--every')
const every = everyAt === -1 ? 20 : Number(process.argv[everyAt + 1])
if ((!history && (version === undefined || !/^\d+\.\d+\.\d+$/.test(version))) || !(every >= 5)) {
  console.error('usage: node _tools/github-release.mjs <version> [--without-mac]   or   node _tools/github-release.mjs --history [--every <seconds, 5 or more>]')
  process.exit(2)
}

const say = (line) => console.log(line)
// Thrown, not exited, so the downloads folder is still cleared on the way out.
class Refused extends Error {}
const fail = (line) => {
  throw new Refused(line)
}
const sleep = (ms) => new Promise((done) => setTimeout(done, ms))

// No shell: notes and paths are single arguments, whatever they hold, and gh and git are found without one.
function run(command, args, what, { allowFailure = false } = {}) {
  const result = spawnSync(command, args, { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024, windowsHide: true })
  if (result.status !== 0 && !allowFailure) fail(`${what} failed (exit ${String(result.status)}): ${(result.stderr || result.stdout || '').trim().slice(0, 600)}`)
  return result
}
const git = (args, what, options) => run('git', args, what, options)
const gh = (args, what, options) => run('gh', args, what, options)

// GitHub's secondary limit on making things answers 403 or 429 and asks for a pause. Anything else is a failure.
async function ghPatient(args, what) {
  for (let attempt = 1; ; attempt += 1) {
    const result = gh(args, what, { allowFailure: true })
    if (result.status === 0) return result.stdout
    const said = `${result.stderr}${result.stdout}`
    if (attempt < 6 && /secondary rate limit|rate limit exceeded|HTTP 429/i.test(said)) {
      say(`  GitHub asked for a pause; waiting ${String(attempt)} minute(s)`)
      await sleep(attempt * 60_000)
      continue
    }
    fail(`${what} failed (exit ${String(result.status)}): ${said.trim().slice(0, 600)}`)
  }
}

/** The commit that first set apps/desktop/package.json to each version, on the public main. */
function versionCommits() {
  const log = git(['log', '--reverse', '--format=@@ %H', '-p', '-U0', `${REMOTE}/main`, '--', 'apps/desktop/package.json'], 'reading the version history').stdout
  const first = new Map()
  let commit
  for (const line of log.split('\n')) {
    const head = /^@@ ([0-9a-f]{40})$/.exec(line)
    if (head !== null) {
      commit = head[1]
      continue
    }
    const set = /^\+\s*"version":\s*"(\d+\.\d+\.\d+)"/.exec(line)
    if (set !== null && commit !== undefined && !first.has(set[1])) first.set(set[1], commit)
  }
  return first
}

const onPublicMain = (sha) => git(['merge-base', '--is-ancestor', sha, `${REMOTE}/main`], 'checking a commit', { allowFailure: true }).status === 0
const localTag = (name) => {
  const result = git(['rev-list', '-n', '1', `refs/tags/${name}`], 'reading a tag', { allowFailure: true })
  return result.status === 0 ? result.stdout.trim() : undefined
}

/**
 * Each version's tag on GitHub: the tag the release pipeline made where it is on the public main (it names the
 * commit the build was made from, sometimes one after the version was set), and otherwise the version's commit.
 */
function tagsUp(versions) {
  git([...CREDENTIAL, 'fetch', '-q', REMOTE, 'main'], 'fetching the public main')
  const up = new Set(git([...CREDENTIAL, 'ls-remote', '--tags', '--refs', REMOTE], 'listing the tags on GitHub').stdout
    .split('\n').map((line) => line.split('refs/tags/')[1]).filter(Boolean))
  const commits = versionCommits()
  const missing = []
  for (const name of versions) {
    if (up.has(name)) continue
    const tagged = localTag(name)
    const sha = tagged !== undefined && onPublicMain(tagged) ? tagged : commits.get(name)
    if (sha === undefined || !onPublicMain(sha)) fail(`${name} has no commit on the public main to tag`)
    missing.push(`${sha}:refs/tags/${name}`)
  }
  for (let at = 0; at < missing.length; at += 100) {
    git([...CREDENTIAL, 'push', '-q', REMOTE, ...missing.slice(at, at + 100)], `pushing tags ${String(at + 1)} to ${String(Math.min(at + 100, missing.length))}`)
  }
  if (missing.length > 0) say(`tagged ${String(missing.length)} build(s) on ${REPO}`)
}

/** The releases already there, drafts included: tag -> { id, draft }. */
function releasesUp() {
  const out = gh(['api', '--paginate', `repos/${REPO}/releases?per_page=100`, '-q', '.[] | [.tag_name, (.id | tostring), (.draft | tostring)] | join(" ")'], 'listing the releases').stdout
  return new Map(out.split('\n').filter(Boolean).map((line) => {
    const [tag, id, draft] = line.split(' ')
    return [tag, { id, draft: draft === 'true' }]
  }))
}

const work = mkdtempSync(join(tmpdir(), 'locust-github-release-'))
const payload = (name, value) => {
  const file = join(work, `${name}.json`)
  writeFileSync(file, JSON.stringify(value))
  return file
}

async function makeRelease(entry, previous, { draft, files = [] }) {
  const file = payload(entry.version, {
    tag_name: entry.version,
    name: `Locust ${entry.version}`,
    body: releaseNotes(entry, { previous, files }),
    draft,
    prerelease: false,
    make_latest: 'false'
  })
  return (await ghPatient(['api', '-X', 'POST', `repos/${REPO}/releases`, '--input', file, '-q', '.id'], `making the ${entry.version} release`)).trim()
}

/** A release's files as GitHub holds them: name -> { size, state }. */
function filesOn(id) {
  const out = gh(['api', `repos/${REPO}/releases/${id}/assets?per_page=100`, '-q', '.[] | [.name, (.size | tostring), .state] | join(" ")'], 'reading the release back').stdout
  return new Map(out.split('\n').filter(Boolean).map((line) => {
    const [name, size, state] = line.split(' ')
    return [name, { size: Number(size), state }]
  }))
}

async function releaseHistory() {
  const entries = changelogEntries(readFileSync(CHANGELOG, 'utf8'))
  // The newest build is released with its files by `<version>`; this is every one before it, oldest first, so the
  // page lists them in the order they came out however GitHub sorts.
  const older = entries.slice(1).reverse()
  tagsUp(older.map((entry) => entry.version))
  const up = releasesUp()
  const todo = older.filter((entry) => !up.has(entry.version))
  say(`${String(older.length - todo.length)} of ${String(older.length)} older builds have a release; making ${String(todo.length)}, one every ${String(every)} s`)
  for (const [done, entry] of todo.entries()) {
    const previous = entries[entries.indexOf(entry) + 1]?.version
    await makeRelease(entry, previous, { draft: false })
    say(`  ${entry.version} (${String(done + 1)}/${String(todo.length)})`)
    if (done + 1 < todo.length) await sleep(every * 1000)
  }
  say(`HISTORY RELEASED: ${String(older.length)} older builds on ${REPO}`)
}

async function releaseWithFiles() {
  const entries = changelogEntries(readFileSync(CHANGELOG, 'utf8'))
  const entry = entries.find((candidate) => candidate.version === version)
  if (entry === undefined) fail(`CHANGELOG.md has no entry for ${version}`)
  const previous = entries[entries.indexOf(entry) + 1]?.version

  // What dl.locust.lol offers must be this version, for every system.
  const list = await (await fetch(`${DOWNLOADS}release.json?check=${String(Date.now())}`, { headers: { 'Cache-Control': 'no-cache' } })).json()
  if (list.tag_name !== version) fail(`dl.locust.lol offers ${String(list.tag_name)}, not ${version}: publish it there first`)
  const wanted = withoutMac ? 4 : 6
  if (list.assets.length !== wanted) fail(`dl.locust.lol has ${String(list.assets.length)} files for ${version}, not ${String(wanted)}: attach the ${withoutMac ? 'Linux' : 'Mac and Linux'} builds first`)
  if (withoutMac && list.assets.some((asset) => /mac/i.test(asset.name))) fail(`dl.locust.lol has a Mac file for ${version}: release it with the Mac builds, without --without-mac`)

  const files = []
  for (const asset of list.assets) {
    const name = releaseFileName(asset.name, version)
    const path = join(work, name)
    const answer = await fetch(asset.browser_download_url)
    if (!answer.ok || answer.body === null) fail(`downloading ${asset.name} answered ${String(answer.status)}`)
    const hash = createHash('sha256')
    const source = Readable.fromWeb(answer.body)
    source.on('data', (chunk) => hash.update(chunk))
    await pipeline(source, createWriteStream(path))
    const sha256 = hash.digest('hex')
    const size = statSync(path).size
    if (asset.digest !== `sha256:${sha256}` || asset.size !== size) fail(`${asset.name} from dl.locust.lol is not the file release.json describes (${String(size)} bytes, sha256 ${sha256})`)
    files.push({ name, path, size, sha256 })
    say(`  ${name}: ${String(size)} bytes, the sum release.json gives`)
  }
  const sums = join(work, 'SHA256SUMS.txt')
  writeFileSync(sums, checksumsFile(files))
  const upload = [...files, { name: 'SHA256SUMS.txt', path: sums, size: statSync(sums).size }]

  tagsUp([version])
  let release = releasesUp().get(version)
  if (release === undefined) {
    release = { id: await makeRelease(entry, previous, { draft: true, files }), draft: true }
    say(`  drafted ${version}`)
  } else {
    gh(['api', '-X', 'PATCH', `repos/${REPO}/releases/${release.id}`, '--input', payload('notes', { body: releaseNotes(entry, { previous, files }) }), '--silent'], 'updating the notes')
  }
  const there = filesOn(release.id)
  for (const file of upload) {
    const held = there.get(file.name)
    if (held?.state === 'uploaded' && held.size === file.size) continue
    gh(['release', 'upload', version, file.path, '--clobber', '-R', REPO], `uploading ${file.name}`)
    say(`  uploaded ${file.name}`)
  }
  const back = filesOn(release.id)
  for (const file of upload) {
    const held = back.get(file.name)
    if (held?.state !== 'uploaded' || held.size !== file.size) fail(`${file.name} is not on the release at ${String(file.size)} bytes (${JSON.stringify(held)}); it is left as a draft`)
  }
  if (release.draft) gh(['api', '-X', 'PATCH', `repos/${REPO}/releases/${release.id}`, '-F', 'draft=false', '-f', 'make_latest=true', '--silent'], 'publishing the release')

  // What the world sees: the installer's own address on the Releases page answers, at its size.
  const installer = files.find((file) => file.name.endsWith('-setup.exe'))
  const served = await fetch(`https://github.com/${REPO}/releases/download/${version}/${installer.name}`, { method: 'HEAD', redirect: 'follow' }).catch(() => undefined)
  if (served?.ok !== true || Number(served.headers.get('content-length')) !== installer.size) fail(`${installer.name} does not download from the Releases page (${String(served?.status)})`)
  say(`RELEASED ${version} on ${REPO}: ${String(upload.length)} files, https://github.com/${REPO}/releases/tag/${version}`)
}

try {
  await (history ? releaseHistory() : releaseWithFiles())
} catch (error) {
  console.error(`\nNOT RELEASED: ${error instanceof Refused ? error.message : String(error?.stack ?? error)}`)
  process.exitCode = 1
} finally {
  rmSync(work, { recursive: true, force: true })
}
