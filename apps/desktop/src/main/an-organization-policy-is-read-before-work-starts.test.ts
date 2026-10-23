import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, win32 } from 'node:path'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { organizationPolicyPath, parseOrganizationPolicy, readOrganizationPolicy, windowsCommonData } from './organization-policy.js'
import { POLICY_MODE_ORDER, POLICY_RUNTIME_IDS, policyCloudTerminalRefusal, policyFeatureRefusal, policyModeAllowed, policyModelAllowed, policyModels, policyMonthlyLimit, policyTurnRefusal, policyUnmeteredRefusal, type OrganizationPolicy, type OrganizationPolicyState } from '../shared/organization-policy.js'
import type { PublicModel } from '../shared/ipc.js'

const roots: string[] = []
afterEach(async () => { vi.unstubAllEnvs(); for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true }) })
const state = (policy: OrganizationPolicy = {}): OrganizationPolicyState => ({ path: '/admin/Locust/policy.json', present: true, policy })
async function fixture(contents?: string) {
  const root = await mkdtemp(join(tmpdir(), 'locust-policy-'))
  roots.push(root)
  const path = join(root, 'policy.json')
  if (contents !== undefined) await writeFile(path, contents)
  const note = vi.fn()
  const options = { packaged: false, env: { LOCUST_POLICY_FILE: path }, note }
  return { path, note, read: () => readOrganizationPolicy(options) }
}

