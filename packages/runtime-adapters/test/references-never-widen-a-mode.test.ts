import { describe, expect, it } from 'vitest'
import { createAgyPrintCommand, createClaudePrintCommand, createCodexExecCommand, createCopilotPromptCommand, createOpenCodeServeCommand, openCodeModeEnv } from '../src/commands.js'

const executable = { executablePath: 'agent', prefixArgs: [] }
const folder = 'C:/references/design kit'
const options = { workspacePath: 'C:/project', prompt: 'Read the reference.', referenceFolders: [folder] }

describe('references never widen a mode', () => {
  it('Codex never receives add-dir, including in full access', () => {
    for (const sandbox of ['read-only', 'workspace-write', 'full-access'] as const) {
      const command = createCodexExecCommand(executable, { ...options, sandbox })
      expect(command.args).not.toContain('--add-dir')
      expect(command.args).not.toContain(folder)
    }
  })
  it('Claude adds references without changing its tools, with scoped Edit protection', () => {
    const args = createClaudePrintCommand(executable, { ...options, sandbox: 'read-only' }).args
    expect(args.slice(args.indexOf('--add-dir'), args.indexOf('--add-dir') + 2)).toEqual(['--add-dir', folder])
    expect(args[args.indexOf('--tools') + 1]).not.toMatch(/\b(?:Edit|Write|Bash)\b/)
    expect(args).not.toContain('--disallowedTools')
    const editing = createClaudePrintCommand(executable, { ...options, sandbox: 'workspace-write' }).args
    expect(editing).toContain('--add-dir')
    expect(editing).toContain('Edit(//c/references/design kit/**)')
    const beforeEdit = createClaudePrintCommand(executable, { workspacePath: options.workspacePath, sandbox: 'workspace-write' }).args
    expect(editing[editing.indexOf('--tools') + 1]).toBe(beforeEdit[beforeEdit.indexOf('--tools') + 1])
    const before = createClaudePrintCommand(executable, { workspacePath: options.workspacePath, sandbox: 'full-access' }).args
    expect(createClaudePrintCommand(executable, { ...options, sandbox: 'full-access' }).args).toEqual(before)
  })
  it('OpenCode adds only named reference reads while every write denial and bash rule stays intact', () => {
    const before = JSON.parse(openCodeModeEnv({ sandbox: 'read-only' }).OPENCODE_CONFIG_CONTENT!)
    const after = JSON.parse(openCodeModeEnv({ sandbox: 'read-only', referenceFolders: [folder] }).OPENCODE_CONFIG_CONTENT!)
    expect(after.permission.external_directory).toMatchObject({ '*': 'deny', [folder]: 'allow', [`${folder}/**`]: 'allow', [`${folder}/*`]: 'allow', 'C:\\references\\design kit\\*': 'allow' })
    for (const key of ['edit', 'write', 'patch', 'bash']) expect(after.permission[key]).toEqual(before.permission[key])
    expect(after.permission.edit).toBe('deny')
    // Every shell command refused, git included (0.818).
    expect(after.permission.bash).toBe('ask')
    expect(openCodeModeEnv({ sandbox: 'read-only', referenceFolders: [folder] }).OPENCODE_PURE).toBe('1')
  })
  it('OpenCode Edit and Auto permissions are unchanged, not silently narrowed', () => {
    for (const sandbox of ['workspace-write', 'full-access'] as const) expect(openCodeModeEnv({ sandbox, referenceFolders: [folder] })).toEqual(openCodeModeEnv({ sandbox }))
    const serve = createOpenCodeServeCommand(executable, { ...options, sandbox: 'read-only' })
    expect(JSON.parse(serve.env!.OPENCODE_CONFIG_CONTENT!).permission.external_directory[`${folder}/**`]).toBe('allow')
  })
  it('Copilot and Antigravity keep their read-only posture when adding a directory', () => {
    const copilot = createCopilotPromptCommand(executable, { ...options, sandbox: 'read-only' }).args
    expect(copilot).toContain('--deny-tool=write,shell')
    expect(copilot.slice(copilot.indexOf('--add-dir'), copilot.indexOf('--add-dir') + 2)).toEqual(['--add-dir', folder])
    const agy = createAgyPrintCommand(executable, { ...options, sandbox: 'read-only' }).args
    expect(agy).toContain('--add-dir')
    expect(agy).not.toContain('--dangerously-skip-permissions')
    expect(agy).not.toContain('--mode')
  })
})
