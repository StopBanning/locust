# C3, first part: the shell on the kit (11 Nov 2026)

Electron's frames were captured with `_tools/polish-frames.mjs` (Home in both themes at 1100 x 780; its thread frame
did not open the conversation, so a screenshot Colin sent of Electron's thread stood in for it). Each Rust screen was
then redone on the kit and snapped at the same size.

## What changed

- **The message box** (`.lc-composer__box`): one rounded box on the chat fill with the text and a round send button
  (ivory with an arrow when there is something to send or queue, a square while a reply runs), and under it the chips:
  the mode (Electron's words: Ask, Edit, Plan, Approve, Auto), the folder, and the model with its agent's mark.
  Electron's +, microphone and voice buttons are not drawn: the port has no attach picker or voice yet, and a dead
  control costs more than a missing one (Electron's own rule).
- **The thread's head**: the teammate's square (an initial until C4's faces), name and role, the agent's mark and model,
  and the conversation's actions at the right, as Electron's header has them: Commit (an outlined button with its
  icon), Changes, Undo, Open in terminal, Catch up, Terminal and Browser as icons. The status line keeps only the live
  dot, the status, the engine and the context ring.
- **Your words** take `.lc-bubble` (the said size, 12 by 16, no border); **a reply's Copy** is CopyButton's icon, a
  check with "Copied" once it lands.
- **The approval card** uses the kit: Approve once is the primary button, the Always choices are ghost buttons, Deny and
  Never allow are deny buttons, as ApprovalCard.tsx draws them.
- **The sidebar**: `.lc-row` rows (9 by 10, radius 9, the row type), each teammate with its agent's mark; the places with
  their icons; "Recent" as a tracked mono label with a refresh icon; the search field as `.lc-search` with its
  magnifier; a conversation's actions behind a dots icon.
- **Home**: a centred column; the banner (the lockup and line on the boot screen's dark glass, dark in both themes as in
  Electron); the folder and away cards on the kit; Your team as a grid of cards (square, name, role, agent and model);
  the starters as StarterList's two columns, each with its icon.

## Checks

- Snapshots of Home, the thread, the approval card and the message box in both themes, compared by eye with Electron's
  frames and Colin's screenshot.
- Tests: 422 in the engine, 187 in the window, all passing.

## The rail and the title bar (12 Nov)

- `ui/rail.rs`. At 1199 points wide or less the sidebar folds to Electron's 64-point rail, unless Settings › Appearance ›
  Sidebar says Full or Rail. It is Electron's `layout` setting with the same values, but saved in the Rust app's own
  copy of the settings: changing it in one app does not change the other. Top to bottom:
  - the mark, which goes Home;
  - the `+`;
  - All conversations (the Board, since the port has no separate list screen), Teammates & Memory, Automations;
  - a face per teammate, with a count badge past one conversation and the lime bar on the selected one;
  - Settings and Board at the foot.
- Hovering a face opens that teammate's card beside it: their state and route, up to six conversations (live first,
  ages as `3d`), All of their conversations, and New conversation with them. Pressing the face pins the card; a press
  outside or Escape closes it, and an unpinned card closes 160 ms after the pointer leaves, as Electron's does.
- The width is read with `cx.layout_with` on the body, so the view re-runs only when the window crosses the line.
- The title bar sits above both layouts at 38 points, the traffic lights moved to Electron's 14,13. It shows the folder's
  name centred, or `Locust` and the build when there is none. While no conversation fills the window it also has the
  browser and terminal toggles, and `N running` with the pulsing dot.
- Scenes: `rail-flyout`, `rail-flyout-light`, `settings-sidebar`. Home at 1100x780 is the rail now, as Electron's is.

## After Fable's review (12 Nov)

- The wide sidebar has the way home and the `+` beside the brand, as Electron's does, and Settings and Board in a
  footer under a hairline. Until now the wide window had none of the four on screen.
- "Native team copy" under the brand is gone; the status line still names the engine.
- The rail remembers which face the pointer is over, so sliding from one face to the next keeps a card open whichever
  of leave and enter arrives first.
- Still open: the card's "All of Scout's conversations" opens everyone's Board, and the rail's All conversations and
  Board go to the same screen, because the port has no list of one teammate's conversations yet.

## Not done

- The glass behind the sidebar and title bar on a Mac (Electron's `vibrancy: 'sidebar'`).
- Faces and bots (C4); Home's connected-accounts and usage lines (no data for them in the port yet).
- Plan card, activity rows and live line still use the older palette; Settings, Board and the other screens are C5.
- No pixel score: the pairs are judged by eye, as the plan asks.
