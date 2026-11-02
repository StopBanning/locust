#!/usr/bin/env node
/*
 * Builds native/mac-glass.mm into out/native/mac-glass.node, for both Mac chips (main/mac-glass.ts loads it).
 *
 * Run after electron-vite, which empties out/. Only on a Mac: elsewhere there is nothing to build and the Look
 * setting is not offered. It needs only clang and the Node-API header that ships with Node, so no node-gyp: Node-API
 * is the same in Electron's Node as in this one. A Mac that cannot build it says so and the build goes on without
 * Liquid glass, except on CI (`CI` set), where a Mac build without it fails rather than ships without it.
 */
const { execFileSync } = require('node:child_process')
const { existsSync, mkdirSync } = require('node:fs')
const { dirname, join } = require('node:path')

if (process.platform !== 'darwin') process.exit(0)

const root = join(__dirname, '..')
const source = join(root, 'native', 'mac-glass.mm')
const target = join(root, 'out', 'native', 'mac-glass.node')
const headers = join(dirname(process.execPath), '..', 'include', 'node')

try {
  if (!existsSync(join(headers, 'node_api.h'))) throw new Error(`no node_api.h under ${headers}`)
  mkdirSync(dirname(target), { recursive: true })
  execFileSync(
    'clang++',
    [
      '-std=c++17', '-ObjC++', '-fobjc-arc', '-O2', '-bundle', '-undefined', 'dynamic_lookup',
      '-arch', 'arm64', '-arch', 'x86_64', '-mmacosx-version-min=11.0',
      `-I${headers}`, '-DNODE_GYP_MODULE_NAME=mac_glass', '-framework', 'AppKit',
      source, '-o', target
    ],
    { stdio: ['ignore', 'inherit', 'inherit'] }
  )
  console.log(`mac-glass: built ${target}`)
} catch (error) {
  console.error(`mac-glass: not built, so Liquid glass is not offered in this build (${error instanceof Error ? error.message : String(error)})`)
  if (process.env.CI) process.exit(1)
}
