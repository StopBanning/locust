import { mkdtemp, readFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { expect, it } from 'vitest'
import { createCursorPrintCommand } from '@teammate/runtime-adapters'
import { prepareRunSkills } from './run-skills.js'
import { SKILL_RUNTIME_NAMES, takesSkills } from '../shared/skill-runtimes.js'

it('gives Cursor a disposable plugin manifest pointing at the copied skills', async () => {
  const root = await mkdtemp(join(tmpdir(), 'cursor-skills-'))
  try {
    const prepared = await prepareRunSkills({ scratchRoot: root, candidate: { name: 'canary', description: 'A canary.', body: 'QUARTZFINCH' } })
    const manifest = JSON.parse(await readFile(join(prepared!.dir, '.cursor-plugin', 'plugin.json'), 'utf8'))
    expect(manifest).toMatchObject({ name: 'locust-run-skills', skills: 'skills' })
    expect(await readFile(join(prepared!.dir, manifest.skills, 'canary', 'SKILL.md'), 'utf8')).toContain('QUARTZFINCH')
    for (const sandbox of ['read-only', 'workspace-write', 'full-access'] as const) {
      const executable = { commandName: 'cursor-agent', discoveredPath: '/bin/cursor-agent', executablePath: '/bin/cursor-agent', prefixArgs: [], kind: 'native' as const }
      const spec = createCursorPrintCommand(executable, { workspacePath: root, sandbox, skillPlugin: prepared!.dir })
      const at = spec.args.indexOf('--plugin-dir')
      expect(at).toBeGreaterThan(-1)
      expect(spec.args[at + 1]).toBe(prepared!.dir)
      expect(spec.args).not.toContain('--add-dir')
      expect(createCursorPrintCommand(executable, { workspacePath: root, sandbox }).args).not.toContain('--plugin-dir')
    }
    expect(takesSkills('cursor')).toBe(true)
    expect(SKILL_RUNTIME_NAMES).toContain('Cursor')
    await prepared!.dispose()
    await expect(readFile(join(prepared!.dir, '.cursor-plugin', 'plugin.json'))).rejects.toThrow()
  } finally { await rm(root, { recursive: true, force: true }) }
})
