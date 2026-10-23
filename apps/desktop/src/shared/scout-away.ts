/**
 * SCOUT WHILE YOUR COMPUTER IS OFF (0.829), as Settings shows it: off (and why it cannot be on, when it cannot),
 * waiting for the person to merge the pull request that adds the look to their repository, or on.
 */
export const SCOUT_AWAY_STATUS = 'scout:away-status'
export const SCOUT_AWAY_SET = 'scout:away-set'
export const SCOUT_AWAY_CHANGED = 'scout:away-changed'

export type ScoutAwayStatus =
  | {
    readonly state: 'off'
    /** What stops it being turned on now, in the person's words; none when it can be. */
    readonly blocked?: string
    /** What happened last: why it went off, or what went wrong turning it on. */
    readonly message?: string
  }
  | { readonly state: 'busy'; readonly doing: string }
  | {
    readonly state: 'waiting'
    readonly repo: string
    /** The pull request that adds the look, for the person to merge. */
    readonly pr: string
    readonly message?: string
  }
  | {
    readonly state: 'on'
    readonly repo: string
    /** Every how many hours it looks. */
    readonly every: 1 | 2
    readonly private: boolean
    /** When the last away look came, as an ISO time. */
    readonly lastLook?: string
    /** A newer away look than the repository's, for another pull request. */
    readonly outdated?: true
    readonly message?: string
  }

export interface ScoutAwayApi {
  /** `check`: ask GitHub whether the pull request was merged since. */
  status(check?: boolean): Promise<ScoutAwayStatus>
  set(on: boolean): Promise<ScoutAwayStatus>
}
