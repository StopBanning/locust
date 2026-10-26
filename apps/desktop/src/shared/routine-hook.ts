/** CALLED LINKS (2026-10-25). Only this deliberate editor call reveals a link; routine lists never carry passwords. */
export const ROUTINE_HOOK_READ = 'routine:hook-read'
export const ROUTINE_HOOK_NEW = 'routine:hook-new'
export interface RoutineHookState { readonly url?: string; readonly message?: string; readonly canMake?: true }
export type RoutineHookResponse = { readonly ok: true; readonly data: RoutineHookState } | { readonly ok: false; readonly error: { readonly code: 'ROUTINE_REJECTED'; readonly message: string } }
export const HOOK_WARNING = 'This link is a password. Anyone who has it can start this routine. What they send passes through Locust’s relay readable, unlike sealed phone traffic. It is kept there until your PC collects it, at most a week. Do not send secrets.'
export const HOOK_ASK = 'A called link always runs in Ask: it reads and changes nothing. Calls during a run wait and become one more run.'
