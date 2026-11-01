import { SETTINGS_A_BROWSER_MAY_CHANGE, type WorkspaceSettings } from '../shared/ipc.js'

/**
 * What a settings change may set (0.849): only the settings that grant nothing, whatever else it names. A browser's
 * change that names Auto, what teammates may reach, swarm or relay sets none of them; the computer keeps its own.
 */
export function settingsChangeKept(change: unknown): Partial<WorkspaceSettings> {
  if (typeof change !== 'object' || change === null || Array.isArray(change)) return {}
  const allowed: readonly string[] = SETTINGS_A_BROWSER_MAY_CHANGE
  return Object.fromEntries(Object.entries(change).filter(([key]) => allowed.includes(key))) as Partial<WorkspaceSettings>
}
