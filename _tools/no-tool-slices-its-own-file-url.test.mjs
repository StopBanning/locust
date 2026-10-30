import assert from 'node:assert/strict'
import { readFileSync, readdirSync, statSync } from 'node:fs'
import { join, relative } from 'node:path'
import { test } from 'vitest'
import { localPath } from './local-path.mjs'

// A tool's own file URL with its pathname's first character sliced off is a path only on Windows: on a Mac it made
// `/Users/x` the relative `Users/x`, and 141 tools here wrote it (the first Mac run, 2026-10-30). They read their own
// files through local-path.mjs's localPath now, the one place that may slice a URL's pathname; this keeps it out.
const tools = localPath(new URL('./', import.meta.url))
const sliced = /import\.meta\.url\s*\)\s*\.pathname\s*\.slice\(\s*1\s*\)/
const walk = (dir) => readdirSync(dir).flatMap((name) => {
  const path = join(dir, name)
  if (name === 'node_modules') return []
  return statSync(path).isDirectory() ? walk(path) : /\.(mjs|cjs|js|ts)$/.test(name) ? [path] : []
})

test("no tool slices its own file URL's pathname", () => {
  const offenders = walk(tools).filter((file) => sliced.test(readFileSync(file, 'utf8'))).map((file) => relative(tools, file))
  assert.deepEqual(offenders, [])
})

test('the check would see one', () => {
  // Built from parts, so this file never holds what it looks for.
  assert.ok(sliced.test(['const root = new URL(\'..\', import.meta.url)', 'pathname', 'slice(1)'].join('.')))
  assert.ok(!sliced.test("const root = localPath(new URL('..', import.meta.url))"))
})
