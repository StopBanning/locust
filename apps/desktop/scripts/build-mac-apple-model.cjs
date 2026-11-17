#!/usr/bin/env node
/*
 * Builds native/apple-model.swift into out/native/locust-apple-model (0.878): the helper that asks Apple's on-device
 * model, behind the "apple" runtime (packages/runtime-adapters/src/apple-model.ts, main/apple-model.ts finds it).
 *
 * Run after electron-vite, which empties out/. Only on a Mac, and for Apple silicon only: Apple Intelligence runs on
 * nothing else. It targets macOS 26, the first with Foundation Models; on an older Mac the helper cannot start, its
 * probe says nothing, and the runtime is simply not offered. A Mac that cannot build it (no Swift, an SDK without the
 * framework) says so and the build goes on without it: Apple Intelligence is an extra, never a reason a build fails.
 */
const { execFileSync } = require('node:child_process')
const { mkdirSync } = require('node:fs')
const { dirname, join } = require('node:path')

if (process.platform !== 'darwin') process.exit(0)

const root = join(__dirname, '..')
const source = join(root, 'native', 'apple-model.swift')
const target = join(root, 'out', 'native', 'locust-apple-model')

try {
  mkdirSync(dirname(target), { recursive: true })
  execFileSync('xcrun', ['swiftc', '-O', '-suppress-warnings', '-target', 'arm64-apple-macos26.0', source, '-o', target], {
    stdio: ['ignore', 'inherit', 'inherit']
  })
  console.log(`apple-model: built ${target}`)
} catch (error) {
  console.warn(`apple-model: not built, so Apple Intelligence is not offered in this build (${error instanceof Error ? error.message : String(error)})`)
}
