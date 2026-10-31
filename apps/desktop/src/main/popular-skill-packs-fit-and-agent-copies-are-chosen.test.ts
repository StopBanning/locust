import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterEach, expect, it } from 'vitest'
import { createSkillLibrary, gitBlobId, type Fetcher } from './skill-library.js'
import { prepareClaudeSkills } from './claude-skills.js'
import { POPULAR_SKILLS, POPULAR_SKILLS_AS_OF, skillPreviewPicks } from '../shared/popular-skills.js'
import { isOutboundLink } from '../shared/outbound-links.js'

const sha = 'a'.repeat(40)
const roots: string[] = []
afterEach(async () => { await Promise.all(roots.splice(0).map((root) => rm(root, { recursive: true, force: true }))) })
async function fixture(files: Record<string, string>) {
  const root = await mkdtemp(join(tmpdir(), 'locust-popular-skills-'))
  roots.push(root)
  const asked: string[] = []
  const fetch: Fetcher = async (url) => {
    asked.push(url)
    if (url.endsWith('/repos/acme/pack')) return Response.json({ full_name: 'acme/pack', default_branch: 'main' })
    if (url.endsWith('/commits/main')) return Response.json({ sha })
    if (url.includes('/git/trees/')) return Response.json({ tree: Object.entries(files).map(([path, text]) => ({ path, type: 'blob', mode: '100644', sha: gitBlobId(Buffer.from(text)), size: Buffer.byteLength(text) })) })
    const path = decodeURIComponent(url.split(`/${sha}/`)[1] ?? '')
    return new Response(files[path] ?? null, { status: files[path] === undefined ? 404 : 200 })
  }
  return { root, asked, library: createSkillLibrary({ root: join(root, 'library'), fetch }) }
}

it('a font-bearing skill above five MB is kept and handed to Claude Code but ten MB remains the ceiling', async () => {
  const { root, library } = await fixture({ 'skills/canvas-design/SKILL.md': 'Draw.', 'skills/canvas-design/font.ttf': 'f'.repeat(6 * 1024 * 1024), 'skills/oversize/SKILL.md': 'x'.repeat(10 * 1024 * 1024 + 1) })
  const preview = await library.preview('acme/pack')
  expect(preview.skills.find((skill) => skill.name === 'canvas-design')?.held).toBeUndefined()
  expect(preview.skills.find((skill) => skill.name === 'oversize')?.held).toMatch(/Larger than 10 MB/)
  await expect(library.install({ ...preview, names: ['oversize'] })).rejects.toThrow(/not a skill/)
  await library.install({ ...preview, names: ['canvas-design'] })
  const run = await prepareClaudeSkills({ scratchRoot: join(root, 'runs'), workspacePath: join(root, 'workspace'), ownSkills: false, libraryFolder: library.skillsFolder, auto: true })
  try {
    expect(run.plugins[0]?.skills).toEqual(['library:canvas-design'])
    expect((await readFile(join(run.plugins[0]!.dir, 'skills/canvas-design/font.ttf'))).length).toBe(6 * 1024 * 1024)
  } finally { await run.dispose() }
})

it('agent copies choose skills then Claude then agents then any without hiding real name clashes', async () => {
  const files: Record<string, string> = {}
  for (const name of ['root-first', 'claude-next', 'agents-next', 'any-agent']) {
    for (const agent of ['.agent', '.agents', '.claude', '.codex', '.cursor', '.opencode', '.github', '.gemini']) files[`${agent}/skills/${name}/SKILL.md`] = agent
  }
  files['bundle/.agent/skills/nested/SKILL.md'] = 'agent'
  files['bundle/.claude/skills/nested/SKILL.md'] = 'claude'
  files['skills/root-first/SKILL.md'] = 'root'
  delete files['.claude/skills/agents-next/SKILL.md']
  delete files['.claude/skills/any-agent/SKILL.md']
  delete files['.agents/skills/any-agent/SKILL.md']
  files['different/claude-next/SKILL.md'] = 'a different skill with a real clash'
  files['tests/.claude/skills/root-first/SKILL.md'] = 'a fixture, not the root copy'
  const { library } = await fixture(files)
  const preview = await library.preview('acme/pack')
  const offered = preview.skills.filter((skill) => skill.held === undefined)
  expect(offered.map((skill) => skill.folder).sort()).toEqual(['.agent/skills/any-agent', '.agents/skills/agents-next', '.claude/skills/claude-next', 'bundle/.claude/skills/nested', 'skills/root-first'])
  expect(preview.skills.filter((skill) => skill.held !== undefined).map((skill) => skill.folder).sort()).toEqual(['different/claude-next', 'tests/.claude/skills/root-first'])
  await library.install({ ...preview, names: offered.map((skill) => skill.name) })
  expect(await readFile(join(library.skillsFolder, 'claude-next/SKILL.md'), 'utf8')).toBe('.claude')
  expect(await readFile(join(library.skillsFolder, 'nested/SKILL.md'), 'utf8')).toBe('claude')
  expect((await library.list())[0]?.skills).toHaveLength(5)
})

it('a popular pack is only previewed on the first click and all offered skills are ready for the second', async () => {
  const { library, asked } = await fixture({ 'skills/review/SKILL.md': 'Review.', 'skills/review/scripts/check.py': 'print(1)', 'skills/bad name/SKILL.md': 'held' })
  const preview = await library.preview('acme/pack')
  expect(await library.list()).toEqual([])
  expect(asked.filter((url) => url.startsWith('https://raw.')).every((url) => url.endsWith('/SKILL.md'))).toBe(true)
  expect(preview.skills.find((skill) => skill.name === 'review')?.files.filter((file) => file.runs).map((file) => file.path)).toEqual(['scripts/check.py'])
  expect([...skillPreviewPicks(preview, true)]).toEqual(['review'])
  expect([...skillPreviewPicks(preview, false)]).toEqual([])
  await library.install({ ...preview, names: [...skillPreviewPicks(preview, true)] })
  expect((await library.list())[0]?.skills.map((skill) => skill.name)).toEqual(['review'])
})

it('the five popular packs have dated stars and licenses and Claude Mem names its separate install', () => {
  expect(POPULAR_SKILLS_AS_OF).toBe('2026-10-30')
  expect(POPULAR_SKILLS.map((pack) => pack.source)).toEqual(['mattpocock/skills', 'obra/superpowers', 'anthropics/skills', 'pbakaus/impeccable', 'thedotmack/claude-mem'])
  for (const pack of POPULAR_SKILLS) {
    expect(pack.stars).toBeGreaterThan(0)
    expect(pack.license.length).toBeGreaterThan(0)
    expect(isOutboundLink(`https://github.com/${pack.source}`)).toBe(true)
  }
  expect(POPULAR_SKILLS[2].license).toBe('Apache-2.0 + source-available')
  expect(POPULAR_SKILLS[4].description).toContain('Needs its own install to work.')
})
