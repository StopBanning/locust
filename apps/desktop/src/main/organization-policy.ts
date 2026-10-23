import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { isAbsolute, win32 } from 'node:path'
import { isMonthlyLimit } from '../shared/spend.js'
import { POLICY_MODE_ORDER, POLICY_RUNTIME_IDS, type OrganizationPolicy, type OrganizationPolicyState, type PolicyRuntimeId } from '../shared/organization-policy.js'

const KEYS = new Set(['version', 'allowedAgents', 'allowedModels', 'highestMode', 'monthlyLimitUsd', 'otherApps', 'phone', 'chatApps', 'cloud', 'terminal', 'browser', 'accounts'])
const object = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
const runtime = (value: unknown): value is PolicyRuntimeId => typeof value === 'string' && (POLICY_RUNTIME_IDS as readonly string[]).includes(value)

export class OrganizationPolicyError extends Error {}

/**
 * A tool from Windows' own folder: C:\Windows when it is there, so a SystemRoot handed to this process cannot swap
 * in a look-alike; the variable only where Windows lives on another drive.
 */
const windowsTool = (...parts: readonly string[]): string => {
  const fixed = win32.join('C:\\Windows', 'System32', ...parts)
  return existsSync(fixed) ? fixed : win32.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', ...parts)
}

/**
 * Resolve the OS known folder, rather than trusting a per-process ProgramData override. Read from the machine's
 * registry, which only an administrator can change, in about 30 ms; PowerShell's answer, about 200 ms on every
 * launch, is the fallback.
 */
export function windowsCommonData(): string {
  const settle = (found: string | undefined): string => {
    if (found === undefined || !win32.isAbsolute(found) || found.startsWith('\\\\')) throw new Error('Windows did not identify its machine-wide application data folder.')
    return found
  }
  try {
    const said = execFileSync(windowsTool('reg.exe'), ['query', 'HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Shell Folders', '/v', 'Common AppData'],
      { encoding: 'utf8', windowsHide: true, timeout: 5_000, maxBuffer: 4096 })
    return settle(/Common AppData\s+REG_SZ\s+(.+?)\s*$/m.exec(said)?.[1])
  } catch {
    return settle(execFileSync(windowsTool('WindowsPowerShell', 'v1.0', 'powershell.exe'),
      ['-NoProfile', '-NonInteractive', '-Command', "[Environment]::GetFolderPath('CommonApplicationData')"],
      { encoding: 'utf8', windowsHide: true, timeout: 5_000, maxBuffer: 4096 }).trim())
  }
}

export function organizationPolicyPath(options: { readonly packaged: boolean; readonly platform?: NodeJS.Platform; readonly env?: NodeJS.ProcessEnv; readonly commonData?: () => string }): string {
  const env = options.env ?? process.env
  // The development test seam is never consulted by a packaged app.
  if (!options.packaged && env.LOCUST_POLICY_FILE !== undefined) {
    if (!isAbsolute(env.LOCUST_POLICY_FILE)) throw new Error('LOCUST_POLICY_FILE must name an absolute path')
    return env.LOCUST_POLICY_FILE
  }
  switch (options.platform ?? process.platform) {
    case 'win32': return win32.join((options.commonData ?? windowsCommonData)(), 'Locust', 'policy.json')
    case 'darwin': return '/Library/Application Support/Locust/policy.json'
    default: return '/etc/locust/policy.json'
  }
}

