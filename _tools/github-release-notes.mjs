// What a release on StopBanning/locust's Releases page says (0.790): its entry in CHANGELOG.md, the day it came
// out, where to download it when it carries files, and what changed since the one before. No I/O here, so
// github-release-notes.test.mjs can read every word; github-release.mjs does the publishing.

export const REPO = 'StopBanning/locust'
const HOME = `https://github.com/${REPO}`
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']

/** Every build in CHANGELOG.md, newest first as the file has them: `## 0.789.0 - 2026-10-18` and what follows it. */
export function changelogEntries(text) {
  const entries = []
  const heading = /^## (\d+\.\d+\.\d+)\s+[-—]\s+(\d{4}-\d{2}-\d{2})\s*$/gm
  const found = [...text.matchAll(heading)]
  found.forEach((match, index) => {
    const end = index + 1 < found.length ? found[index + 1].index : text.length
    const body = text.slice(match.index + match[0].length, end).replace(/<!--[\s\S]*?-->/g, '').trim()
    entries.push({ version: match[1], date: match[2], body })
  })
  return entries
}

/** `2026-10-18` as a person reads it, the same on every computer: October 18, 2026. */
export function shownDate(date) {
  const [year, month, day] = date.split('-').map(Number)
  return `${MONTHS[month - 1]} ${String(day)}, ${String(year)}`
}

/**
 * GitHub drops a tag it does not know, so `<model>` written in an entry would vanish from the page. Outside code, a
 * `<` that opens one is written as `&lt;`; inside backticks it is left alone, since there GitHub shows it as typed.
 */
export function escapeRawTags(markdown) {
  return markdown
    // A backtick is \x60 here: written as itself, harness-backticks.test.ts reads it as a template literal opening.
    .split(/(\x60\x60\x60[\s\S]*?\x60\x60\x60|\x60[^\x60\n]*\x60)/)
    .map((part, index) => (index % 2 === 1 ? part : part.replace(/<(?=[A-Za-z/])/g, '&lt;')))
    .join('')
}

/** The name a file has on the Releases page: the Windows installer carries its version there, like every other file. */
export function releaseFileName(name, version) {
  return name === 'Locust-Setup.exe' ? `Locust-${version}-setup.exe` : name
}

/** Which computer each file is for, in the order the download table lists them. */
const SYSTEMS = [
  [/-setup\.exe$/, 'Windows 10 or 11'],
  [/-mac-arm64\.dmg$/, 'Mac with Apple silicon (M1 or later)'],
  [/-mac-x64\.dmg$/, 'Mac with an Intel chip'],
  [/\.deb$/, 'Ubuntu, Debian or Mint'],
  [/\.rpm$/, 'Fedora or openSUSE'],
  [/\.AppImage$/, 'Any other Linux']
]

/** `sha256sum`'s own format, so `sha256sum -c SHA256SUMS.txt` checks a download where it was saved. */
export function checksumsFile(files) {
  return files.map((file) => `${file.sha256}  ${file.name}\n`).join('')
}

/**
 * A release's page. `files` are the installers it carries (none for a build whose installers are no longer
 * offered), `previous` the version before it, for GitHub's comparison of the two.
 */
export function releaseNotes(entry, { previous, files = [] } = {}) {
  const parts = [`Released ${shownDate(entry.date)}.`, escapeRawTags(entry.body)]
  if (files.length > 0) {
    const rows = SYSTEMS.flatMap(([pattern, system]) => files
      .filter((file) => pattern.test(file.name))
      .map((file) => `| ${system} | [${file.name}](${HOME}/releases/download/${entry.version}/${file.name}) |`))
    parts.push([
      '### Download',
      '',
      '| Your computer | File |',
      '| --- | --- |',
      ...rows,
      '',
      'Or download it at [locust.lol](https://locust.lol). After that, Locust tells you when a new version is out, and on Windows, Mac and the AppImage it installs it for you. The Windows and Mac builds are not signed by a publisher yet, so each asks once before it opens: [what to press](' + HOME + '#readme).',
      '',
      'The SHA-256 sum of every file is in `SHA256SUMS.txt`.'
    ].join('\n'))
    // A build GitHub had no Mac machine for (0.835: its macOS runners were out of capacity for hours) is
    // released for Windows and Linux alone, and says so rather than leave a Mac reader looking for a file.
    if (!files.some((file) => /-mac-(arm64|x64)\.dmg$/.test(file.name))) {
      parts.push(`There is no Mac build of ${entry.version}: GitHub had no Mac build machine free for it. On a Mac, Locust stays on ${previous ?? 'the version before'} and offers the next version that has one.`)
    }
  }
  parts.push(previous === undefined
    ? `**Full changelog:** [CHANGELOG.md](${HOME}/blob/main/CHANGELOG.md)`
    : `**Full changelog:** [${previous}...${entry.version}](${HOME}/compare/${previous}...${entry.version})`)
  return parts.join('\n\n') + '\n'
}
