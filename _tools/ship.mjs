// The gate a release has to get through.
//
//   node _tools/ship.mjs                 check everything, package, verify
//   node _tools/ship.mjs --check-only    the gates, no packaging
//   node _tools/ship.mjs --marker "Account default" --marker "Show output"
//
// Written because three releases in one night were cut over a RED typecheck
// gate and nobody noticed. `pnpm typecheck` runs two configs -- node and web
// -- and I had been running one of them plus a third that is not the gate at
// all. Eight errors sat there through 0.38.7, 0.38.8 and 0.38.9, six of them
// dead props left by my own change and two in a test file I wrote.
//
// And because "I verified it" twice meant reading `out/`, which is not what
// anybody installs. The installer carries `app.asar`; that is the artefact,
// and it is what this checks.
//
// The markers are the honest part. A changelog bullet claims something is in
// the app; passing that claim's own words here proves the built artefact
// contains them. Read as UTF-8 -- a latin1 read reported a present string as
// missing twice tonight, because an em-dash is three bytes.

import { spawnSync } from 'node:child_process'
import { readFileSync, existsSync, statSync, copyFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

import { retryDecision } from './gate-support.mjs'

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..')
const DESKTOP = join(ROOT, 'apps', 'desktop')

const args = process.argv.slice(2)
const checkOnly = args.includes('--check-only')
const markers = args.flatMap((arg, index) => (arg === '--marker' ? [args[index + 1] ?? ''] : [])).filter(Boolean)

let failed = 0
const ok = (what) => console.log(`  ok    ${what}`)
const bad = (what, detail) => {
  failed += 1
  console.log(`  FAIL  ${what}`)
  if (detail !== undefined) console.log(`        ${detail}`)
}

const failure = (label, result) => {
  // A null status on Windows means the shim never ran, which is NOT a pass.
  const tail = `${result.stdout ?? ''}${result.stderr ?? ''}`
    .split('\n')
    .filter((line) => line.trim().length > 0)
    .slice(-6)
    .join('\n        ')
  bad(label, `exit ${String(result.status)}\n        ${tail}`)
  return false
}

const run = (command, argv, label) => {
  const result = spawnSync(command, argv, { cwd: ROOT, shell: true, encoding: 'utf8' })
  if (result.status === 0) {
    ok(label)
    return true
  }
  return failure(label, result)
}

/*
 * The unit tests, with the gate's one allowance (gate-support.mjs): a red run whose EVERY failure timed out runs
 * those files once more, alone. An assertion, a file that would not load or an unreadable log stays red. The 0.786
 * ship went red three runs in a row, each on a test that passes alone in a second or two (a scan of every source
 * file, a spring simulation): the whole suite's load on this PC, not the change. The default reporter is the one
 * whose log says "timed out".
 */
const unitTests = () => {
  const label = 'unit tests'
  const first = spawnSync('npx', ['vitest', 'run', '--reporter=default'], { cwd: ROOT, shell: true, encoding: 'utf8' })
  if (first.status === 0) return ok(label)
  const decision = retryDecision(`${first.stdout ?? ''}${first.stderr ?? ''}`)
  if (!decision.retry) return failure(label, first)
  const again = spawnSync('npx', ['vitest', 'run', '--reporter=default', ...decision.files], { cwd: ROOT, shell: true, encoding: 'utf8' })
  if (again.status !== 0) return failure(`${label} (timed out, then failed again alone: ${decision.files.join(', ')})`, again)
  ok(`${label} (timed out in the full run, then passed alone: ${decision.files.join(', ')})`)
}

console.log('\nGates')

// 1. The real gate, both configs, exactly as CI would run it. The app's typecheck and tests read the workspace
//    packages' BUILT types (their dist/), so the packages are built first: the 0.796.0 ship read the 0.795 build of
//    runtime-adapters, which had none of the browser's types, and failed a typecheck that passed on a fresh build.
run('pnpm', ['--filter', './packages/*', 'build'], 'build the workspace packages the app reads')
run('pnpm', ['typecheck'], 'pnpm typecheck (node AND web configs)')
unitTests()

// 2. A version nobody bumped ships the previous release's bytes under a new
//    name, which is worse than not shipping.
const version = JSON.parse(readFileSync(join(DESKTOP, 'package.json'), 'utf8')).version
// Releases go to Locust's own downloads since 0.783 (publish-downloads.mjs),
// so a local tag proves nothing. Ask the downloads themselves: a published
// version's feed file is kept there by name. An answer that is neither "there"
// nor "not there" FAILS the gate: "could not ask" is not "not released"
// (L10, the code review: 0.333.0 was rebuilt and passed a day after it shipped).
const DOWNLOADS = 'https://dl.locust.lol/'
const published = await fetch(`${DOWNLOADS}Locust-${version}.yml`, { method: 'HEAD' }).then((answer) => answer.status, (error) => String(error))
if (published === 200) {
  bad(`version ${version} is not already released`, `Locust-${version}.yml is already on ${DOWNLOADS}`)
} else if (published !== 404) {
  bad(`version ${version} is not already released`, `${DOWNLOADS} could not say (${String(published).slice(0, 200)})`)
} else {
  ok(`version ${version} is not published yet`)
}

// 2b. An installer is built from the WORKING TREE, not from a commit.
//
//     On 2026-09-08 two agents were editing this checkout at once, and for
//     about forty-five minutes any release would have packaged the other's
//     half-finished feature -- typechecking, passing its own tests, and
//     completely unintended. The gates above cannot catch that: uncommitted
//     work can be perfectly green and still be nobody's idea of a release.
//
//     So a release is cut from a tree with nothing outstanding in it. That is
//     also what makes `git log` an honest record of what any given installer
//     contains, which it silently was not before.
//
//     Untracked files under `docs/user-session/` are the one exception: drives
//     write their records there constantly, they ship nothing, and requiring a
//     commit for every screenshot would make this gate something to route
//     around rather than obey.

//     WHAT COUNTS AS OUTSTANDING IS NOW WHAT CAN REACH THE INSTALLER.
//
//     `electron-builder.yml` packages `out/**`, `package.json` and
//     `resources/**` and nothing else, so a drive script or a design note
//     cannot be "built into this installer" however dirty it is. The gate
//     said it could, which made it say something false -- and on 2026-09-21,
//     with a second agent mid-task in this tree on the drive tools, that
//     false claim blocked a release fixing a bug the user had reported twice
//     in ten minutes.
//
//     The pressure then is to force it, or to commit the other agent's
//     half-finished work under your own message. Both are worse than the
//     gate. So the rule is narrowed to the paths that actually decide what
//     ships, and EVERY exemption is printed: a gate that quietly forgives
//     things is how the next one gets routed around.
//
//     `_tools/ship.mjs` is deliberately NOT exempt. It is this file, and a
//     half-edited release script decides what a release is.
const dirty = spawnSync('git', ['status', '--porcelain'], { cwd: ROOT, shell: true, encoding: 'utf8' })
if (dirty.status !== 0) {
  // Not a git repository, or git is unavailable. Say so rather than passing a
  // check that was never actually made.
  bad('the working tree is clean', 'git status could not be read, so this was not checked')
} else {
  const changed = (dirty.stdout ?? '')
    .split('\n')
    .map((line) => line.trim())
    .filter((line) => line.length > 0)
  // The path part of a porcelain line: after the two status characters, and
  // through any `old -> new` rename arrow.
  const pathOf = (line) => (line.slice(2).trim().split(' -> ').pop() ?? '').replace(/^"|"$/g, '')
  // Cannot reach `out/`, `package.json` or `resources/`, which is everything
  // the installer contains. A drive harness and a design note are evidence
  // ABOUT a build, never part of one.
  const cannotShip = (line) => {
    const path = pathOf(line)
    if (path.startsWith('docs/')) return true
    if (path.startsWith('_smoke/')) return true
    return /^_tools\/drive-[^/]*\.mjs$/.test(path)
  }
  const exempt = changed.filter(cannotShip)
  const outstanding = changed.filter((line) => !cannotShip(line))
  if (outstanding.length === 0) {
    ok('the working tree is clean')
    // Never silent. Someone reading this output has to be able to see that
    // the tree was not actually clean, and decide for themselves.
    if (exempt.length > 0) {
      console.log(`  note  ${String(exempt.length)} outstanding file(s) the installer cannot contain, left alone:`)
      for (const line of exempt.slice(0, 12)) console.log(`        ${line}`)
      if (exempt.length > 12) console.log(`        ... and ${String(exempt.length - 12)} more`)
    }
  } else {
    bad(
      'the working tree is clean',
      `${String(outstanding.length)} uncommitted change(s) would be built into this installer:\n        ${outstanding.slice(0, 6).join('\n        ')}`
    )
  }
}

// 3. A release with no entry is a release nobody can read.
const changelog = readFileSync(join(ROOT, 'CHANGELOG.md'), 'utf8')
if (changelog.includes(`## ${version} —`) || changelog.includes(`## ${version} -`)) {
  ok(`CHANGELOG has an entry for ${version}`)
} else {
  bad(`CHANGELOG has an entry for ${version}`, 'add one before shipping')
}

if (failed > 0) {
  console.log(`\n${String(failed)} gate(s) failed. Nothing was packaged.\n`)
  process.exit(1)
}

if (checkOnly) {
  console.log('\nGates pass. --check-only, so nothing was packaged.\n')
  process.exit(0)
}

console.log('\nBuilding and packaging')
// npm, staged where the packager will carry it, so a machine with no Node.js
// can still install a coding CLI. It refuses rather than staging a half copy,
// which is the failure that would otherwise reach a person as a broken
// button instead of an honest "you need Node".
if (!run('node', ['_tools/vendor-npm.mjs'], 'stage the npm this app ships')) process.exit(1)
// Memory recall's model and runtime, checked against their pinned hashes.
if (!run('node', ['_tools/vendor-recall.mjs'], 'stage the memory-recall model')) process.exit(1)
if (!run('pnpm', ['build'], 'pnpm build')) process.exit(1)
if (!run('pnpm', ['--filter', '@teammate/desktop', 'package'], 'electron-builder package')) process.exit(1)

console.log('\nWhat actually shipped')

// 4. The artefact, not `out/`. This is the half that was never checked.
const asarPath = join(DESKTOP, 'release', 'win-unpacked', 'resources', 'app.asar')
if (!existsSync(asarPath)) {
  bad('app.asar exists', asarPath)
} else {
  // UTF-8: an em-dash is three bytes and a latin1 read called a present
  // string missing, twice, on 2026-09-07.
  const asar = readFileSync(asarPath).toString('utf8')
  ok(`app.asar is ${String(Math.round(statSync(asarPath).size / 1048576))} MB`)

  const installer = join(DESKTOP, 'release', `Locust-${version}-setup.exe`)
  if (existsSync(installer)) ok(`installer Locust-${version}-setup.exe exists`)
  else bad(`installer Locust-${version}-setup.exe exists`)

  const yml = join(DESKTOP, 'release', 'latest.yml')
  if (existsSync(yml) && readFileSync(yml, 'utf8').includes(`version: ${version}`)) {
    ok(`latest.yml points at ${version}`)
  } else {
    bad(`latest.yml points at ${version}`, 'the update feed would offer the wrong build')
  }

  /*
   * The files BESIDE the archive, not just inside it.
   *
   * `extraResources` are laid down next to app.asar because something has
   * to open them as real files: the window icon, and since 0.63.0 the
   * permission bridge that Claude Code spawns as a process. A missing one
   * fails silently in the installed app -- the icon falls back to Electron's
   * emblem, the bridge never starts and every connector call is refused --
   * and nothing in the asar check would notice, because they are not in it.
   * Read from the builder config so a new entry is checked without anybody
   * remembering to add it here.
   */
  const builderConfig = readFileSync(join(DESKTOP, 'electron-builder.yml'), 'utf8')
  const extra = [...builderConfig.matchAll(/^\s*-\s*from:\s*(\S+)\s+to:\s*(\S+)/gm)]
  /*
   * What the app reads from beside the archive, for the markers (0.615). The
   * starter routines' names live in their own files in routines/, which the
   * app lists at run time, so "Challenge an idea before building it" was
   * reported missing from a build that shipped it. Code and data only:
   * never CHANGELOG.md, which holds every claim and would make each one pass.
   */
  const besideText = []
  const readable = (path) => /\.(json|mjs|js)$/.test(path) && !/CHANGELOG\.md$/i.test(path)
  for (const [, from, to] of extra) {
    const shipped = join(DESKTOP, 'release', 'win-unpacked', 'resources', to)
    const source = join(DESKTOP, from)
    if (!existsSync(shipped)) {
      bad(`beside the asar: ${to}`, 'named in extraResources and not laid down')
    } else if (statSync(shipped).isDirectory()) {
      // A folder (recall/): every file its filter names, each as staged.
      const after = builderConfig.slice(builderConfig.indexOf(`from: ${from}`)).split('\n').slice(1)
      const filter = /^\s+to:/.test(after[0] ?? '') && /^\s+filter:\s*$/.test(after[1] ?? '') ? after.slice(2) : []
      const named = []
      for (const line of filter) {
        const match = /^\s+-\s+(\S+)\s*$/.exec(line)
        if (match === null) break
        named.push(match[1])
      }
      const wrong = named.filter((name) => !existsSync(join(shipped, name)) || !readFileSync(join(shipped, name)).equals(readFileSync(join(source, name))))
      if (named.length === 0) bad(`beside the asar: ${to}/`, 'a folder with no filter naming what it must hold')
      else if (wrong.length > 0) bad(`beside the asar: ${to}/`, `missing or different: ${wrong.join(', ')}`)
      else ok(`beside the asar: ${to}/ (${named.join(', ')})`)
      for (const name of named.filter((entry) => readable(entry) && existsSync(join(shipped, entry)))) {
        besideText.push({ where: `${to}/${name}`, text: readFileSync(join(shipped, name)).toString('utf8') })
      }
    } else if (!existsSync(source) || !readFileSync(shipped).equals(readFileSync(source))) {
      bad(`beside the asar: ${to}`, 'differs from its source')
    } else {
      ok(`beside the asar: ${to}`)
      if (readable(to)) besideText.push({ where: to, text: readFileSync(shipped).toString('utf8') })
    }
  }

  for (const marker of markers) {
    const beside = besideText.find((entry) => entry.text.includes(marker))
    if (asar.includes(marker)) ok(`shipped: ${JSON.stringify(marker)}`)
    else if (beside !== undefined) ok(`shipped: ${JSON.stringify(marker)} (in ${beside.where})`)
    else bad(`shipped: ${JSON.stringify(marker)}`, 'the changelog claims it and the artefact does not have it')
  }
  if (markers.length === 0) {
    console.log('  note  no --marker given; the changelog’s claims were not checked against the artefact')
  }
}

// A copy under a STABLE name, so one link always fetches the newest build.
// locust.lol's download button points at
// `/releases/latest/download/Locust-Setup.exe`. GitHub needs an exact
// filename there, and the versioned installer carries the version in its own,
// so pinning that 404s on the very next release. Uploading this beside it is
// what keeps the button working without anyone remembering to edit a URL --
// and the people it is for do not use GitHub, so a page of assets is not an
// acceptable fallback.
if (failed === 0 && !checkOnly) {
  const installer = join(DESKTOP, 'release', `Locust-${version}-setup.exe`)
  if (existsSync(installer)) {
    copyFileSync(installer, join(DESKTOP, 'release', 'Locust-Setup.exe'))
    ok('stable copy Locust-Setup.exe — upload it beside the versioned one')
  }
}

/*
 * LOCUST IN A BROWSER (0.830): the window this installer carries, as the one zip the relay serves it from
 * (apps/relay/src/window-zip.ts). publish-downloads.mjs puts it beside the installer as window/Locust-<v>-window.zip;
 * a browser opening this version's window is served nothing else.
 */
if (failed === 0 && !checkOnly) {
  const { filesUnder, writeZip } = await import('./window-zip.mjs')
  const files = filesUnder(join(DESKTOP, 'out', 'renderer'))
  const missing = ['desk.html', 'desk-remote.js', 'index.html'].filter((path) => !files.some((file) => file.path === path))
  if (missing.length > 0) bad('the window zip', `the built window has no ${missing.join(', ')}`)
  else {
    const zip = writeZip(files)
    writeFileSync(join(DESKTOP, 'release', `Locust-${version}-window.zip`), zip)
    ok(`window zip Locust-${version}-window.zip: ${String(files.length)} files, ${String(Math.round(zip.length / 1048576))} MB, for Locust in a browser`)
  }
}

/*
 * The two steps that come AFTER the release, spelled out here because this is
 * the last thing anyone reads before doing them.
 *
 * `publish-changelog` is the one that is easy to forget and invisible when
 * forgotten: the app and the site both point at the public CHANGELOG.md, and
 * a release without it leaves both describing the previous build. Uploading
 * the installers one at a time is not fussiness either -- a single
 * `gh release create` carrying both stalled for fifty minutes on 2026-09-17
 * and had to be killed, where one asset per call went through immediately.
 *
 * The block map is what makes an update small: with it, an installed Locust
 * downloads only the blocks that changed (about 2 MB of 117 MB); without it,
 * the whole installer, every time. It went missing from this list for months.
 *
 * A PRERELEASE, THEN ONE A DAY PROMOTED (0.307). The beta handover: testers
 * get at most one new build a day. Every build goes out as a prerelease --
 * taken only by a Locust whose "every build" switch is on -- and
 * promote-release.mjs makes one a day the release GitHub calls latest, which
 * the tester lane and the site both read.
 *
 * DRAFT FIRST (0.304). An installed Locust reads latest.yml from the newest
 * PUBLISHED release. Published with only latest.yml on it, the release sends
 * every Locust that checks in that minute after an installer that is not
 * there yet, or one without its block map -- a full download. A draft is
 * invisible to the channel until every file is on it.
 */
/*
 * ONE COMMAND TO PUBLISH, AND IT CHECKS (2026-09-24). The steps below used
 * to be typed as a chain of gh calls, each piped to `tail`, and a stalled
 * installer upload that was stopped let the chain publish 0.325.0 with no
 * installer: every checking Locust got a 404. publish-release.mjs uploads one
 * file per call with its own exit code, reads the draft back, and publishes
 * only when every file is on it whole -- then checks the installer answers.
 */
console.log(
  failed === 0
    ? `\nReady to release ${version}.\n\n`
      + `  node _tools/publish-release.mjs ${version} --notes "..."   # uploads, reads back, publishes, checks\n`
      + `  node _tools/publish-changelog.mjs      # the app and the site both read this\n`
      + `  node _smoke/update-smoke.mjs\n`
      + `  Never chain gh through a pipe: a pipe reports the last command's exit, not gh's.\n`
    : `\n${String(failed)} check(s) failed AFTER packaging. Do not publish this.\n`
)
process.exit(failed === 0 ? 0 : 1)
