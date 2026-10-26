import { cp, lstat, mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises'
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
    expect([...(await readdir(prepared!.dir))].sort()).toEqual(['.agents', '.plugin', 'skills'])
    expect((await readdir(prepared!.skillsDir)).sort()).toEqual(['evidence-first', 'review', 'tune-candidate'])
    // The teammate's own copy, not the library's of the same name.
    expect(await readFile(join(prepared!.skillsDir, 'evidence-first', 'SKILL.md'), 'utf8')).toContain('The teammate’s own.')
    expect(await readFile(join(prepared!.skillsDir, 'tune-candidate', 'SKILL.md'), 'utf8')).toContain('name: tune-candidate')
    // Copilot's manifest points at that same folder. Antigravity gets a copy, not a link.
    expect(JSON.parse(await readFile(join(prepared!.dir, '.plugin', 'plugin.json'), 'utf8'))).toMatchObject({ name: 'locust-run-skills', skills: 'skills' })
    expect([...(await readdir(join(prepared!.dir, '.agents', 'skills'))).sort()]).toEqual(['evidence-first', 'review', 'tune-candidate'])
    expect(await readFile(join(prepared!.dir, '.agents', 'skills', 'evidence-first', 'SKILL.md'), 'utf8')).toContain('The teammate’s own.')
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
    if (linked) {
      expect(await readdir(join(prepared!.skillsDir, 'review'))).toEqual(['SKILL.md'])
      expect(await readdir(join(prepared!.dir, '.agents', 'skills', 'review'))).toEqual(['SKILL.md'])
    }
    await prepared!.dispose()
    expect(await readFile(join(elsewhere, 'precious.txt'), 'utf8')).toBe('keep me')
  })

  it('keeps the candidate on Windows when a library folder differs only in case', async () => {
    const root = await scratch('locust-skill-case-')
    await skill(join(root, 'library'), 'Review', 'The library must not win.')
    await skill(join(root, 'library'), 'another-folder')
    await writeFile(join(root, 'library', 'another-folder', 'SKILL.md'), '---\nname: ReView\ndescription: A case-only identity collision.\n---\nThis must not be offered.\n')
    const prepared = await prepareRunSkills({ scratchRoot: join(root, 'runs'), libraryFolder: join(root, 'library'), platform: 'win32', candidate: { name: 'review', description: 'Candidate.', body: 'The candidate wins.' } })
    expect(prepared!.skills).toEqual(['review'])
    expect(await readFile(join(prepared!.skillsDir, 'review', 'SKILL.md'), 'utf8')).toContain('The candidate wins.')
    expect(await readdir(prepared!.skillsDir)).toEqual(['review'])
  })

  it('keeps the first frontmatter name even when the directory names and YAML spellings differ', async () => {
    const root = await scratch('locust-skill-name-')
    const teammate = join(root, 'mate')
    const library = join(root, 'library')
    await skill(teammate, 'a-different-folder')
    await writeFile(join(teammate, 'a-different-folder', 'SKILL.md'), '---\nname: "review" # a name, not the directory\ndescription: First.\n---\nThe teammate wins.\n')
    await skill(library, 'z-library')
    await writeFile(join(library, 'z-library', 'SKILL.md'), "---\nname: 'review'\ndescription: Later.\n---\nThe library must not win.\n")
    const first = await prepareRunSkills({ scratchRoot: join(root, 'runs'), teammateFolder: teammate, teammateSkills: ['a-different-folder'], libraryFolder: library })
    expect(first!.skills).toEqual(['a-different-folder'])
    const candidate = await prepareRunSkills({ scratchRoot: join(root, 'runs'), teammateFolder: teammate, teammateSkills: ['a-different-folder'], libraryFolder: library, candidate: { name: 'review', description: 'Candidate.', body: 'The candidate wins.' } })
    expect(candidate!.skills).toEqual(['review'])
    expect(await readFile(join(candidate!.skillsDir, 'review', 'SKILL.md'), 'utf8')).toContain('The candidate wins.')
  })

  it('keeps case-distinct skills on a case-sensitive machine', async () => {
    const root = await scratch('locust-skill-sensitive-')
    await skill(join(root, 'library'), 'Review')
    const prepared = await prepareRunSkills({ scratchRoot: join(root, 'runs'), libraryFolder: join(root, 'library'), platform: 'linux', candidate: { name: 'review', description: 'Candidate.', body: 'First.' } })
    // Windows itself cannot hold both directories; the key's semantics can still be proved on every host.
    const { skillNameKey } = await import('./skill-discovery.js')
    expect(skillNameKey('Review', 'linux')).not.toBe(skillNameKey('review', 'linux'))
    if (process.platform !== 'win32') expect(prepared!.skills).toEqual(['review', 'Review'])
  })

  it('publishes no part of a skill whose copy fails after SKILL.md arrived', async () => {
    const root = await scratch('locust-skill-partial-')
    await skill(join(root, 'library'), 'broken')
    const prepared = await prepareRunSkills({
      scratchRoot: join(root, 'runs'), libraryFolder: join(root, 'library'),
      candidate: { name: 'good', description: 'Good.', body: 'Keep this one.' },
      copyFolder: async (source, destination, options) => { await cp(source, destination, options); throw new Error('Disk read failed after SKILL.md.') }
    })
    expect(prepared!.skills).toEqual(['good'])
    expect(await readdir(prepared!.skillsDir)).toEqual(['good'])
    expect(await readdir(join(prepared!.dir, '.agents', 'skills'))).toEqual(['good'])
    expect((await readdir(prepared!.dir)).sort()).toEqual(['.agents', '.plugin', 'skills'])
  })
})

describe('which agents take a skill', () => {
  it('is Claude Code, Codex, OpenCode, Copilot and Antigravity, and a comparison says so for any other', () => {
    expect(['claude', 'codex', 'opencode', 'copilot', 'antigravity'].every(takesSkills)).toBe(true)
    expect(['gemini', 'cursor', 'muse', 'kilo', 'acp', undefined].some(takesSkills)).toBe(false)
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

  it('allows the copied skill subtree through native rules without changing edit or shell denials', () => {
    const config = JSON.parse(openCode2Launch(executable, { workspacePath: '/project', sandbox: 'read-only', approveEach: false, skillsDir: '/profile/run-skills/abc/skills' }).env.OPENCODE_CONFIG_CONTENT!)
    expect(config.agents.locust.permissions).toEqual(expect.arrayContaining([
      { action: 'external_directory', resource: '*', effect: 'deny' },
      { action: 'external_directory', resource: '/profile/run-skills/abc/skills/**', effect: 'allow' },
      { action: 'edit', resource: '*', effect: 'deny' },
      { action: 'shell', resource: '*', effect: 'ask' }
    ]))
  })
})

describe('the mission service', () => {
  it('hands the folder to Codex on its app-server, OpenCode, Copilot and Antigravity, never on a cloud computer, and removes it after', () => {
    const source = readFileSync(new URL('./codex-mission.ts', import.meta.url), 'utf8')
    expect(source).toContain("((runtime === 'codex' && codexStreams) || runtime === 'opencode' || runtime === 'copilot' || runtime === 'antigravity')")
    expect(source).toContain('skillPlugin: runSkills.dir')
    expect(source).toContain('skillWorkspace: runSkills.dir')
    expect(source).toContain('cloud === undefined && options.runSkills !== undefined')
    expect(source).toContain('const clear = (): void => void runSkills.dispose()')
  })
})