/** Every known key is validated. Empty allowlists intentionally deny everything. */
export function parseOrganizationPolicy(value: unknown, unknownKey: (key: string) => void = () => undefined): OrganizationPolicy {
  if (!object(value)) throw new Error('The policy must be a JSON object.')
  for (const key of Object.keys(value)) if (!KEYS.has(key)) unknownKey(key)
  const bad = (key: string): never => { throw new Error(`The ${key} setting is invalid.`) }
  if (value.version !== undefined && value.version !== 1) bad('version')
  if (value.allowedAgents !== undefined && (!Array.isArray(value.allowedAgents) || !value.allowedAgents.every(runtime))) bad('allowedAgents')
  const models: Partial<Record<PolicyRuntimeId, readonly string[]>> = {}
  if (value.allowedModels !== undefined) {
    if (!object(value.allowedModels)) bad('allowedModels')
    for (const [id, patterns] of Object.entries(value.allowedModels as Record<string, unknown>)) {
      if (!runtime(id) || !Array.isArray(patterns) || !patterns.every((pattern): pattern is string => typeof pattern === 'string' && pattern.length > 0 && pattern.length <= 512 && !/[\r\n\0]/.test(pattern))) bad('allowedModels')
      models[id as PolicyRuntimeId] = patterns as string[]
    }
  }
  if (value.highestMode !== undefined && !(POLICY_MODE_ORDER as readonly unknown[]).includes(value.highestMode)) bad('highestMode')
  if (value.monthlyLimitUsd !== undefined && value.monthlyLimitUsd !== 0 && !isMonthlyLimit(value.monthlyLimitUsd)) bad('monthlyLimitUsd')
  for (const key of ['otherApps', 'phone', 'chatApps', 'cloud', 'terminal', 'browser', 'accounts']) if (value[key] !== undefined && typeof value[key] !== 'boolean') bad(key)
  return {
    ...(value.version === undefined ? {} : { version: 1 }),
    ...(value.allowedAgents === undefined ? {} : { allowedAgents: value.allowedAgents as PolicyRuntimeId[] }),
    ...(value.allowedModels === undefined ? {} : { allowedModels: models }),
    ...(value.highestMode === undefined ? {} : { highestMode: value.highestMode as OrganizationPolicy['highestMode'] }),
    ...(value.monthlyLimitUsd === undefined ? {} : { monthlyLimitUsd: value.monthlyLimitUsd as number }),
    ...Object.fromEntries(['otherApps', 'phone', 'chatApps', 'cloud', 'terminal', 'browser', 'accounts'].filter((key) => value[key] !== undefined).map((key) => [key, value[key]]))
  }
}

/** Read once, without ever creating, replacing or writing the administrator's file. */
export function readOrganizationPolicy(options: {
  readonly packaged: boolean
  readonly platform?: NodeJS.Platform
  readonly env?: NodeJS.ProcessEnv
  readonly commonData?: () => string
  readonly read?: (path: string) => string
  readonly note?: (label: string, detail: string) => void
}): OrganizationPolicyState {
  let path = !options.packaged && options.env?.LOCUST_POLICY_FILE !== undefined ? options.env.LOCUST_POLICY_FILE : 'the machine-wide Locust/policy.json file'
  try {
    path = organizationPolicyPath(options)
    let contents: string
    try { contents = (options.read ?? ((file) => readFileSync(file, 'utf8')))(path) }
    catch (cause) { if ((cause as NodeJS.ErrnoException).code === 'ENOENT') return { path, present: false, policy: {} }; throw new Error('The file could not be read.') }
    if (Buffer.byteLength(contents, 'utf8') > 1024 * 1024) throw new Error('The file exceeds 1 MB.')
    let value: unknown
    try { value = JSON.parse(contents.replace(/^\uFEFF/, '')) } catch { throw new Error('The file is not valid JSON.') }
    const policy = parseOrganizationPolicy(value, (key) => options.note?.('organization-policy', `Unknown setting ${JSON.stringify(key)} in ${path} was skipped.`))
    return { path, present: true, policy }
  } catch (cause) {
    const error = `Locust cannot use the organization policy at ${path}. ${cause instanceof Error ? cause.message : 'The file is invalid.'} No turns or connections will start until your administrator fixes it and Locust restarts.`
    options.note?.('organization-policy', error)
    return { path, present: true, policy: {}, error }
  }
}
