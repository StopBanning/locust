import assert from 'node:assert/strict'
import { test } from 'vitest'
import { changelogEntries, checksumsFile, escapeRawTags, releaseFileName, releaseNotes, shownDate } from './github-release-notes.mjs'

const CHANGELOG = `# Changelog

Intro.

## 0.789.0 - 2026-10-18
<!-- big -->

### New

- **Locust runs on Linux.** A teammate answers on <model>, and \`<locust-ask>\` stays as typed.

## 0.3.0 — 2026-09-02

### Fixed

- **An old fix.**
`

test('every build in the changelog, newest first, with the splash marker left out', () => {
  const entries = changelogEntries(CHANGELOG)
  assert.deepEqual(entries.map((entry) => [entry.version, entry.date]), [['0.789.0', '2026-10-18'], ['0.3.0', '2026-09-02']])
  assert.ok(entries[0].body.startsWith('### New'))
  assert.ok(!entries[0].body.includes('big'))
  assert.equal(entries[1].body, '### Fixed\n\n- **An old fix.**')
})

test('the day reads the same on every computer', () => {
  assert.equal(shownDate('2026-10-18'), 'October 18, 2026')
  assert.equal(shownDate('2026-09-02'), 'September 2, 2026')
})

test('a tag GitHub would drop is shown; code is left as typed', () => {
  assert.equal(escapeRawTags('on <model> and `<locust-ask>`'), 'on &lt;model> and `<locust-ask>`')
  assert.equal(escapeRawTags('```\n<claude>\n```\n<id>'), '```\n<claude>\n```\n&lt;id>')
  assert.equal(escapeRawTags('2 < 3 and a <b'), '2 < 3 and a &lt;b')
})

test('an old build: the day, its entry and the comparison with the one before', () => {
  const [newest, oldest] = changelogEntries(CHANGELOG)
  const notes = releaseNotes(newest, { previous: '0.788.0' })
  assert.ok(notes.startsWith('Released October 18, 2026.\n\n### New'))
  assert.ok(notes.includes('answers on &lt;model>'))
  assert.ok(notes.endsWith('**Full changelog:** [0.788.0...0.789.0](https://github.com/StopBanning/locust/compare/0.788.0...0.789.0)\n'))
  assert.ok(!notes.includes('### Download'))
  assert.ok(releaseNotes(oldest).includes('[CHANGELOG.md](https://github.com/StopBanning/locust/blob/main/CHANGELOG.md)'))
})

test('a build that carries files says which one is for which computer', () => {
  const [newest] = changelogEntries(CHANGELOG)
  const names = ['Locust-Setup.exe', 'Locust-0.789.0-linux-x86_64.AppImage', 'Locust-0.789.0-mac-arm64.dmg', 'Locust-0.789.0-linux-amd64.deb', 'Locust-0.789.0-mac-x64.dmg', 'Locust-0.789.0-linux-x86_64.rpm']
  const files = names.map((name) => ({ name: releaseFileName(name, '0.789.0') }))
  const notes = releaseNotes(newest, { previous: '0.788.0', files })
  const rows = notes.split('\n').filter((line) => line.startsWith('| ') && !line.startsWith('| Your') && !line.startsWith('| ---'))
  assert.deepEqual(rows.map((row) => row.split(' | ')[0].slice(2)), ['Windows 10 or 11', 'Mac with Apple silicon (M1 or later)', 'Mac with an Intel chip', 'Ubuntu, Debian or Mint', 'Fedora or openSUSE', 'Any other Linux'])
  assert.ok(rows[0].includes('[Locust-0.789.0-setup.exe](https://github.com/StopBanning/locust/releases/download/0.789.0/Locust-0.789.0-setup.exe)'))
  assert.ok(notes.indexOf('### Download') < notes.indexOf('**Full changelog:**'))
})

test('the Windows installer is named by its version; the rest keep their names', () => {
  assert.equal(releaseFileName('Locust-Setup.exe', '0.790.0'), 'Locust-0.790.0-setup.exe')
  assert.equal(releaseFileName('Locust-0.790.0-mac-arm64.dmg', '0.790.0'), 'Locust-0.790.0-mac-arm64.dmg')
})

test('the sums file is the one sha256sum -c reads', () => {
  assert.equal(checksumsFile([{ name: 'a.exe', sha256: 'ab12' }, { name: 'b.deb', sha256: 'cd34' }]), 'ab12  a.exe\ncd34  b.deb\n')
})

test('a build with no Mac files says there is none, and which version a Mac stays on', () => {
  const [entry] = changelogEntries(CHANGELOG)
  const files = ['Locust-0.789.0-setup.exe', 'Locust-0.789.0-x86_64.AppImage', 'Locust-0.789.0-amd64.deb', 'Locust-0.789.0-x86_64.rpm'].map((name) => ({ name }))
  const notes = releaseNotes(entry, { previous: '0.788.0', files })
  assert.ok(notes.includes('There is no Mac build of 0.789.0'))
  assert.ok(notes.includes('On a Mac, Locust stays on 0.788.0'))
  const withMac = releaseNotes(entry, { previous: '0.788.0', files: [...files, { name: 'Locust-0.789.0-mac-arm64.dmg' }] })
  assert.ok(!withMac.includes('There is no Mac build'))
})
