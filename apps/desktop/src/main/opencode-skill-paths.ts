import { readFile } from 'node:fs/promises'
import { homedir } from 'node:os'
import { join, resolve } from 'node:path'
import { parse, type ParseError } from 'jsonc-parser'
import { skillProjectFolders } from './skill-discovery.js'

/**
 * The person's own OpenCode 1 skill paths, for a run whose config adds Locust's folder to them. Read configuration,
 * never auth, and never ask the CLI to load plugins just to discover a path.
 *
 * MEASURED (OpenCode 1.18.27, `debug config` and `debug skill` in an isolated home): `skills.paths` is not added up
 * across files. The last config that names it wins: the global folder's files, then `OPENCODE_CONFIG`, then each
 * folder's `opencode.json` and `opencode.jsonc` from the repository's root down to the work, then the `.opencode`
 * folders, then `OPENCODE_CONFIG_CONTENT`. That last one is the run's, so on its own it dropped the person's paths. A
 * relative path, `~/` and `{env:...}` resolve the same from the content as from a file, so the strings go as written.
 */
export async function openCodeSkillPaths(options: {
  readonly cwd: string
  readonly home?: string
  readonly env?: NodeJS.ProcessEnv
}): Promise<readonly string[]> {
  const home = options.home ?? homedir()
  const env = options.env ?? process.env
  const global = join(env.XDG_CONFIG_HOME || join(home, '.config'), 'opencode')
  const folders = await skillProjectFolders(options.cwd)
  const projectDisabled = /^(?:1|true)$/i.test(env.OPENCODE_DISABLE_PROJECT_CONFIG ?? '')
  // The global folder's files are read once, first: global then project gives the project's paths (measured).
  const directories = [...new Set([
    ...(projectDisabled ? [] : folders.map((folder) => join(folder, '.opencode'))),
    join(home, '.opencode'),
    ...(env.OPENCODE_CONFIG_DIR ? [env.OPENCODE_CONFIG_DIR] : [])
  ])]
  const files = [
    ...['config.json', 'opencode.json', 'opencode.jsonc'].map((name) => join(global, name)),
    ...(env.OPENCODE_CONFIG ? [resolve(options.cwd, env.OPENCODE_CONFIG)] : []),
    ...(projectDisabled ? [] : [...folders].reverse().flatMap((folder) => ['opencode.json', 'opencode.jsonc'].map((name) => join(folder, name)))),
    // Config directories are loaded after project files, CWD outward, as in OpenCode 1's ConfigPaths.directories.
    ...directories.flatMap((folder) => ['opencode.json', 'opencode.jsonc'].map((name) => join(folder, name)))
  ]
  let paths: readonly string[] = []
  const add = (text: string, from: string): void => {
    const errors: ParseError[] = []
    const config: unknown = parse(text.replace(/^﻿/, ''), errors, { allowTrailingComma: true, allowEmptyContent: true })
    if (errors.length > 0) throw new Error(`OpenCode’s settings in ${from} aren’t valid JSON, so nothing was started. Fix them, then send again.`)
    if (typeof config !== 'object' || config === null || !('skills' in config)) return
    const skills = config.skills
    if (typeof skills !== 'object' || skills === null || !('paths' in skills) || !Array.isArray(skills.paths)) return
    // A later config's list takes the earlier one's place. Only the run's in-memory config is changed.
    paths = [...new Set(skills.paths.filter((path): path is string => typeof path === 'string' && path.length > 0))]
  }
  for (const file of files) {
    const text = await readFile(file, 'utf8').catch((cause: NodeJS.ErrnoException) => {
      if (cause.code === 'ENOENT' || cause.code === 'ENOTDIR') return undefined
      throw new Error(`OpenCode’s settings file ${file} couldn’t be read, so nothing was started.`)
    })
    if (text !== undefined) add(text, file)
  }
  if (env.OPENCODE_CONFIG_CONTENT) add(env.OPENCODE_CONFIG_CONTENT, 'OPENCODE_CONFIG_CONTENT')
  return paths
}