describe('the machine policy file', () => {
  it('uses the machine-wide path on each platform and ignores the test override in a packaged app', () => {
    const env = { LOCUST_POLICY_FILE: '/person/escape.json', ProgramData: 'C:\\person\\escape' }
    expect(organizationPolicyPath({ packaged: true, platform: 'win32', env, commonData: () => 'D:\\ProgramData' })).toBe('D:\\ProgramData\\Locust\\policy.json')
    expect(organizationPolicyPath({ packaged: true, platform: 'darwin', env })).toBe('/Library/Application Support/Locust/policy.json')
    expect(organizationPolicyPath({ packaged: true, platform: 'linux', env })).toBe('/etc/locust/policy.json')
    expect(organizationPolicyPath({ packaged: true, platform: 'win32', env: {}, commonData: () => 'C:\\ProgramData' })).toBe('C:\\ProgramData\\Locust\\policy.json')
  })

  it('leaves a missing file unrestricted and never creates it', async () => {
    const f = await fixture()
    expect(f.read()).toEqual({ path: f.path, present: false, policy: {} })
    await expect(readFile(f.path)).rejects.toMatchObject({ code: 'ENOENT' })
    expect(policyTurnRefusal(f.read(), { runtime: 'codex', mode: 'auto' })).toBeUndefined()
  })

  it('uses the Windows known folder even if the process changes ProgramData', () => {
    if (process.platform !== 'win32') { expect(typeof windowsCommonData).toBe('function'); return }
    const actual = windowsCommonData()
    vi.stubEnv('ProgramData', 'C:\\person\\escape')
    expect(organizationPolicyPath({ packaged: true })).toBe(win32.join(actual, 'Locust', 'policy.json'))
  })

  it('reads a development override once without changing its bytes', async () => {
    const contents = '{ "version": 1, "highestMode": "plan", "otherApps": false }\n'
    const f = await fixture(contents)
    const loaded = f.read()
    await writeFile(f.path, '{}')
    expect(loaded.policy).toEqual({ version: 1, highestMode: 'plan', otherApps: false })
    expect(f.note).not.toHaveBeenCalled()
    await writeFile(f.path, contents)
    f.read()
    expect(await readFile(f.path, 'utf8')).toBe(contents)
  })

  it('accepts a UTF-8 byte order mark from Windows deployment tools', async () => {
    const f = await fixture('\uFEFF{"phone":false}')
    expect(f.read().policy).toEqual({ phone: false })
    expect(f.read().error).toBeUndefined()
  })

  it('skips an unknown key with one log line', async () => {
    const f = await fixture('{"futureKey":true,"phone":false}')
    expect(f.read().policy).toEqual({ phone: false })
    expect(f.note).toHaveBeenCalledExactlyOnceWith('organization-policy', `Unknown setting "futureKey" in ${f.path} was skipped.`)
  })

  it('fails closed for broken JSON and names its file in every refusal', async () => {
    const f = await fixture('{')
    const loaded = f.read()
    expect(loaded.error).toContain(f.path)
    expect(loaded.error).toContain('not valid JSON')
    expect(policyTurnRefusal(loaded, { runtime: 'codex', mode: 'ask' })).toBe(loaded.error)
    for (const key of ['phone', 'otherApps', 'cloud'] as const) expect(policyFeatureRefusal(loaded, key)).toBe(loaded.error)
    expect(policyModels(loaded, [{ id: 'a', runtime: 'codex', displayName: 'A', description: '', supportedEfforts: [] }])).toEqual([])
  })

  it('fails closed on an unreadable file and an invalid override', () => {
    const loaded = readOrganizationPolicy({ packaged: true, platform: 'linux', read: () => { throw Object.assign(new Error('denied'), { code: 'EACCES' }) } })
    expect(loaded.error).toContain('/etc/locust/policy.json')
    expect(loaded.error).toContain('could not be read')
    const invalid = readOrganizationPolicy({ packaged: false, env: { LOCUST_POLICY_FILE: 'relative.json' } })
    expect(invalid.error).toContain('absolute path')
  })

  it('reports an invalid known setting and an oversized file as blocking errors', async () => {
    const invalid = await fixture('{"phone":"false"}')
    expect(invalid.read().error).toContain('phone setting is invalid')
    expect(invalid.read().error).toContain(invalid.path)
    const large = await fixture(' '.repeat(1024 * 1024 + 1))
    expect(large.read().error).toContain('exceeds 1 MB')
  })

  it('validates every optional setting and rejects a malformed known key', () => {
    const policy: OrganizationPolicy = { version: 1, allowedAgents: [...POLICY_RUNTIME_IDS], allowedModels: { acp: ['model-*'], codex: ['*'] }, highestMode: 'approve-each', monthlyLimitUsd: 12.5, otherApps: false, phone: true, chatApps: false, cloud: false, terminal: true, browser: false, accounts: true }
    // Every known key is known: none of them is logged as skipped.
    const unknown: string[] = []
    expect(parseOrganizationPolicy(policy, (key) => { unknown.push(key) })).toEqual(policy)
    expect(unknown).toEqual([])
    expect(parseOrganizationPolicy({})).toEqual({})
    expect(parseOrganizationPolicy({ monthlyLimitUsd: 0 })).toEqual({ monthlyLimitUsd: 0 })
    for (const bad of [null, [], 1, { version: 2 }, { allowedAgents: ['typo'] }, { allowedAgents: 'codex' }, { allowedModels: [] }, { allowedModels: { typo: ['*'] } }, { allowedModels: { codex: '*' } }, { allowedModels: { codex: [''] } }, { highestMode: 'edit' }, { monthlyLimitUsd: -1 }, { monthlyLimitUsd: '5' }, { monthlyLimitUsd: Number.NaN }, { otherApps: 1 }, { phone: null }, { chatApps: 'no' }, { cloud: 'false' }]) {
      expect(() => parseOrganizationPolicy(bad), JSON.stringify(bad)).toThrow()
    }
  })
})

