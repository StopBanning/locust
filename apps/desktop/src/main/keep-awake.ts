/**
 * THE COMPUTER STAYS AWAKE WHILE A TEAMMATE WORKS (0.379).
 *
 * A person starts a long job and walks away; Windows sleeps after its idle
 * timeout, and the run stops mid-step with nobody there to see why. Orca
 * holds the machine awake only while an agent is working (its
 * agent-awake-service, read 2026-09-26), and deliberately leaves the power
 * plan alone -- so does this.
 *
 * `prevent-app-suspension` keeps the SYSTEM awake and lets the display sleep:
 * nobody needs the screen lit to have a teammate keep working. Held while at
 * least one run is live, released the moment none is -- the host reads the
 * live count on a short beat, so a start or an end in any transport is seen
 * without a hook in each.
 *
 * The decision is pure; the Electron call is injected.
 *
 * PLUGGED IN, IT CAN STAY AWAKE FOR WHAT IS SCHEDULED (0.838). A routine at
 * 7:00 or Scout's next look never comes on a computer that slept at 1:00.
 * With Keep this computer awake while it is plugged in on, the host also
 * holds while the computer is on power, with nothing running. On battery it
 * holds only for live runs, as before. OpenBot has the same switch (read
 * 2026-10-27); the system may still sleep when someone asks it to.
 */

export interface PowerBlocker {
  /** Start holding the system awake; returns the hold's id. */
  readonly start: () => number
  readonly stop: (id: number) => void
}

export interface KeepAwake {
  /** How many runs are live now, and whether to hold with none (plugged in and asked to). Holds or releases to match. */
  update(liveRuns: number, standing?: boolean): void
  /** Whether the machine is being held awake. */
  readonly holding: boolean
  /** Release for good (the app is quitting). */
  dispose(): void
}

export function createKeepAwake(blocker: PowerBlocker): KeepAwake {
  let hold: number | undefined
  let disposed = false
  const release = (): void => {
    if (hold === undefined) return
    const id = hold
    hold = undefined
    try {
      blocker.stop(id)
    } catch {
      // Already gone: nothing is held either way.
    }
  }
  return {
    update(liveRuns, standing = false) {
      if (disposed) return
      const wanted = liveRuns > 0 || standing
      if (wanted && hold === undefined) {
        try {
          hold = blocker.start()
        } catch {
          // A machine that will not be held is a machine that may sleep;
          // the run itself is unaffected, and the next beat tries again.
          hold = undefined
        }
      } else if (!wanted) {
        release()
      }
    },
    get holding() {
      return hold !== undefined
    },
    dispose() {
      disposed = true
      release()
    }
  }
}

/** How often the host reads the live count: sleep timers are minutes, so seconds is plenty. */
export const KEEP_AWAKE_BEAT_MS = 5_000
