import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { isAbsolute } from 'node:path'
import { test } from 'vitest'

// The drive library on a Mac or Linux (the first Mac run, 2026-10-30): its record folders came out relative
// (`/Users/x` sliced to `Users/x`), its Electron was `electron.exe`, and the app's PATH was joined with `;`.
// Windows does exactly what it did; that half is read from the source, since a Windows path cannot be made here.
const source = readFileSync(new URL('./drive-lib.mjs', import.meta.url), 'utf8')

test('its folders are absolute paths on the machine running it', async () => {
  const { APP_DIR, localPath, recordRoot } = await import('./drive-lib.mjs')
  assert.ok(isAbsolute(APP_DIR), APP_DIR)
  assert.ok(isAbsolute(recordRoot('a-record')), recordRoot('a-record'))
  if (process.platform !== 'win32') assert.equal(localPath(new URL('file:///Users/x/a%20b/')), '/Users/x/a b/')
})

test('on Windows, a file URL is read as it always was', () => {
  assert.match(source, /process\.platform === 'win32' \? url\.pathname\.slice\(1\) : fileURLToPath\(url\)/)
})

test("the app's PATH is joined by the system's own separator, npm's folder first on Windows only", () => {
  assert.match(source, /\.join\(delimiter\)/)
  assert.doesNotMatch(source, /\$\{NPM_DIR\};/)
  assert.match(source, /const NPM_DIR = process\.platform === 'win32' \? join\(homedir\(\), 'AppData', 'Roaming', 'npm'\) : undefined/)
})

test("a Mac drive never reads the person's keychain", () => {
  assert.match(source, /const keychain = process\.platform === 'darwin' \? \['--use-mock-keychain'\] : \[\]/)
  assert.match(source, /\.\.\.extraArgs, \.\.\.keychain, `--remote-debugging-port/)
})

test('each system starts its own Electron', () => {
  assert.match(source, /'electron\.exe'/)
  assert.match(source, /'Electron\.app', 'Contents', 'MacOS', 'Electron'/)
})