describe('a policy only narrows a turn', () => {
  it('enforces every mode ceiling in the documented authority order', () => {
    expect(POLICY_MODE_ORDER).toEqual(['ask', 'plan', 'approve-each', 'accept-edits', 'auto'])
    for (const [ceiling, highestMode] of POLICY_MODE_ORDER.entries()) {
      for (const [rank, mode] of POLICY_MODE_ORDER.entries()) {
        expect(policyModeAllowed(state({ highestMode }), mode), `${highestMode}: ${mode}`).toBe(rank <= ceiling)
      }
    }
  })

  it('allows only listed agents, including all own ACP agents under one runtime id', () => {
    for (const runtime of POLICY_RUNTIME_IDS) expect(policyTurnRefusal(state({ allowedAgents: ['acp'] }), { runtime, model: 'acp_goose::m', mode: 'approve-each' }) === undefined).toBe(runtime === 'acp')
    expect(policyTurnRefusal(state({ allowedAgents: [] }), { runtime: 'codex', mode: 'ask' })).toContain('allowedAgents')
  })

  it('matches only allowed model ids with literal punctuation and star wildcards', () => {
    const policy = state({ allowedModels: { codex: ['gpt-6.*', 'exact'], claude: [], acp: ['safe-*'] } })
    expect(policyModelAllowed(policy, 'codex', 'gpt-6.1')).toBe(true)
    expect(policyModelAllowed(policy, 'codex', 'gpt-6X1')).toBe(false)
    expect(policyModelAllowed(policy, 'codex', 'exact')).toBe(true)
    expect(policyModelAllowed(policy, 'claude', 'sonnet')).toBe(false)
    expect(policyModelAllowed(policy, 'muse', 'any')).toBe(true)
    expect(policyModelAllowed(policy, 'acp', 'acp_goose::safe-a')).toBe(true)
    expect(policyModelAllowed(policy, 'acp', 'acp_goose::other')).toBe(false)
  })

  it('cannot use an unresolved default to escape a model allowlist', () => {
    const policy = state({ allowedModels: { codex: ['account-default', 'account-*', 'gpt-*'], acp: ['safe-*'] } })
    expect(policyModelAllowed(policy, 'codex', undefined)).toBe(false)
    expect(policyModelAllowed(policy, 'codex', 'account-default')).toBe(false)
    expect(policyModelAllowed(policy, 'acp', 'acp_goose')).toBe(false)
    expect(policyModelAllowed(state({ allowedModels: { codex: ['*'] } }), 'codex', undefined)).toBe(true)
  })

  it('filters model families and their blocked effort variants', () => {
    const models: PublicModel[] = [{ runtime: 'cursor', id: 'family', displayName: 'Family', description: '', supportedEfforts: ['low', 'high'], defaultEffort: 'high', variants: { low: 'family-low', high: 'family-high' } }]
    const filtered = policyModels(state({ allowedModels: { cursor: ['family-low'] } }), models)
    expect(filtered).toMatchObject([{ id: 'family-low', variants: { low: 'family-low' }, supportedEfforts: ['low'], defaultEffort: 'low' }])
    expect(policyModels(state({ allowedAgents: [] }), models)).toEqual([])
    expect(policyModels(undefined, models)).toEqual(models)
  })

  it('keeps each connection off only when the organization disables it', () => {
    for (const key of ['phone', 'chatApps', 'otherApps', 'cloud'] as const) {
      expect(policyFeatureRefusal(state({ [key]: false }), key)).toContain(key)
      expect(policyFeatureRefusal(state({ [key]: true }), key)).toBeUndefined()
      expect(policyFeatureRefusal(state(), key)).toBeUndefined()
    }
    expect(policyTurnRefusal(state({ cloud: false }), { runtime: 'codex', mode: 'ask', cloud: true })).toContain('cloud')
    expect(policyTurnRefusal(state({ cloud: false }), { runtime: 'codex', mode: 'ask' })).toBeUndefined()
  })

  it('caps an absent or higher personal limit while keeping a lower one', () => {
    const policy = state({ monthlyLimitUsd: 10 })
    expect(policyMonthlyLimit(policy, undefined)).toBe(10)
    expect(policyMonthlyLimit(policy, 50)).toBe(10)
    expect(policyMonthlyLimit(policy, 5)).toBe(5)
    expect(policyMonthlyLimit(state({ monthlyLimitUsd: 0 }), 5)).toBe(0)
    expect(policyMonthlyLimit(state(), undefined)).toBeUndefined()
  })

  it('refuses an unmetered launch when the monthly cap cannot be enforced', () => {
    expect(policyUnmeteredRefusal(state({ monthlyLimitUsd: 5 }), { runtime: 'claude', mode: 'auto' })).toContain('monthlyLimitUsd')
    expect(policyUnmeteredRefusal(state(), { runtime: 'claude', mode: 'auto' })).toBeUndefined()
  })

  it('refuses an unrestricted cloud terminal when it cannot enforce the managed route', () => {
    expect(policyCloudTerminalRefusal(state({ allowedAgents: ['codex'] }))).toContain('allowedAgents')
    expect(policyCloudTerminalRefusal(state({ allowedModels: { claude: ['sonnet'] } }))).toContain('allowedModels')
    expect(policyCloudTerminalRefusal(state({ highestMode: 'plan' }))).toContain('highestMode')
    expect(policyCloudTerminalRefusal(state())).toBeUndefined()
  })
})
