import { readdirSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, it } from 'vitest'

const main = fileURLToPath(new URL('.', import.meta.url))
const read = (name: string) => readFileSync(new URL(name, import.meta.url), 'utf8')
const index = read('index.ts')
const service = read('codex-mission.ts')
const between = (text: string, from: string, to: string) => {
  const begin = text.indexOf(from)
  const end = text.indexOf(to, begin + from.length)
  if (begin < 0 || end < 0) throw new Error(`Missing source boundary ${from} / ${to}`)
  return text.slice(begin, end)
}

describe('every way a turn starts crosses the organization policy', () => {
  it('keeps window, routine, relay, compare, voice handoff, CLI and MCP starts on the guarded mission service', () => {
    const launches = /(?:codexMissions\.(?:start|resume|handOff)|antigravityMissions\.start|backgroundRuns\.start|cloudTasks\.start|claudeCloud\.(?:start|send|home|continueInTerminal)|voiceCalls\.start)\(/
    const callers = readdirSync(main).filter((name) => name.endsWith('.ts') && !name.endsWith('.test.ts') && launches.test(read(name)))
    expect(callers).toEqual(['index.ts'])
    // An additional launch needs to be accounted for here; routing everything through the services is the boundary.
    // 14 since 0.828: Scout's fix, in its copy, on the picked teammate's route.
    expect(index.match(/codexMissions\.start\(/g)).toHaveLength(14)
    expect(index.match(/antigravityMissions\.start\(/g)).toHaveLength(2)
    for (const factory of ['createCodexMissionService', 'createAntigravityMissionService']) {
      expect(index.slice(index.indexOf(`= ${factory}({`), index.indexOf(`= ${factory}({`) + 100)).toContain('organizationPolicy,')
    }
    const start = between(service, '    async start(', '    async handOff(')
    expect(start.indexOf('policyTurnRefusal(options.organizationPolicy')).toBeLessThan(start.indexOf('options.discover(runtime'))
    expect(start).toContain("if (managed !== undefined) return error('POLICY_REFUSED', managed)")
    expect(start).toContain('await spendRefusalFor(peer?.self.teammateId)')
    expect(index).toContain('const spendRefusal = createSpendRefusal({ policy: organizationPolicy')
  })

  it('checks a handoff target before stopping the existing run and sends resumes through start', () => {
    const target = between(service, '  async function targetRefusal(', '  const acpAgentFor = ')
    expect(target).toContain('policyTurnRefusal(options.organizationPolicy')
    const handoff = between(service, '    async handOff(', '    async resume(')
    expect(handoff.indexOf('await targetRefusal(')).toBeLessThan(handoff.indexOf('previous.controller.abort()'))
    const resume = between(service, '    async resume(', '    hasMission(')
    expect(resume).toContain('service.start(')
    expect(read('routine-runner.ts')).toContain("'SPEND_LIMIT_REACHED', 'POLICY_REFUSED'")
  })

  it('gates every separate background and cloud launch before calling its service', () => {
    const channels = [
      ['BACKGROUND_START_CHANNEL', 'backgroundRuns.start(', 'policyStartRefusal('],
      ['CLOUD_START_CHANNEL', 'cloudTasks.start(', 'policyUnmeteredRefusal('],
      ['CLAUDE_CLOUD_START_CHANNEL', 'claudeCloud.start(', 'policyUnmeteredRefusal('],
      ['CLAUDE_CLOUD_SEND_CHANNEL', 'claudeCloud.send(', 'policyUnmeteredRefusal('],
      ['CLAUDE_CLOUD_HOME_CHANNEL', 'claudeCloud.home(', 'policyUnmeteredRefusal('],
      ['CLAUDE_CLOUD_TERMINAL_CHANNEL', 'claudeCloud.continueInTerminal(', 'policyUnmeteredRefusal('],
      ['CLOUD_COMPUTER_SETUP_CHANNEL', 'cloudTurns.setUp(', 'policyFeatureRefusal('],
      ['CLOUD_COMPUTER_SIGN_IN_CHANNEL', 'cloudTurns.signIn(', 'policyFeatureRefusal(']
    ]
    for (const [channel, call, guard] of channels) {
      const handler = index.slice(index.indexOf(`ipcMain.handle(${channel}`))
      const launch = handler.indexOf(call!)
      expect(launch, channel).toBeGreaterThan(-1)
      expect(handler.slice(0, launch), channel).toContain(guard!)
      expect(handler.slice(0, launch), channel).toContain('if (managed !== undefined) return')
    }
  })

  it('checks voice, phone, MCP and CLI access in the main process, including saved opt-ins', () => {
    const voice = read('voice-call.ts')
    const start = between(voice, '    async start(', '    dispose()')
    expect(start.indexOf('await deps.policyRefusal(')).toBeLessThan(start.indexOf('deps.spawn('))
    expect(index).toContain("policyRefusal: (target, preview) => policyStartRefusal({ runtime: 'codex'")
    expect(index).toContain("refusal: () => policyFeatureRefusal(organizationPolicy, 'otherApps')")
    expect(index).toContain("refusal: () => policyFeatureRefusal(organizationPolicy, 'phone')")
    expect(index).toContain("name.startsWith('cli/') ? cliSessions.call(name, args) : mcpTools(name, args)")
    for (const file of ['locust-mcp-host.ts', 'phone-host.ts']) {
      const host = read(file)
      expect(host).toContain('if (options.refusal?.() !== undefined) return')
      expect(host).toContain('if (enabled && options.refusal?.() !== undefined) return')
    }
    const anti = between(read('antigravity-mission.ts'), '    async start(prompt, peer, route)', '        const host = await options.probe()')
    expect(anti).toContain('policyTurnRefusal(options.organizationPolicy')
    expect(anti).toContain('if (managed !== undefined) throw')
  })

  it('loads once at launch, keeps the test seam out of packaged builds and exposes errors in the window', () => {
    expect(index.match(/readOrganizationPolicy\(/g)).toHaveLength(1)
    expect(index).toContain('readOrganizationPolicy({ packaged: app.isPackaged, note })')
    expect(read('organization-policy.ts')).toContain('if (!options.packaged && env.LOCUST_POLICY_FILE !== undefined)')
    const app = read('../renderer/src/App.tsx')
    expect(app).toContain('organizationPolicy: info.organizationPolicy')
    expect(app).toContain('<OrganizationPolicyNotice />')
    expect(read('../renderer/src/components/Screens.tsx')).toContain('<OrganizationPolicySettings heading=')
  })
})
