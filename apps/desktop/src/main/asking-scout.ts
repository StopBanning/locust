import type { MissionStarter } from '@teammate/mission-store'

/**
 * WHETHER A TURN IS A QUESTION TO SCOUT (0.828; narrowed 0.872).
 *
 * A turn with Scout's teammate is answered from What Scout saw and sealed as Scout's looks are -- Ask, no tool, no file,
 * no command -- because that log quotes other people's words. That is right for the person's own turn from the window,
 * and for a voice call, where the person is asking what Scout saw.
 *
 * It is NOT right for a turn another app started through Locust's MCP door with its own work: the app's message is the
 * whole prompt, no Scout log is put in, and the app may have named the mode and folder within the person's Settings
 * switch. Sealing it ran Colin's review handed to Scout (an OpenCode teammate) in Ask with every tool refused, while
 * the door replied it ran in Auto in the named folder (9 Nov 2026). Such a turn runs as any teammate's does.
 */
export function asksScout(teammateId: unknown, scoutTeammateId: string | undefined, startedBy?: MissionStarter): boolean {
  if (typeof teammateId !== 'string' || scoutTeammateId === undefined || teammateId !== scoutTeammateId) return false
  return !(startedBy?.kind === 'mcp' && startedBy.via !== 'voice')
}
