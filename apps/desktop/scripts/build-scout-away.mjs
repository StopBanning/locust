// Builds Scout's away look (src/main/scout-away-entry.ts) into one readable file, resources/scout-away.mjs: the file
// Locust's pull request puts in the watched repository as .github/locust/scout-away.mjs (0.829). Not minified, so
// the person can read what runs. `--check` builds it in memory and fails when the file kept here is not what the
// source builds to (scout-away-is-built-from-scouts-own-look.test.ts runs it).
import { readFile, writeFile } from 'node:fs/promises'
import { createRequire } from 'node:module'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
const desktop = join(here, '..')
const out = join(desktop, 'resources', 'scout-away.mjs')
// esbuild comes with vite, which Locust builds with.
const { build } = createRequire(createRequire(import.meta.url).resolve('vite/package.json'))('esbuild')

const HEADER = `// Scout's away look, from Locust (https://github.com/StopBanning/locust). While your computer is off, this looks at
// this repository's pull requests, reviews, checks and issues for you, with this workflow's read-only token, and
// leaves what it found at Locust's relay, sealed with a key only your devices have. Built from
// apps/desktop/src/main/scout-away-entry.ts; Locust's pull request put it here. Delete it, the workflow
// .github/workflows/locust-scout.yml and the LOCUST_SCOUT secret to stop it.
`

export async function builtScoutAway() {
  const result = await build({
    entryPoints: [join(desktop, 'src', 'main', 'scout-away-entry.ts')],
    // The comments name each source file from here, wherever the build is run from.
    absWorkingDir: desktop,
    bundle: true,
    platform: 'node',
    format: 'esm',
    target: 'node20',
    write: false,
    minify: false,
    legalComments: 'none',
    charset: 'utf8',
    banner: { js: HEADER },
    logLevel: 'silent'
  })
  // One line ending everywhere, whatever the checkout's.
  return result.outputFiles[0].text.replace(/\r\n/g, '\n')
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const text = await builtScoutAway()
  if (process.argv.includes('--check')) {
    const kept = (await readFile(out, 'utf8').catch(() => '')).replace(/\r\n/g, '\n')
    if (kept !== text) {
      console.error('resources/scout-away.mjs is not what its source builds to. Run: node scripts/build-scout-away.mjs')
      process.exit(1)
    }
    console.log('resources/scout-away.mjs is up to date.')
  } else {
    await writeFile(out, text)
    console.log(`Wrote resources/scout-away.mjs (${String(Math.round(text.length / 1024))} KB).`)
  }
}
