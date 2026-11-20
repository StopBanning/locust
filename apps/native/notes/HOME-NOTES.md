# Home notes

## Home round 1 (19 Nov 2026)

### Part 1 (78152a94): the pure rules, in core
- `core/src/home_cover.rs`: the cover's scale, growth into spare height, the short-Home check and the normal-spacing
  memory. Also `glass_status`, the status line that names who waits on you and who is working.
- `core/src/home_team_state.rs`: each teammate card's state, `last_worked_phrase` and `waiting_line`.
- These are tested in core but Home does not draw them yet. Their rows stay `waiting` until it does.

### Part 2: install first, and the line under the machine
- **Install first** (`with-nothing-installed-install-comes-first`, 4/4).
  - Rust Home had no agents panel. It now draws Settings' own "AI agents & accounts" panel, Install rows included.
  - The panel comes first while discovery says nothing is installed (`core/src/home_order.rs` `accounts_lead`), and
    after the teams and the folder once anything is.
  - An agent still being checked counts as installed.
  - Before discovery answers, Home keeps its usual order rather than jumping when the answer lands.
  - Home asks for discovery when it opens if nothing has answered yet.
  - Not Electron's: an answer naming no agents puts no empty panel first. Electron's `every` is true on an empty list.
- **The line under the machine** (`the-first-screen-line-is-centred`, 2/2).
  - Electron reads its stylesheet; here the drawn frame is measured, which is what Electron's probe on the built app
    does.
  - The line is centred on the lockup within 2px at 1215 wide, sits under it, shows for a first run only, and waits
    for the agents to answer.
  - Its second sentence is a promise about OpenCode's prices: `free_start_still_free` (Electron `freeStartStillFree`)
    withdraws it only when a ready OpenCode lists nothing free.
  - Rune has no per-line text centring. The box is centred, and the line fits one line in the 760 column; a window
    narrow enough to wrap it would centre the box, not each line.
- **A stale sentence fixed.** Settings' none-installed lede said "run its line below", although the row has had an
  Install button for some time. It says "press its Install" again, as Electron does.

### Part 3: the glass and the cards say what is happening
- **The cover's glass** (`homeCoverStatus`, 3/3).
  - While work runs, the glass says who waits on you, then who is working, in place of the claim (`glass_status`).
  - It is fed by the same `teammate_status` the cards below use, so the two never disagree.
- **The team cards** (`homeTeamState`, 10/10).
  - The state word sits beside the name in its tone: the live word, "waiting on you" in amber, or what blocks the
    teammate in red.
  - While they wait, the waiting line (the question as asked, "Wants to run …", or "Wants your approval: …") takes
    the role's place.
  - An idle card says nothing beside the name, and keeps "last worked …" for its hover.
  - The older shared test now expects Home's mid-line "sign-in needed"; Team still says "Sign-in needed".
  - Not drawn yet: the step under way, as the line under a working card.

### Not done
- Electron folds a ready agents list to one head line ("N agents ready"). Rust Home draws the whole panel, so a
  returning Home is taller than Electron's.
- Home drawing the cover rules (scale, growth, fitting one screen):
  - `the-home-screen-is-the-cover`, `home-fits-one-screen`, `home-fits-two-rows-of-teammates`;
  - `home-holds-still-when-tight-spacing-shrinks-it`, `the-cover-grows-into-spare-height`.
