# The first live hour (12 Nov 2026, 00:30–00:50)

This was the real window, built as `Locust Native.app`, with Electron quit. The Rust engine ran every turn. Colin said go.
Afterwards Electron was reopened, and Codec's mode was set back to Edit.

## Seen working

- **Sending from the real window.** A turn to Scout (OpenCode, the free DeepSeek route) streamed its answer. It ran in
  Electron's open folder, now-coding.
- **Follow-up.** "What two words did I ask you to say?" was answered "LIVE ONE", so the conversation carried on.
- **Stop.** On a long turn it ended the turn at once.
- **The door.** The switch was turned on in Settings › General ("Use each teammate's own mode" was already on), and it
  stayed on across a restart. `start_conversation` and `read_reply` worked through `--door-bridge`. The turn ran in
  the window's process in Codec's own mode (Approve each action), and `read_reply` said it was waiting on the person.
- **An approval.** The card showed on the Board under Needs you, in Electron's words. Allow once ran the command, and
  the turn completed.
- **The rail.** At 1040 wide the face card opened, and the count badges showed once the owner fix below landed.
- **Codex's sandbox.** In Edit, `curl` was refused for lack of network and `/tmp` was writable without a card. Electron
  sets the same thing (neither overrides Codex's default), so this is not a port difference.

## Found and fixed in this hour

- **The recent list named the agent, not the teammate.** Rows said "Codex" where Electron says "Codec". So the rail card
  said "Nothing yet" for every teammate, no count badge ever showed, and the wide sidebar's subtitles were wrong.
  `people::owner_names` now reads both owner maps once, the native file over Electron's.
- **A turn the door started never appeared** in the sidebar, the rail or the Board, because nothing re-read the list
  when a turn started outside the window's own send. Its card could not be found. The poll now re-reads the list when
  the set of running or waiting turns changes.
- **The release build was broken by `f76c8744`.** Spend helpers were still behind the test-only gate, which tests turn
  on and the release build does not, so every test passed. Fixed in `4602f6ce`. From now on, `cargo build --release`
  of the whole workspace runs before a commit.

## Found, not fixed yet

1. **Typing through accessibility does nothing.** Setting `AXSelectedText` on the message box reports success, but Rune's
   editor ignores it. Dictation apps such as Larynx may insert text this way. This is Rune's text area, so the fix
   belongs in Rune or a wrapper.
2. **A switch's accessibility "select" action does nothing.** A pointer click works.
3. **The window can stop drawing until real input arrives.** After background (accessibility) clicks and engine
   updates, the window kept showing an old frame while it was in front and uncovered. One real click drew the new
   state. Needs a look at how Rune schedules frames for changes that do not come from input.
4. **Quitting leaves turns marked as running.** Turns stopped by quitting the app had no cancelled record written. The
   title bar said "3 running", and the Board listed two dead turns under Working.
5. **The folder chip is wrong.** The message box said "No folder" while the turn ran in now-coding, Electron's open
   folder.
6. **A person's Stop is shown as an error.** It read "error: OpenCode stopped: Aborted" and then "Stopped". Electron
   shows only that it stopped.
7. **Pressing a face on the Board keeps the Board.** Electron opens that teammate's conversation.
8. **Board rows print raw state words**, such as "· needs-you" and "· working", after the title.

## Not reached

These need another session:
- carry-on to another agent;
- the model picker in the real window;
- the folder dialog and a reference folder;
- a notification on screen;
- Quick Ask.

Typing needs full-screen control until item 1 is fixed, so the next session should be planned around that.
