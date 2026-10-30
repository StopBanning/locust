// One debugging port per smoke, and no two the same.
//
// MEASURED 2026-09-11, and it is the root of most of the suite's flaky tail.
// Eleven port numbers were used by two smokes each, and the sweep runs them in
// alphabetical order, so several collided at close range:
//
//   schedule / steering      both 9299, TWO apart in run order
//   raw-conversation / relay both 9233, ONE apart
//   avatar / cursor          both 9227, TWO apart
//
// Those are four of the exact smokes `docs/FINDING-smoke-sweep-2026-09-11.md`
// recorded as flaky -- "roughly four of thirty-two are red on any given run,
// and it is not the same four."
//
// The sweep is sequential, so in principle nothing overlaps. In practice each
// smoke launches an ELECTRON app as its own child and the sweep waits only for
// the node process that spawned it. An app that is still shutting down still
// holds its debugging port, so the next smoke either cannot bind -- or, far
// worse, attaches its CDP client to the PREVIOUS app and drives a window that
// is on the wrong screen and about to disappear. That produces exactly the
// reported symptoms: "no row", "no composer", a screen nobody navigated to,
// none of it reproducible alone.
//
// Every app-launching smoke takes its port from here. A number appearing twice
// is a failure of this file, so `assertPortsAreUnique` exists and the sweep
// calls it before running anything.

/**
 * The port each smoke drives its app on.
 *
 * Explicit rather than derived from a directory listing: a person reading a
 * smoke should be able to see which port it takes without running anything,
 * and adding a smoke should not silently move everybody else's.
 *
 * The `_tools` drives use 9490-9599 and must stay out of this range.
 */
export const SMOKE_PORTS = Object.freeze({
  antigravity: 9240,
  avatar: 9241,
  copilot: 9242,
  cursor: 9243,
  diff: 9244,
  exchange: 9245,
  'folder-brief': 9246,
  'follow-up': 9247,
  handoff: 9248,
  'limit-restore': 9249,
  memory: 9250,
  'model-choice': 9251,
  opencode: 9252,
  packaged: 9253,
  picker: 9254,
  'raw-conversation': 9255,
  relay: 9256,
  renderer: 9257,
  retention: 9258,
  room: 9259,
  routine: 9260,
  'row-menu': 9261,
  schedule: 9262,
  'side-by-side': 9263,
  steering: 9264,
  update: 9265,
  workroom: 9266,
  workspace: 9267,
  worktree: 9268,
  hub: 9269,
  'login-item': 9270,
  // The Mac checks in ci.yml, one after the other on one runner (0.838): both took 9334 until then.
  'mac-first-hour': 9271,
  'mac-update': 9272,
  // Self-contained like the two above; ci.yml can still name a port with LOCUST_SMOKE_PORT (the first Mac run: 9333
  // was another app's on that Mac, and the Stop check drove its window).
  'posix-stop': 9273,
  'linux-update': 9274
})

/**
 * The port for the smoke at `import.meta.url`.
 *
 * Taken from the FILENAME rather than passed in, because a smoke naming
 * somebody else's port is the bug this file exists to stop.
 */
export function portFor(url) {
  const file = url.split('/').at(-1) ?? ''
  const name = file.replace(/-smoke\.mjs$/, '')
  const port = SMOKE_PORTS[name]
  if (port === undefined) {
    throw new Error(`No smoke port for "${name}". Add one to _smoke/ports.mjs; do not borrow another smoke's.`)
  }
  return port
}

/** Every port distinct. Called by the sweep before it runs anything. */
export function assertPortsAreUnique() {
  const seen = new Map()
  for (const [name, port] of Object.entries(SMOKE_PORTS)) {
    const other = seen.get(port)
    if (other !== undefined) {
      throw new Error(`_smoke/ports.mjs gives ${other} and ${name} the same port (${String(port)}).`)
    }
    seen.set(port, name)
  }
}

/**
 * Wait until nothing is listening on `port`.
 *
 * The other half of the fix, and the half that survives a smoke leaving an
 * app behind: unique ports stop two smokes colliding by design, and this
 * stops a smoke colliding with ITSELF on a re-run, or with an app a previous
 * failure orphaned.
 */
export async function waitForPortFree(port, timeoutMs = 20_000) {
  const { createConnection } = await import('node:net')
  const inUse = () =>
    new Promise((resolve) => {
      const socket = createConnection({ port, host: '127.0.0.1' })
      const done = (answer) => {
        socket.destroy()
        resolve(answer)
      }
      socket.once('connect', () => done(true))
      socket.once('error', () => done(false))
      setTimeout(() => done(false), 500)
    })
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    if (!(await inUse())) return true
    await new Promise((resolve) => setTimeout(resolve, 250))
  }
  return false
}
