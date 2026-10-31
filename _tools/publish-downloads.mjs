// Publish a built release to Locust's own downloads (dl.locust.lol, a Cloudflare R2 bucket), and refuse to until
// every file is really there.
//
//   node _tools/publish-downloads.mjs <version>                every build: installer, block map, then beta.yml
//   node _tools/publish-downloads.mjs <version> --mac <dir>    the Mac disk images for <version>, from the CI run
//   node _tools/publish-downloads.mjs <version> --linux <dir>  the Linux packages and their feed, from the CI run
//   node _tools/publish-downloads.mjs <version> --promote      the tester lane: latest.yml, Locust-Setup.exe, the
//                                                              Mac release list, latest-linux.yml, and
//                                                              release.json + CHANGELOG.md
//                                                              for locust.lol, once a day
//
// WHY (0.783). GitHub suspended the account that held Locust's releases on 2026-10-16 and every installed copy's
// update check failed with it (src/shared/locust-home.ts). The rules publish-release.mjs learned on GitHub hold
// here too: every upload is its own command with its own exit code, each file is read back over HTTPS at the size
// on disk, and the feed file goes LAST, so no copy is ever pointed at an installer that is not there yet.

import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, mkdtempSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const BUCKET = 'locust-downloads'
const DOWNLOADS = 'https://dl.locust.lol/'
const RELEASE = new URL('../apps/desktop/release/', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')
const CHANGELOG = new URL('../CHANGELOG.md', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1')
const version = process.argv[2]
const flag = (name) => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] ?? '' : undefined)
const promote = process.argv.includes('--promote')
const macDir = flag('--mac')
const linuxDir = flag('--linux')
if (version === undefined || !/^\d+\.\d+\.\d+$/.test(version) || macDir === '' || linuxDir === '') {
  console.error('usage: node _tools/publish-downloads.mjs <version> [--mac <dir> | --linux <dir> | --promote]')
  process.exit(2)
}

const say = (line) => console.log(line)
const fail = (line) => {
  console.error(`\nNOT PUBLISHED: ${line}\nThe feed files were not changed, so no installed Locust sees anything new.`)
  process.exit(1)
}
const TYPES = { '.zip': 'application/zip', '.exe': 'application/vnd.microsoft.portable-executable', '.dmg': 'application/x-apple-diskimage', '.yml': 'text/yaml; charset=utf-8', '.json': 'application/json', '.blockmap': 'application/octet-stream', '.md': 'text/plain; charset=utf-8', '.AppImage': 'application/vnd.appimage', '.deb': 'application/vnd.debian.binary-package', '.rpm': 'application/x-rpm' }
const typeOf = (name) => TYPES[Object.keys(TYPES).find((ending) => name.endsWith(ending)) ?? '.blockmap']

// No shell: wrangler is run by node from npx's own script, with fixed arguments.
function put(key, file, { fresh = false } = {}) {
  for (let attempt = 1; ; attempt += 1) {
    const run = spawnSync(process.execPath, [npxCli(), '--yes', 'wrangler@4', 'r2', 'object', 'put', `${BUCKET}/${key}`, '--file', file, '--remote', '--content-type', typeOf(key), ...(fresh ? ['--cache-control', 'no-cache'] : [])], { encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 })
    if (run.status === 0) return
    const said = (run.stderr || run.stdout || '').trim()
    // Cloudflare's API answered a 134 MB upload with its "502: Bad gateway" page twice (0.793.0 and 0.794.0, both in
    // the Linux attach's promote): a gateway or connection failure is tried twice more; anything else stops here.
    if (attempt < 3 && /\b50[234]\b|Bad Gateway|Service Unavailable|Gateway Time-?out|ECONNRESET|ETIMEDOUT/i.test(said)) {
      say(`  ${key}: Cloudflare's gateway failed (try ${String(attempt)} of 3); again in ${String(attempt * 15)} s`)
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, attempt * 15_000)
      continue
    }
    fail(`uploading ${key} failed (exit ${String(run.status)}): ${said.slice(-400)}`)
  }
}
// Where node keeps npm: beside node.exe on Windows, and in ../lib/node_modules beside bin/node on a Mac or Linux
// (Homebrew's and nodejs.org's layout). The Mac publishes since the PC's retirement (2026-10-30).
const npxCliPlaces = (execPath) => [
  join(execPath, '..', 'node_modules', 'npm', 'bin', 'npx-cli.js'),
  join(execPath, '..', '..', 'lib', 'node_modules', 'npm', 'bin', 'npx-cli.js')
]
function npxCli() {
  const places = npxCliPlaces(process.execPath)
  const cli = places.find((place) => existsSync(place))
  if (cli === undefined) fail(`npx is not where node keeps it (${places.join(' or ')})`)
  return cli
}
// What the world sees, not what the upload said: a HEAD over HTTPS, at the size on disk.
async function served(key, size) {
  for (let attempt = 0; attempt < 5; attempt += 1) {
    // identity: Cloudflare compresses text files for a client that accepts gzip, and then sends no length.
    const answer = await fetch(`${DOWNLOADS}${key}?check=${String(Date.now())}`, { method: 'HEAD', headers: { 'Accept-Encoding': 'identity' } }).catch(() => undefined)
    if (answer?.ok === true && Number(answer.headers.get('content-length')) === size) return
    await new Promise((resolve) => setTimeout(resolve, 3000))
  }
  fail(`${DOWNLOADS}${key} does not answer at ${String(size)} bytes`)
}
async function upload(key, file, options) {
  const size = statSync(file).size
  put(key, file, options)
  await served(key, size)
  say(`  ${key}  ${String(size)} bytes, served`)
}
const scratch = () => mkdtempSync(join(tmpdir(), 'locust-publish-downloads-'))
async function fetchText(key) {
  const answer = await fetch(`${DOWNLOADS}${key}?check=${String(Date.now())}`).catch(() => undefined)
  return answer?.ok === true ? answer.text() : undefined
}

