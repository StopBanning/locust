import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'
import { createOpenCodeRunCommand } from '@teammate/runtime-adapters'
import { openCodeSkillPaths } from './opencode-skill-paths.js'

/*
 * AN OPENCODE RUN KEEPS THE PERSON'S SKILL PATHS (0.816). The run's own config named only Locust's folder, and OpenCode
 * keeps the last config's `skills.paths` (measured, 1.18.27), so the person's paths went. The run now carries the paths
 * OpenCode itself would have used, in its own order, then Locust's folder.
 */
const made: string[] = []
afterEach(async () => { for (const folder of made.splice(0)) await rm(folder, { recursive: true, force: true }) })
const fixture = async () => {
  const root = await mkdtemp(join(tmpdir(), 'locust-skill-paths-'))
  made.push(root)
  const home = join(root, 'home')
  const project = join(root, 'project')
  const cwd = join(project, 'child')
  await mkdir(cwd, { recursive: true })
  await writeFile(join(project, '.git'), 'A worktree marker: never read.')
  const put = async (file: string, content: string) => { await mkdir(dirname(file), { recursive: true }); await writeFile(file, content) }
  return { root, home, project, cwd, put }
}

describe('a run keeps the person’s configured OpenCode skill paths', () => {
  it('keeps the last config that names them, in OpenCode’s own order, without changing any file', async () => {
    const { root, home, project, cwd, put } = await fixture()
    const global = join(home, '.config', 'opencode', 'opencode.jsonc')
    const explicit = join(root, 'custom.jsonc')
    const local = join(project, 'opencode.json')
    const text = '{ // do not mistake a URL for a comment\n "skills": {"paths": ["/global", "https://example.com//folder", "/global"],},\n "provider": {"example": {"key": "{file:do-not-read-auth}"}},\n}\n'
    await put(global, text)
    const read = (env: NodeJS.ProcessEnv = {}) => openCodeSkillPaths({ cwd, home, env })
    expect(await read()).toEqual(['/global', 'https://example.com//folder'])
    await put(explicit, '{/* custom */"skills":{"paths":["/custom",],},}')
    expect(await read({ OPENCODE_CONFIG: explicit })).toEqual(['/custom'])
    await put(local, '{"skills":{"paths":["/project"]}}')
    expect(await read({ OPENCODE_CONFIG: explicit })).toEqual(['/project'])
    await put(join(cwd, 'opencode.json'), '{"skills":{"paths":["/child"]}}')
    expect(await read()).toEqual(['/child'])
    // In one folder, .jsonc comes after .json; a config that names no paths leaves the last list alone.
    await put(join(cwd, 'opencode.jsonc'), '{"skills":{"paths":["/child-jsonc"]}}')
    await put(join(cwd, '.opencode', 'opencode.json'), '{"skills":{}}')
    expect(await read()).toEqual(['/child-jsonc'])
    await put(join(cwd, '.opencode', 'opencode.json'), '{"skills":{"paths":["/dot-opencode"]}}')
    expect(await read()).toEqual(['/dot-opencode'])
    const paths = await read({ OPENCODE_CONFIG_CONTENT: '{"skills":{"paths":["/inline"]}}' })
    expect(paths).toEqual(['/inline'])
    const command = createOpenCodeRunCommand({ commandName: 'opencode', discoveredPath: '/bin/opencode', executablePath: '/bin/opencode', prefixArgs: [], kind: 'native' }, { workspacePath: cwd, sandbox: 'read-only', prompt: 'go', skillsDir: '/run/skills', skillPaths: paths })
    expect(JSON.parse(command.env!.OPENCODE_CONFIG_CONTENT!).skills.paths).toEqual(['/inline', '/run/skills'])
    expect(await readFile(global, 'utf8')).toBe(text)
    expect(await readFile(local, 'utf8')).toBe('{"skills":{"paths":["/project"]}}')
  })

  it('reads the global folder once and first, honors XDG and the config directories, and stops at the worktree', async () => {
    const { root, home, project, cwd, put } = await fixture()
    const xdg = join(root, 'xdg')
    const override = join(root, 'override')
    const read = (env: NodeJS.ProcessEnv = {}) => openCodeSkillPaths({ cwd, home, env: { XDG_CONFIG_HOME: xdg, ...env } })
    await put(join(root, 'opencode.json'), '{"skills":{"paths":["/outside-worktree"]}}')
    expect(await read()).toEqual([])
    await put(join(xdg, 'opencode', 'opencode.json'), '{"skills":{"paths":["/global"]}}')
    expect(await read()).toEqual(['/global'])
    // Global then project gives the project's, as OpenCode does: the global folder is not read again after it.
    await put(join(project, 'opencode.json'), '{"skills":{"paths":["/project"]}}')
    expect(await read()).toEqual(['/project'])
    await put(join(home, '.opencode', 'opencode.json'), '{"skills":{"paths":["/home-opencode"]}}')
    expect(await read()).toEqual(['/home-opencode'])
    await put(join(override, 'opencode.jsonc'), '{"skills":{"paths":["/override"]}}')
    expect(await read({ OPENCODE_CONFIG_DIR: override })).toEqual(['/override'])
  })

  it('does not load disabled project config, and names a file it cannot read', async () => {
    const { home, project, cwd, put } = await fixture()
    await put(join(project, 'opencode.jsonc'), '{broken')
    expect(await openCodeSkillPaths({ cwd, home, env: { OPENCODE_DISABLE_PROJECT_CONFIG: '1' } })).toEqual([])
    expect(await openCodeSkillPaths({ cwd, home, env: { OPENCODE_DISABLE_PROJECT_CONFIG: 'true' } })).toEqual([])
    await expect(openCodeSkillPaths({ cwd, home, env: {} })).rejects.toThrow(`OpenCode’s settings in ${join(project, 'opencode.jsonc')} aren’t valid JSON, so nothing was started. Fix them, then send again.`)
  })

  it('keeps an empty configuration empty and returns only string paths', async () => {
    const { home, cwd } = await fixture()
    expect(await openCodeSkillPaths({ cwd, home, env: {} })).toEqual([])
    expect(await openCodeSkillPaths({ cwd, home, env: { OPENCODE_CONFIG_CONTENT: '{"skills":{"paths":[false,1,null,"/kept",""]}}' } })).toEqual(['/kept'])
  })
})
