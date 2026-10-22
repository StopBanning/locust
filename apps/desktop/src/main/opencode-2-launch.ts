import { createOpenCodeServeCommand } from '@teammate/runtime-adapters'
import type { ExecutableLaunch, MissionSandbox, OpenCodeProvider } from '@teammate/runtime-adapters'
import { fileURLToPath } from 'node:url'
import { join } from 'node:path'

type Effect = 'allow' | 'ask' | 'deny'
type Rule = { readonly action: string; readonly resource: string; readonly effect: Effect }

/** The mode is owned by this turn, rather than an agent with saved permission overrides. */
export const OPENCODE_2_MODE = 'locust'

/**
 * V2 has ordered permissions and a directory-based plugin API. Reuse the
 * existing mode's resource rules, translating their documented renamed
 * actions, without passing V1's plugin or experimental flags to V2.
 */
export function openCode2Launch(executable: ExecutableLaunch, options: {
  readonly workspacePath: string
  readonly sandbox: MissionSandbox
  readonly approveEach: boolean
  readonly repositoryRoot?: string
  readonly referenceFolders?: readonly string[]
  readonly providers?: Readonly<Record<string, OpenCodeProvider>>
  readonly guardPlugin?: string
  /**
   * The run's own skills (0.813, run-skills.ts): a folder of `<name>/SKILL.md`, in V2's own `skills` list, which its
   * documentation says every config adds to. V2 is not installed on the machine this was written on, so this follows
   * the documentation; V1's way, `OPENCODE_CONFIG_DIR`, is the one measured.
   */
  readonly skillsDir?: string
}): { readonly sandbox: MissionSandbox; readonly env: Readonly<Record<string, string>> } {
  const legacy = createOpenCodeServeCommand(executable, {
    workspacePath: options.workspacePath,
    ...(options.approveEach ? {} : { sandbox: options.sandbox }),
    ...(options.repositoryRoot === undefined ? {} : { repositoryRoot: options.repositoryRoot }),
    ...(options.referenceFolders === undefined ? {} : { referenceFolders: options.referenceFolders }),
    ...(options.providers === undefined ? {} : { providers: options.providers }),
    ...(options.skillsDir === undefined ? {} : { skillsDir: options.skillsDir })
  })
  const config = JSON.parse(legacy.env!.OPENCODE_CONFIG_CONTENT!) as {
    permission: Record<string, Effect | Record<string, Effect>>
    provider?: unknown
    tools?: Record<string, boolean>
  }
  const permissions: Rule[] = [{ action: '*', resource: '*', effect: options.approveEach ? 'ask' : 'allow' }]
  for (const [key, value] of Object.entries(config.permission)) {
    /*
     * V1's own catch-all is not V2's (0.821). V1 asks about every tool its rules do not name, so a person's MCP
     * tool is refused in Ask, and allows its reading tools after it by V1's names. V2's names for those were never
     * measured -- V2 is not installed on the machine this was written on -- and a catch-all "ask" there could refuse
     * V2's own reads in Ask. V2 keeps the catch-all above, as before.
     */
    if (key === '*') continue
    const action = key === 'bash' ? 'shell' : key === 'write' || key === 'patch' ? 'edit' : key === 'task' ? 'subagent' : key
    if (typeof value === 'string') permissions.push({ action, resource: '*', effect: value })
    else {
      // V2 uses last-match semantics. Put the default before specific paths.
      const entries = Object.entries(value)
      for (const [resource, effect] of [...entries.filter(([path]) => path === '*'), ...entries.filter(([path]) => path !== '*')]) {
        permissions.push({ action, resource, effect })
      }
    }
  }
  permissions.push({ action: 'question', resource: '*', effect: 'deny' })
  if (options.sandbox === 'read-only') permissions.push({ action: 'subagent', resource: '*', effect: 'deny' })
  if (config.tools?.['*'] === false) permissions.push({ action: '*', resource: '*', effect: 'deny' })
  const native = {
    default_agent: OPENCODE_2_MODE,
    agents: { [OPENCODE_2_MODE]: { mode: 'primary', description: 'The current Locust turn', permissions } },
    ...(options.guardPlugin === undefined ? {} : { plugins: [options.guardPlugin] }),
    ...(options.skillsDir === undefined ? {} : { skills: [options.skillsDir] }),
    // OpenCode's V2 normalizer explicitly supports V1 provider declarations.
    // The host's own-provider config stays per-run and never touches its files.
    ...(config.provider === undefined ? {} : { provider: config.provider }),
    ...(options.sandbox !== 'read-only' ? {} : {
      policies: [
        { action: 'integration.use', resource: 'plugin:*', effect: 'deny' },
        ...(options.guardPlugin === undefined ? [] : [{ action: 'integration.use', resource: `plugin:${fileURLToPath(options.guardPlugin)}`, effect: 'allow' }])
      ]
    })
  }
  return { sandbox: options.sandbox, env: {
    OPENCODE_CONFIG_CONTENT: JSON.stringify(native),
    ...(options.sandbox !== 'read-only' || options.guardPlugin === undefined ? {} : {
      // V2 has no V1 PURE flag. Do not import repository or personal plugins
      // for an Ask/Plan turn; its host-owned directory contains no config.
      // Runtime credentials remain in OpenCode's own data directory.
      OPENCODE_CONFIG_PROJECT_DISABLE: '1',
      OPENCODE_DISABLE_CLAUDE_CODE: '1',
      OPENCODE_CONFIG: '',
      OPENCODE_CONFIG_DIR: join(fileURLToPath(options.guardPlugin), 'config')
    })
  } }
}
