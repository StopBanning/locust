import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, describe, expect, it } from 'vitest'

import type { PublicCompare } from '../shared/compare.js'
import {
  keptSkill,
  otherSkill,
  parseTuneDrafts,
  repeatsOf,
  skillMarkdown,
  tuneRoutes,
  tuneSkillOf,
  TUNE_FALLBACK,
  type TuneSkill
} from '../shared/tune.js'
import { prepareClaudeSkills } from './claude-skills.js'
import { compareSkillRefusal } from './compare-instructions.js'
import { createCompareStore } from './compare-store.js'
import { createTeammateSkillStore, keptSkillText, MAX_SKILLS_PER_TEAMMATE } from './teammate-skills.js'
import { tuneDraftPrompt } from './tune-drafts.js'
import { tuneLessonPrompt } from './tune-lessons.js'

/*
 * WHAT IT KNOWS, AS A SKILL (0.811). Harness² finds skills the strongest of a teammate's levers and standing
 * instructions the weakest, so for a Claude Code teammate a tune drafts "What it knows" as one skill: that column's run
 * alone is handed it, beside Current's instructions. Kept and used, it is the teammate's from then on -- a folder of
 * Claude Code's own shape in Locust's profile, handed to that teammate's runs as the `teammate` plugin.
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

const skill: TuneSkill = {
  name: 'check-the-cause',
  description: 'Use when explaining why something failed: name the evidence before the cause.',
  body: '- Quote the line that shows the failure.\n- Say which assumption the cause rests on.'
}
const current = 'Be concise.'
const how = { instructions: 'Be concise. Check your work before you finish.', why: 'The answer stopped before checking.' }
const knows = { instructions: current, why: 'The answer named a cause with no evidence.', skill }
const route = { runtime: 'claude' as const, model: 'claude-haiku-5-5', label: 'Haiku 5.5' }

describe('a skill read whole', () => {
  it('takes a name Claude Code takes, one line on when, and what it says', () => {
    expect(tuneSkillOf({ name: ' check-the-cause ', description: 'Use when\n  explaining.', body: '\r\nSay it.\r\n' })).toEqual({ name: 'check-the-cause', description: 'Use when explaining.', body: 'Say it.' })
    for (const name of ['Check-The-Cause', 'check the cause', '-check', 'check--cause', 'check-', 'a'.repeat(65), '', 'check_cause']) {
      expect(tuneSkillOf({ ...skill, name })).toBeUndefined()
    }
    expect(tuneSkillOf({ ...skill, description: '' })).toBeUndefined()
    expect(tuneSkillOf({ ...skill, description: `Use when\u0007.` })).toBeUndefined()
    expect(tuneSkillOf({ ...skill, description: 'x'.repeat(1_025) })).toBeUndefined()
    expect(tuneSkillOf({ ...skill, body: '   ' })).toBeUndefined()
    expect(tuneSkillOf({ ...skill, body: 'x'.repeat(8_001) })).toBeUndefined()
    expect(tuneSkillOf([skill])).toBeUndefined()
    expect(tuneSkillOf('check-the-cause')).toBeUndefined()
  })

  it('is written as Claude Code reads a SKILL.md: a colon in its description cannot break the header', () => {
    expect(skillMarkdown({ ...skill, description: 'Use when: a cause is asked for.' })).toBe(
      '---\nname: check-the-cause\ndescription: "Use when: a cause is asked for."\n---\n\n- Quote the line that shows the failure.\n- Say which assumption the cause rests on.\n'
    )
  })
})

describe('the drafts of a tune of a Claude Code teammate', () => {
  it('are asked for "knows" as a skill, and only for Claude Code', () => {
    const asked = tuneDraftPrompt(current, [], { skills: true })
    expect(asked).toContain('This teammate runs on Claude Code')
    expect(asked).toContain('"knows":{"skill":{"name":')
    expect(asked).toContain('"how":{"instructions":"complete replacement text"')
    const plain = tuneDraftPrompt(current)
    expect(plain).not.toContain('skill')
    expect(plain).toContain('"knows":{"instructions":"complete replacement text"')
  })

  it('take "knows" as a skill beside Current, and nothing but a skill there', () => {
    const raw = JSON.stringify({ how, knows: { skill, why: knows.why } })
    expect(parseTuneDrafts(raw, current, { skills: true })).toEqual({ how, knows })
    // Not asked for a skill: a skill-only "knows" is no edit at all.
    expect(parseTuneDrafts(raw, current)).toEqual({ how: { instructions: current, why: TUNE_FALLBACK }, knows: { instructions: current, why: TUNE_FALLBACK } })
    // A skill that will not read whole, or a skill under "how", is read as no edits.
    expect(parseTuneDrafts(JSON.stringify({ how, knows: { skill: { ...skill, name: 'Bad Name' }, why: knows.why } }), current, { skills: true }).knows.skill).toBeUndefined()
    expect(parseTuneDrafts(JSON.stringify({ how: { skill, why: 'x' }, knows: { skill, why: knows.why } }), current, { skills: true }).how.instructions).toBe(current)
    // Read past a think block, a fence, or a line before the object, as the lesson and the combine are.
    const fence = '```'
    expect(parseTuneDrafts(`<think>hm</think>Here are the drafts:\n${fence}json\n${raw}\n${fence}`, current, { skills: true })).toEqual({ how, knows })
    expect(parseTuneDrafts(`Here are the drafts: ${raw}`, current, { skills: true })).toEqual({ how, knows })
    // Keeping things as they are stays a valid "knows".
    expect(parseTuneDrafts(JSON.stringify({ how, knows: { instructions: current, why: 'Nothing to add.' } }), current, { skills: true }).knows).toEqual({ instructions: current, why: 'Nothing to add.' })
  })

  it('run the skill in its own column alone, over Current as the card has it', () => {
    const routes = tuneRoutes(route, 'Be concise. Edited on the card.', { how, knows })
    expect(routes.map((one) => one.skill)).toEqual([undefined, undefined, skill])
    expect(routes[2]!.instructions).toBe('Be concise. Edited on the card.')
    expect(routes.map((one) => one.label)).toEqual(['Haiku 5.5 · Current', 'Haiku 5.5 · How it works', 'Haiku 5.5 · What it knows'])
  })

  it('never call a skill setup the same as Current', () => {
    expect(repeatsOf(current, { how: { instructions: current, why: 'Keep.' }, knows })).toEqual({ how: 'Current', distinct: 2 })
  })
})

describe('a column with a skill', () => {
  it('starts only on Claude Code, and only beside its instructions', () => {
    expect(compareSkillRefusal([{ ...route, instructions: current, skill }])).toBeUndefined()
    // Codex and OpenCode take one since 0.813 (codex-and-opencode-teammates-get-their-skills.test.ts); Gemini does not.
    expect(compareSkillRefusal([{ runtime: 'codex', model: 'gpt-6', instructions: current, skill }])).toBeUndefined()
    expect(compareSkillRefusal([{ runtime: 'gemini', model: 'account-default', instructions: current, skill }])).toBe('A skill reaches only a Claude Code, Codex, OpenCode, Copilot or Antigravity teammate for now. Nothing was started.')
    expect(compareSkillRefusal([{ ...route, skill }])).toBe('A skill goes with a tune’s column, beside its instructions. Nothing was started.')
  })

  it('is remembered whole with its tune, and one that will not read whole is refused', async () => {
    const root = await scratch('locust-tune-skill-cmp-')
    const store = createCompareStore({ rootDirectory: root, createId: () => 'cmp_skill' })
    const made = await store.create({ teammateId: 'tm_wren', prompt: 'Why did it fail?', routes: tuneRoutes(route, current, { how, knows }), blind: true })
    expect(made.slots[2]!.route.skill).toEqual(skill)
    expect((await createCompareStore({ rootDirectory: root }).get('cmp_skill'))?.slots[2]!.route.skill).toEqual(skill)
    await expect(store.create({ teammateId: 'tm_wren', prompt: 'x', routes: [{ ...route, instructions: current, skill: { ...skill, name: 'Bad Name' } }] })).rejects.toThrow('A skill goes with a tune’s column, whole and valid. Nothing was started.')
  })
})

describe('the skill a kept tune offers', () => {
  const tune = (slot: 'a' | 'b' | 'c'): PublicCompare => ({
    compareId: 'cmp_skill',
    teammateId: 'tm_wren',
    prompt: 'Why did it fail?',
    createdAt: '2026-10-21T12:00:00.000Z',
    blind: true,
    kept: { slot, at: '2026-10-21T12:00:00.000Z' },
    slots: tuneRoutes(route, current, { how, knows }).map((one, index) => ({ slot: (['a', 'b', 'c'] as const)[index]!, route: one, missionIds: [`m_${String(index)}`] }))
  })

  it('is the kept setup’s own, or with How it works kept the other’s, beside it; none when Current was kept', () => {
    expect(keptSkill(tune('c'))).toEqual(skill)
    expect(otherSkill(tune('c'))).toBeUndefined()
    expect(keptSkill(tune('b'))).toBeUndefined()
    expect(otherSkill(tune('b'))).toEqual({ setup: 'What it knows', skill })
    expect(keptSkill(tune('a'))).toBeUndefined()
    expect(otherSkill(tune('a'))).toBeUndefined()
    const { kept: _kept, ...undecided } = tune('c')
    expect(keptSkill(undecided)).toBeUndefined()
  })
})

describe('a teammate’s own skills', () => {
  it('are kept as Claude Code’s own shape, one a name, with the tune each came from', async () => {
    const root = await scratch('locust-teammate-skills-')
    const store = createTeammateSkillStore({ rootDirectory: root, now: () => new Date('2026-10-21T12:00:00.000Z') })
    expect(await store.list()).toEqual([])
    const kept = await store.add('tm_wren', skill, 'cmp_skill')
    expect(kept).toEqual({ teammateId: 'tm_wren', name: skill.name, description: skill.description, compareId: 'cmp_skill', createdAt: '2026-10-21T12:00:00.000Z' })
    expect(await keptSkillText(store, 'tm_wren', skill.name)).toBe(skillMarkdown(skill))
    // The same name again replaces it: one entry, the new words.
    await store.add('tm_wren', { ...skill, description: 'Use when a cause is asked for.' })
    expect((await store.list('tm_wren')).map((one) => [one.name, one.description, one.compareId])).toEqual([[skill.name, 'Use when a cause is asked for.', undefined]])
    expect(await keptSkillText(store, 'tm_wren', skill.name)).toContain('Use when a cause is asked for.')
    // Nothing is left beside it: no staged or set-aside folder.
    expect(await readdir(store.folderOf('tm_wren'))).toEqual([skill.name])
    await store.add('tm_ada', { ...skill, name: 'other-skill' })
    expect((await store.list()).map((one) => one.teammateId)).toEqual(['tm_wren', 'tm_ada'])
    expect((await store.list('tm_ada')).map((one) => one.name)).toEqual(['other-skill'])
    await store.remove('tm_wren', skill.name)
    expect(await store.list('tm_wren')).toEqual([])
    expect(await keptSkillText(store, 'tm_wren', skill.name)).toBeUndefined()
    await store.removeForTeammate('tm_ada')
    expect(await store.list()).toEqual([])
    expect(await readdir(join(root, 'teammate-skills'))).toEqual(['skills.json', 'tm_wren'])
    expect(await readdir(store.folderOf('tm_wren'))).toEqual([])
  })

  it('refuse what will not read whole, past twenty, and a record that cannot be read', async () => {
    const root = await scratch('locust-teammate-skills-')
    const store = createTeammateSkillStore({ rootDirectory: root })
    await expect(store.add('tm_wren', { ...skill, name: 'Bad Name' })).rejects.toThrow('That skill could not be saved: it did not read whole. The teammate is unchanged.')
    await expect(store.add('../escape', skill)).rejects.toThrow('That skill could not be saved: it did not read whole. The teammate is unchanged.')
    for (let index = 0; index < MAX_SKILLS_PER_TEAMMATE; index += 1) await store.add('tm_wren', { ...skill, name: `skill-${String(index)}` })
    await expect(store.add('tm_wren', { ...skill, name: 'one-more' })).rejects.toThrow('A teammate keeps at most 20 skills. Remove one in Edit teammate first.')
    // Replacing one of the twenty is not one more.
    await expect(store.add('tm_wren', { ...skill, name: 'skill-3', body: 'New words.' })).resolves.toMatchObject({ name: 'skill-3' })
    await writeFile(join(root, 'teammate-skills', 'skills.json'), '{ not json')
    await expect(store.list()).rejects.toThrow('The teammate skills record on disk could not be read.')
  })
})

describe('a Claude Code run of the teammate', () => {
  async function teammateFolder(): Promise<{ readonly folder: string; readonly copies: string; readonly workspace: string }> {
    const folder = await scratch('locust-teammate-folder-')
    for (const name of ['check-the-cause', 'unlisted']) {
      await mkdir(join(folder, name), { recursive: true })
      await writeFile(join(folder, name, 'SKILL.md'), skillMarkdown({ ...skill, name, body: `Kept ${name}.` }))
    }
    // A save in flight is never copied.
    await mkdir(join(folder, '.check-the-cause.0000.tmp'), { recursive: true })
    await writeFile(join(folder, '.check-the-cause.0000.tmp', 'SKILL.md'), 'half written')
    return { folder, copies: await scratch('locust-teammate-copies-'), workspace: await scratch('locust-teammate-ws-') }
  }

  it('is handed the skills its record lists, as the plugin `teammate`, Auto included', async () => {
    const { folder, copies, workspace } = await teammateFolder()
    for (const auto of [false, true]) {
      const prepared = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: false, auto, teammateFolder: folder, teammateSkills: ['check-the-cause'] })
      expect(prepared.plugins.map((plugin) => [plugin.name, plugin.skills])).toEqual([['teammate', ['teammate:check-the-cause']]])
      const dir = prepared.plugins[0]!.dir
      expect(JSON.parse(await readFile(join(dir, '.claude-plugin', 'plugin.json'), 'utf8'))).toMatchObject({ name: 'teammate', description: "This teammate's own skills" })
      expect(await readFile(join(dir, 'skills', 'check-the-cause', 'SKILL.md'), 'utf8')).toContain('Kept check-the-cause.')
      await prepared.dispose()
    }
  })

  it('in a tune’s column is handed that column’s skill, standing in for a kept one of its name', async () => {
    const { folder, copies, workspace } = await teammateFolder()
    const prepared = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: false, teammateFolder: folder, teammateSkills: ['check-the-cause', 'unlisted'], candidate: skill })
    expect(prepared.plugins[0]!.skills).toEqual(['teammate:check-the-cause', 'teammate:unlisted'])
    expect(await readFile(join(prepared.plugins[0]!.dir, 'skills', 'check-the-cause', 'SKILL.md'), 'utf8')).toBe(skillMarkdown(skill))
    await prepared.dispose()
    // A column with a skill and a teammate with none.
    const alone = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: false, candidate: skill })
    expect(alone.plugins.map((plugin) => plugin.skills)).toEqual([['teammate:check-the-cause']])
    await alone.dispose()
    expect(await readdir(copies)).toEqual([])
  })

  it('is handed none when the teammate keeps none', async () => {
    const { copies, workspace } = await teammateFolder()
    const prepared = await prepareClaudeSkills({ scratchRoot: copies, workspacePath: workspace, ownSkills: false, teammateFolder: join(copies, 'missing'), teammateSkills: [] })
    expect(prepared.plugins).toEqual([])
  })
})

describe('Note what this tune showed', () => {
  it('is told which setup added a skill, and what it said', () => {
    const asked = tuneLessonPrompt({
      ask: 'Why did it fail?',
      setups: [
        { setup: 'Current', instructions: current, answer: 'It failed.' },
        { setup: 'How it works', instructions: how.instructions, why: how.why, answer: 'It failed; checked.' },
        { setup: 'What it knows', instructions: current, why: knows.why, skill, answer: 'Line 4 shows it failed.' }
      ],
      kept: 'What it knows',
      usedInstructions: true
    })
    expect(asked).toContain('and any skill a setup added')
    expect(asked).toContain('A setup\'s "skill" is one it added beside the same standing instructions')
    expect(asked).toContain(JSON.stringify({ name: skill.name, description: skill.description, body: skill.body }))
    const plain = tuneLessonPrompt({ ask: 'x', setups: [{ setup: 'Current', instructions: current, answer: 'y' }], kept: 'Current', usedInstructions: true })
    expect(plain).not.toContain('skill')
  })
})