if (macDir !== undefined) {
  // The two disk images, then a small file naming them with their sizes and hashes, which --promote reads.
  const images = readdirSync(macDir).filter((name) => name === `Locust-${version}-mac-arm64.dmg` || name === `Locust-${version}-mac-x64.dmg`)
  if (images.length !== 2) fail(`${macDir} must hold Locust-${version}-mac-arm64.dmg and -mac-x64.dmg; it holds ${images.join(', ') || 'neither'}`)
  const assets = []
  for (const name of images.sort()) {
    const file = join(macDir, name)
    await upload(name, file)
    assets.push({ name, browser_download_url: `${DOWNLOADS}${name}`, size: statSync(file).size, digest: `sha256:${createHash('sha256').update(readFileSync(file)).digest('hex')}` })
  }
  const sidecar = join(scratch(), `Locust-${version}-mac.json`)
  writeFileSync(sidecar, `${JSON.stringify({ tag_name: version, draft: false, prerelease: false, assets }, null, 2)}\n`)
  await upload(`Locust-${version}-mac.json`, sidecar)
  say(`${version}: the Mac images are up. Macs see them when ${version} is promoted.`)
} else if (linuxDir !== undefined) {
  // The three Linux packages (0.789), a small file naming them for the site, then their feed: the AppImage's copies
  // read it to update themselves, a .deb's or .rpm's to say a new version is out. `beta-linux.yml` is every build's
  // lane at once; --promote makes it the tester lane's as `latest-linux.yml`.
  const expected = [`Locust-${version}-linux-x86_64.AppImage`, `Locust-${version}-linux-amd64.deb`, `Locust-${version}-linux-x86_64.rpm`]
  const missing = expected.filter((name) => !existsSync(join(linuxDir, name)))
  if (missing.length > 0) fail(`${linuxDir} is missing ${missing.join(', ')}; it holds ${readdirSync(linuxDir).join(', ') || 'nothing'}`)
  const feedFile = join(linuxDir, 'latest-linux.yml')
  if (!existsSync(feedFile)) fail(`${linuxDir} has no latest-linux.yml`)
  const feed = readFileSync(feedFile, 'utf8')
  if (!feed.includes(`version: ${version}`)) fail(`latest-linux.yml is not for ${version}:\n${feed}`)
  const sha512 = createHash('sha512').update(readFileSync(join(linuxDir, expected[0]))).digest('base64')
  if (!feed.includes(`sha512: ${sha512}`)) fail("latest-linux.yml's sha512 is not the AppImage on disk")
  const assets = []
  for (const name of expected) {
    const file = join(linuxDir, name)
    await upload(name, file)
    assets.push({ name, browser_download_url: `${DOWNLOADS}${name}`, size: statSync(file).size, digest: `sha256:${createHash('sha256').update(readFileSync(file)).digest('hex')}` })
  }
  const sidecar = join(scratch(), `Locust-${version}-linux.json`)
  writeFileSync(sidecar, `${JSON.stringify({ tag_name: version, draft: false, prerelease: false, assets }, null, 2)}\n`)
  await upload(`Locust-${version}-linux.json`, sidecar)
  await upload(`Locust-${version}-linux.yml`, feedFile)
  await upload('beta-linux.yml', feedFile, { fresh: true })
  say(`${version}: the Linux packages are up. Linux copies on every build see them now, testers once ${version} is promoted.`)
} else if (!promote) {
  const installer = `Locust-${version}-setup.exe`
  for (const name of [installer, `${installer}.blockmap`, 'latest.yml']) {
    if (!existsSync(join(RELEASE, name))) fail(`${name} is not in ${RELEASE} -- build it with _tools/ship.mjs first`)
  }
  // latest.yml must name this version's installer, with the hash of the file that is about to go up.
  const feed = readFileSync(join(RELEASE, 'latest.yml'), 'utf8')
  if (!feed.includes(`version: ${version}`) || !feed.includes(`path: ${installer}`)) fail(`latest.yml is not for ${version}:\n${feed}`)
  const sha512 = createHash('sha512').update(readFileSync(join(RELEASE, installer))).digest('base64')
  if (!feed.includes(`sha512: ${sha512}`)) fail("latest.yml's sha512 is not the installer on disk")
  await upload(`${installer}.blockmap`, join(RELEASE, `${installer}.blockmap`))
  await upload(installer, join(RELEASE, installer))
  // Its window, for Locust in a browser (0.830): the relay serves a version's window from this zip only. Up before
  // the feed, so no Locust is updated to a version whose window a browser cannot open.
  const windowZip = `Locust-${version}-window.zip`
  if (!existsSync(join(RELEASE, windowZip))) fail(`${windowZip} is not in ${RELEASE} -- build it with _tools/ship.mjs first`)
  await upload(`window/${windowZip}`, join(RELEASE, windowZip))
  // This version's feed file, kept by name so --promote can hand it to the tester lane later.
  await upload(`Locust-${version}.yml`, join(RELEASE, 'latest.yml'))
  await upload('beta.yml', join(RELEASE, 'latest.yml'), { fresh: true })
  say(`${version} is out on every build. Promote it for testers with --promote.`)
} else {
  const installer = `Locust-${version}-setup.exe`
  const feed = await fetchText(`Locust-${version}.yml`)
  if (feed === undefined || !feed.includes(`version: ${version}`)) fail(`Locust-${version}.yml is not on ${DOWNLOADS}: publish ${version} first`)
  const local = join(RELEASE, installer)
  if (!existsSync(local)) fail(`${installer} is not in ${RELEASE}; the site's Locust-Setup.exe is uploaded from it`)
  const dir = scratch()
  writeFileSync(join(dir, 'latest.yml'), feed)
  // A promote run again -- the Mac and Linux attaches each promote after the first -- finds this version's installer
  // already up: the first promote put it there before latest.yml, which goes last. Sending the same 134 MB again only
  // gave Cloudflare's API the chance to fail it (above), so a copy this version already holds is left as it is.
  const tester = await fetchText('latest.yml')
  const holding = await fetch(`${DOWNLOADS}Locust-Setup.exe?check=${String(Date.now())}`, { method: 'HEAD', headers: { 'Accept-Encoding': 'identity' } }).catch(() => undefined)
  if (tester !== undefined && tester.includes(`version: ${version}`) && holding?.ok === true && Number(holding.headers.get('content-length')) === statSync(local).size) {
    say(`  Locust-Setup.exe  already ${version}'s, left as it is`)
  } else {
    await upload('Locust-Setup.exe', local, { fresh: true })
  }
  // The Mac release list, newest first, in the shape GitHub's releases API answered in (src/main/mac-release.ts).
  const mac = await fetchText(`Locust-${version}-mac.json`)
  // The Linux packages and their feed, when the CI run has put them up (0.789).
  const linux = await fetchText(`Locust-${version}-linux.json`)
  const linuxFeed = await fetchText(`Locust-${version}-linux.yml`)
  // What locust.lol's Download button reads: the tester lane's release in the shape of GitHub's /releases/latest,
  // which the site read until the suspension -- the version, and the size and SHA-256 of Locust-Setup.exe.
  const setup = readFileSync(local)
  const release = {
    tag_name: version,
    draft: false,
    prerelease: false,
    assets: [
      { name: 'Locust-Setup.exe', browser_download_url: `${DOWNLOADS}Locust-Setup.exe`, size: setup.length, digest: `sha256:${createHash('sha256').update(setup).digest('hex')}` },
      ...(mac === undefined ? [] : JSON.parse(mac).assets),
      ...(linux === undefined ? [] : JSON.parse(linux).assets),
    ],
  }
  writeFileSync(join(dir, 'release.json'), `${JSON.stringify(release, null, 2)}
`)
  if (mac === undefined) {
    say(`  no Mac images for ${version} yet: the Mac list is left as it is`)
  } else {
    const before = JSON.parse((await fetchText('mac-releases.json')) ?? '[]')
    const list = [JSON.parse(mac), ...before.filter((entry) => entry.tag_name !== version)].slice(0, 10)
    writeFileSync(join(dir, 'mac-releases.json'), `${JSON.stringify(list, null, 2)}\n`)
    await upload('mac-releases.json', join(dir, 'mac-releases.json'), { fresh: true })
  }
  if (linuxFeed === undefined || !linuxFeed.includes(`version: ${version}`)) {
    say(`  no Linux packages for ${version} yet: the Linux feed is left as it is`)
  } else {
    writeFileSync(join(dir, 'latest-linux.yml'), linuxFeed)
    await upload('latest-linux.yml', join(dir, 'latest-linux.yml'), { fresh: true })
  }
  await upload('latest.yml', join(dir, 'latest.yml'), { fresh: true })
  await upload('release.json', join(dir, 'release.json'), { fresh: true })
  await upload('CHANGELOG.md', CHANGELOG, { fresh: true })
  say(`${version} is the tester lane's now.`)
}
