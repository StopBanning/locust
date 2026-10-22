import { lstat, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { readFileSync } from 'node:fs'
import { afterEach, describe, expect, it } from 'vitest'
import type { ExecutableLaunch } from '@teammate/runtime-adapters'

import { SKILL_RUNTIME_ANY, takesSkills } from '../shared/skill-runtimes.js'
import { compareSkillRefusal } from './compare-instructions.js'
import { openCode2Launch } from './opencode-2-launch.js'
import { prepareRunSkills } from './run-skills.js'
import { tuneDraftPrompt } from './tune-drafts.js'

/*
 * SKILLS FOR CODEX AND OPENCODE TEAMMATES (0.813). A tune could give only a Claude Code teammate a skill, and the
 * skills kept from GitHub reached only Claude Code: Locust could hand a skill to no other agent for one run. Codex's
 * app-server and OpenCode both take a folder of skills for one run (MEASURED 2026-10-22, run-skills.ts), so a run on
 * either is handed one, copied from Locust's profile and removed after it.
 */

const made: string[] = []
afterEach(async () => {
  for (const dir of made.splice(0)) await rm(dir, { recursive: true, force: true })
})
const scratch = async (prefix: string): Promise<string> => {
  const dir = await mkdtemp(join(tmpdir(), prefix))
  made.push(dir)
  return dir
}
const skill = async (root: string, name: string, says = 'Say hi.'): Promise<void> => {
  await mkdir(join(root, name), { recursive: true })
  await writeFile(join(root, name, 'SKILL.md'), `---\nname: ${name}\ndescription: test\n---\n${says}\n`)
}

describe('a run’s own skills folder', () => {
  it('holds the teammate’s listed skills, the kept ones and a tune column’s one, the most particular winning a name', async () => {
    const scratchRoot = await scratch('locust-run-skills-')
    const teammate = await scratch('locust-run-skills-mate-')
    const library = await scratch('locust-run-skills-lib-')
    await skill(teammate, 'evidence-first', 'The teammate’s own.')
    await skill(teammate, 'not-listed')
    await skill(library, 'evidence-first', 'The library’s.')
    await skill(library, 'review')
    const prepared = await prepareRunSkills({
      scratchRoot,
      libraryFolder: library,
      teammateFolder: teammate,
      teammateSkills: ['evidence-first'],
      candidate: { name: 'tune-candidate', description: 'When the tune column runs.', body: 'Check twice.' }
    })
    expect(prepared).toBeDefined()
    expect([...prepared!.skills].sort()).toEqual(['evidence-first', 'review', 'tune-candidate'])
    expect(prepared!.skillsDir).toBe(join(prepared!.dir, 'skills'))
    expect((await readdir(prepared!.skillsDir)).sort()).toEqual(['evidence-first', 'review', 'tune-candidate'])
    // The teammate's own copy, not the library's of the same name.
    expect(await readFile(join(prepared!.skillsDir, 'evidence-first', 'SKILL.md'), 'utf8')).toContain('The teammate’s own.')
    expect(await readFile(join(prepared!.skillsDir, 'tune-candidate', 'SKILL.md'), 'utf8')).toContain('name: tune-candidate')
    // Nothing else is in it.
    expect(await readdir(prepared!.dir)).toEqual(['skills'])
    await prepared!.dispose()
    await expect(lstat(prepared!.dir)).rejects.toThrow()
    await prepared!.dispose()
  })

  it('is nothing at all when there is nothing to hand over, and leaves no folder behind', async () => {
    const scratchRoot = await scratch('locust-run-skills-')
    const teammate = await scratch('locust-run-skills-mate-')
    await skill(teammate, 'unlisted')
    expect(await prepareRunSkills({ scratchRoot, libraryFolder: join(scratchRoot, 'missing'), teammateFolder: teammate, teammateSkills: [] })).toBeUndefined()
    expect(await prepareRunSkills({ scratchRoot })).toBeUndefined()
    expect(await readdir(scratchRoot)).toEqual([])
  })

  it('copies no link, so removing the copy can never reach past it', async () => {
    const scratchRoot = await scratch('locust-run-skills-')
    const library = await scratch('locust-run-skills-lib-')
    const elsewhere = await scratch('locust-run-skills-elsewhere-')
    await writeFile(join(elsewhere, 'precious.txt'), 'keep me')
    await skill(library, 'review')
    const linked = await symlink(elsewhere, join(library, 'review', 'outside'), 'junction').then(() => true, () => false)
    const prepared = await prepareRunSkills({ scratchRoot, libraryFolder: library })
    if (linked) expect(await readdir(join(prepared!.skillsDir, 'review'))).toEqual(['SKILL.md'])
    await prepared!.dispose()
    expect(await readFile(join(elsewhere, 'precious.txt'), 'utf8')).toBe('keep me')
  })
})

describe('which agents take a skill', () => {
  it('is Claude Code, Codex and OpenCode, and a comparison says so for any other', () => {
    expect(['claude', 'codex', 'opencode'].every(takesSkills)).toBe(true)
    expect(['gemini', 'cursor', 'copilot', 'antigravity', 'muse', 'acp', undefined].some(takesSkills)).toBe(false)
    const asked = { name: 'review', description: 'When reviewing.', body: 'Check.' }
    expect(compareSkillRefusal([{ runtime: 'codex', model: 'gpt-6-mini', skill: asked, instructions: 'x' } as never])).toBeUndefined()
    expect(compareSkillRefusal([{ runtime: 'gemini', model: 'account-default', skill: asked, instructions: 'x' } as never])).toBe(`A skill reaches only a ${SKILL_RUNTIME_ANY} teammate for now. Nothing was started.`)
  })

  it('a tune names the teammate’s own agent when it drafts "What it knows" as a skill', () => {
    expect(tuneDraftPrompt('Be concise.', [], { skills: true, runtimeName: 'Codex' })).toContain('This teammate runs on Codex, which loads a skill only when the work matches')
    expect(tuneDraftPrompt('Be concise.', [], { skills: true })).toContain('This teammate runs on Claude Code,')
    const handler = readFileSync(new URL('./index.ts', import.meta.url), 'utf8')
    expect(handler).toContain('{ skills: takesSkills(teammate.route.runtime), runtimeName: runtimeDisplayName(teammate.route.runtime) ?? teammate.route.runtime }')
  })
})

describe('OpenCode 2', () => {
  const executable: ExecutableLaunch = { commandName: 'opencode', discoveredPath: '/bin/opencode', executablePath: '/bin/opencode', prefixArgs: [], kind: 'native' }
  it('lists the run’s folder in its own config’s skills, and nothing when there is none', () => {
    const withSkills = JSON.parse(openCode2Launch(executable, { workspacePath: '/project', sandbox: 'workspace-write', approveEach: false, skillsDir: '/profile/run-skills/abc/skills' }).env.OPENCODE_CONFIG_CONTENT!)
    expect(withSkills.skills).toEqual(['/profile/run-skills/abc/skills'])
    const without = JSON.parse(openCode2Launch(executable, { workspacePath: '/project', sandbox: 'workspace-write', approveEach: false }).env.OPENCODE_CONFIG_CONTENT!)
    expect(without.skills).toBeUndefined()
  })
})

describe('the mission service', () => {
  it('hands the folder only to Codex on its app-server and OpenCode, never on a cloud computer, and removes it after', () => {
    const source = readFileSync(new URL('./codex-mission.ts', import.meta.url), 'utf8')
    expect(source).toContain("((runtime === 'codex' && codexStreams) || runtime === 'opencode')")
    expect(source).toContain('cloud === undefined && options.runSkills !== undefined')
    expect(source).toContain('const clear = (): void => void runSkills.dispose()')
  })
})
