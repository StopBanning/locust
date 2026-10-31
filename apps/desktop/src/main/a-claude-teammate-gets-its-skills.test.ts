import { mkdir, mkdtemp, readFile, readdir, rm, stat, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import { clearClaudeSkillCopies, prepareClaudeSkills, skillFoldersIn } from './claude-skills.js'

/**
 * A CLAUDE CODE TEAMMATE GETS ITS SKILLS (0.679).
 *
 * `--restricted` drops the folder's `.claude/skills` and the person's own, and
 * a plugin named with `--plugin-dir` is the one way back (measured
 * 2026-10-04). So each run is handed copies: the folder's always, the
 * person's only when Settings lends them, and the copies -- never the
 * originals -- are what is removed when the run ends.
 */
const made: string[] = []
afterEach(async () => {
  for (const dir of made.splice(0)) await rm(dir, { recursive: true, force: true })
})
async function scratch(prefix: string): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), prefix))
  made.push(dir)
  return dir
}
async function skill(root: string, name: string, body = `---\nname: ${name}\ndescription: test\n---\nSay hi.\n`): Promise<void> {
  await mkdir(join(root, name), { recursive: true })
  await writeFile(join(root, name, 'SKILL.md'), body)
}

describe('a Claude Code teammate gets its skills', () => {
  it("the folder's skills always, as the plugin `project`, named the way a run calls them", async () => {
    const workspace = await scratch('locust-skills-ws-')
    const home = await scratch('locust-skills-home-')
    const copies = await scratch('locust-skills-copies-')
    await skill(join(workspace, '.claude', 'skills'), 'review')
    await writeFile(join(workspace, '.claude', 'skills', 'review', 'checklist.md'), 'one\n')
    await skill(join(home, '.claude', 'skills'), 'mine')
    const prepared = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: false, homeDirectory: home })
    expect(prepared.plugins.map((plugin) => plugin.name)).toEqual(['project'])
    expect(prepared.plugins[0]!.skills).toEqual(['project:review'])
    const dir = prepared.plugins[0]!.dir
    expect(JSON.parse(await readFile(join(dir, '.claude-plugin', 'plugin.json'), 'utf8'))).toMatchObject({ name: 'project' })
    // The whole skill folder, its supporting files with it.
    expect(await readFile(join(dir, 'skills', 'review', 'checklist.md'), 'utf8')).toBe('one\n')
    await prepared.dispose()
    expect(await readdir(copies)).toEqual([])
    // The original is untouched.
    expect((await stat(join(workspace, '.claude', 'skills', 'review', 'SKILL.md'))).isFile()).toBe(true)
  })

  it("the person's own only when Settings lends them, as the plugin `personal`", async () => {
    const workspace = await scratch('locust-skills-ws-')
    const home = await scratch('locust-skills-home-')
    const copies = await scratch('locust-skills-copies-')
    await skill(join(home, '.claude', 'skills'), 'shipcheck')
    const off = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: false, homeDirectory: home })
    expect(off.plugins).toEqual([])
    const on = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: true, homeDirectory: home })
    expect(on.plugins.map((plugin) => plugin.skills)).toEqual([['personal:shipcheck']])
    await on.dispose()
    await on.dispose()
  })

  it('only folders holding a SKILL.md, with a name a skill can have, and never a link', async () => {
    const root = await scratch('locust-skills-root-')
    await skill(root, 'good')
    await mkdir(join(root, 'notes'))
    await writeFile(join(root, 'notes', 'README.md'), 'not a skill\n')
    await writeFile(join(root, 'loose.md'), 'a file, not a folder\n')
    await skill(root, '.hidden')
    await skill(root, 'has space')
    const elsewhere = await scratch('locust-skills-elsewhere-')
    await skill(elsewhere, 'linked')
    const linked = await symlink(join(elsewhere, 'linked'), join(root, 'linked'), 'junction').then(() => true, () => false)
    expect(await skillFoldersIn(root)).toEqual(['good'])
    expect(linked).toBe(true)
  })

  it('a link inside a skill is not copied, so removing the copy cannot reach past it', async () => {
    const workspace = await scratch('locust-skills-ws-')
    const copies = await scratch('locust-skills-copies-')
    const precious = await scratch('locust-skills-precious-')
    await writeFile(join(precious, 'keep.txt'), 'keep\n')
    const root = join(workspace, '.claude', 'skills')
    await skill(root, 'tricky')
    await symlink(precious, join(root, 'tricky', 'outside'), 'junction')
    const prepared = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: false })
    const dir = prepared.plugins[0]!.dir
    expect(await readdir(join(dir, 'skills', 'tricky'))).toEqual(['SKILL.md'])
    await prepared.dispose()
    await clearClaudeSkillCopies(copies)
    expect(await readFile(join(precious, 'keep.txt'), 'utf8')).toBe('keep\n')
  })

  it('a folder with no skills hands over nothing and leaves nothing behind', async () => {
    const workspace = await scratch('locust-skills-ws-')
    const copies = await scratch('locust-skills-copies-')
    const prepared = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: false })
    expect(prepared.plugins).toEqual([])
    expect(await readdir(copies)).toEqual([])
  })

  it('a skill folder past the size bound is left out', async () => {
    const root = await scratch('locust-skills-root-')
    await skill(root, 'small')
    await skill(root, 'huge')
    await writeFile(join(root, 'huge', 'blob.bin'), Buffer.alloc(11 * 1024 * 1024))
    expect(await skillFoldersIn(root)).toEqual(['small'])
  })
})
