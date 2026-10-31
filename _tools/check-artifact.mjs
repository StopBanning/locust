// What a release's Windows build holds, checked: the archive, the installer, its feed, every file laid down beside
// the archive, and each changelog claim (--marker) in the shipped bytes. ship.mjs runs it after packaging on the PC;
// since the PC's retirement (2026-10-30) CI runs it on its Windows build too, so the Mac can publish what CI made.
//
//   node _tools/check-artifact.mjs [--marker "<a string the release adds>"]...
//
// Markers also come from LOCUST_MARKERS, one a line (how CI is handed them). Exits 0 and says WINDOWS ARTIFACT
// PASSED when every check passes.
import { existsSync, readFileSync, statSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

/** Every check, said through `ok` and `bad`; the caller counts what `bad` was told. */
export function checkArtifact({ desktop: DESKTOP, version, markers, ok, bad }) {
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
}

if (process.argv[1] !== undefined && fileURLToPath(import.meta.url) === process.argv[1]) {
  const args = process.argv.slice(2)
  const markers = [
    ...args.flatMap((arg, index) => (arg === '--marker' ? [args[index + 1] ?? ''] : [])),
    ...(process.env.LOCUST_MARKERS ?? '').split(/\r?\n/)
  ].map((marker) => marker.trim()).filter(Boolean)
  const desktop = join(dirname(fileURLToPath(import.meta.url)), '..', 'apps', 'desktop')
  const version = JSON.parse(readFileSync(join(desktop, 'package.json'), 'utf8')).version
  let failed = 0
  checkArtifact({
    desktop,
    version,
    markers,
    ok: (what) => console.log(`  ok    ${what}`),
    bad: (what, detail) => {
      failed += 1
      console.log(`  FAIL  ${what}`)
      if (detail !== undefined) console.log(`        ${detail}`)
    }
  })
  console.log(failed === 0 ? `WINDOWS ARTIFACT PASSED: ${version}` : `\n${String(failed)} check(s) failed on the artifact of ${version}.`)
  process.exit(failed === 0 ? 0 : 1)
}
