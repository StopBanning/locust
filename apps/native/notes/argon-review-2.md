I have completed the review of [`main.rs`](file:///Users/locust/Documents/Locust/native-review/main.rs) and its interaction with [`engine.rs`](file:///Users/locust/Documents/Locust/native-review/engine.rs) and the Rune framework's `Latest` implementation.

Here are the findings across the send/reply state machine, team reloading, escape/close paths, and potential panic sites.

---

### 1. The Send / Reply State Machine

#### 1.1 `Reply::WaitingForYou` tight-loop without exit or backoff
- **Lines:** [`main.rs#L77`](file:///Users/locust/Documents/Locust/native-review/main.rs#L77), [`main.rs#L221-L224`](file:///Users/locust/Documents/Locust/native-review/main.rs#L221-L224)
- **What goes wrong:**
  In [`locust-mcp-tools.ts#L415`](file:///Users/locust/Documents/Locust/native-review/locust-mcp-tools.ts#L415), when Locust is waiting for the user to approve or deny an action card in the Electron app (`options.waiting(mission.missionId) === true`), `read_reply` does not block for 20 seconds; it returns immediately (0 ms wait).
  In [`main.rs#L221-L224`](file:///Users/locust/Documents/Locust/native-review/main.rs#L221-L224), `Reply::WaitingForYou` sets the status and immediately invokes `request_reply(s, index)`. Although `waiting` is debounced by 500ms ([`main.rs#L77`](file:///Users/locust/Documents/Locust/native-review/main.rs#L77)), this produces an infinite, continuous 500ms polling loop against the local MCP door as long as the confirmation card remains unhandled in Locust. Native Locust provides no timeout, no backoff, and no user action (no button or Escape shortcut) to stop or abort the loop.
- **The fix:**
  Increase the poll delay or backoff when waiting for person approval (e.g., pace it to 2–3 seconds as done in [`ask_cli`](file:///Users/locust/Documents/Locust/native-review/main.rs#L709)), and provide a way for the user to cancel or break the loop from Native Locust (see Escape path below).

#### 1.2 Permanent flight deadlock when `conversation_id` is None
- **Lines:** [`main.rs#L141-L145`](file:///Users/locust/Documents/Locust/native-review/main.rs#L141-L145)
- **What goes wrong:**
  ```rust
  fn request_reply(s: &State, index: usize) {
      if let Some(id) = s.conversations[index].conversation_id.clone() {
          s.waiting.request(id, |id| Engine::local()?.reply(&id, 20));
      }
  }
  ```
  If `s.conversations[index].conversation_id` is `None` (for instance, if the team reloaded mid-flight and wiped `conversations`, or if `started` had not stored an id), `s.waiting.request` is never enqueued. Because no request is enqueued, neither `s.waiting.take()` nor `s.sending.take()` will ever produce a value again, leaving `s.flight` set to `Some(Flight)` indefinitely. As a result, `can_send` returns `false` forever, `load_team` exits early forever, and the UI becomes permanently wedged.
- **The fix:**
  If `conversation_id` is `None`, terminate the flight and set an error status:
  ```rust
  fn request_reply(s: &mut State, index: usize) {
      match s.conversations.get(index).and_then(|c| c.conversation_id.clone()) {
          Some(id) => s.waiting.request(id, |id| Engine::local()?.reply(&id, 20)),
          None => {
              set_status(s, index, "No active conversation id", RED);
              s.flight = None;
          }
      }
  }
  ```

#### 1.3 Sent text retained in composer if user edits during transit
- **Lines:** [`main.rs#L190-L192`](file:///Users/locust/Documents/Locust/native-review/main.rs#L190-L192), [`main.rs#L533-L546`](file:///Users/locust/Documents/Locust/native-review/main.rs#L533-L546)
- **What goes wrong:**
  While `button` is disabled during flight ([`main.rs#L557`](file:///Users/locust/Documents/Locust/native-review/main.rs#L557)), `composer` is not disabled. If the user types any keystroke while `sending` is in transit, `s.draft.text != message`. When `started` confirms acceptance, `if s.draft.text == message` evaluates to `false`, leaving `s.draft` uncleared. The user now has their previously sent message mingled with their new input.
- **The fix:**
  Clear `s.draft` immediately upon send (restoring it if `s.sending` returns `Err`), or disable the composer input while a flight is active.

---

### 2. Team Reloading Mid-Flight

#### 2.1 Conversations wiped, causing turn loss and reply drops
- **Lines:** [`main.rs#L153-L161`](file:///Users/locust/Documents/Locust/native-review/main.rs#L153-L161)
- **What goes wrong:**
  ```rust
  if let Some(result) = s.team.take() {
      changed = true;
      match result {
          Ok(team) => {
              s.conversations = (0..team.len())
                  .map(|_| Conversation::default())
                  .collect();
              s.teammates = team;
              s.selected = 0;
  ```
  Whenever `s.team.take()` yields a fresh teammate list, `s.conversations` is completely reallocated with blank defaults. If this occurs while a message is in flight:
  1. All prior turns (including the "You" turn just pushed) are destroyed.
  2. `conversation_id` is cleared to `None`, which triggers the permanent flight deadlock in `request_reply` described in 1.2, permanently dropping the agent's eventual answer.
- **The fix:**
  Preserve existing conversations across reloads by matching `teammate.id`, retaining their `conversation_id` and history instead of discarding them all.

#### 2.2 Vector index invalidation leading to panics or wrong teammate attribution
- **Lines:** [`main.rs#L41-L44`](file:///Users/locust/Documents/Locust/native-review/main.rs#L41-L44), [`main.rs#L181-L189`](file:///Users/locust/Documents/Locust/native-review/main.rs#L181-L189), [`main.rs#L210-L227`](file:///Users/locust/Documents/Locust/native-review/main.rs#L210-L227)
- **What goes wrong:**
  `Flight` stores `index: usize` ([`main.rs#L42`](file:///Users/locust/Documents/Locust/native-review/main.rs#L42)). If `s.teammates` is reloaded while a flight is active:
  - If the new team has fewer members (`new_team.len() <= flight.index`), `s.conversations[index]` and `s.teammates[index]` panic with `index out of bounds`.
  - If teammates were reordered or inserted/deleted, `flight.index` points to the wrong teammate. The agent's answer is attributed to the wrong teammate (`let who = s.teammates[index].name.clone()`) and appended to the wrong conversation (`s.conversations[index].turns.push(...)`).
- **The fix:**
  Change `Flight` to track `teammate_id: String` rather than a raw vector index. In `poll`, resolve the current index dynamically via `s.teammates.iter().position(|t| t.id == flight.teammate_id)`. If the teammate no longer exists in the updated team, cancel the flight cleanly.

---

### 3. Escape and Close Paths

#### 3.1 Window close wedges flight state permanently
- **Lines:** [`main.rs#L762-L766`](file:///Users/locust/Documents/Locust/native-review/main.rs#L762-L766)
- **What goes wrong:**
  ```rust
  .on_close(|s, _| {
      s.team.cancel();
      s.sending.cancel();
      s.waiting.cancel();
  })
  ```
  Calling `cancel()` on `Latest` ensures any in-flight background tasks will discard their results upon completion and will never wake the app or land in `take()`. However, `s.flight` is not cleared. Because Rune preserves `App` state across window closes on macOS and Windows, reopening the window finds `s.flight.is_some() == true`. Because the handles were cancelled, `take()` never fires, leaving the reopened window permanently stuck with a pulsing activity dot, a disabled send button, and blocked team reloading.
- **The fix:**
  Reset `s.flight = None;` in `on_close` (and set status back to `"Ready"`).

#### 3.2 Missing Escape handler
- **Lines:** [`main.rs#L746-L756`](file:///Users/locust/Documents/Locust/native-review/main.rs#L746-L756)
- **What goes wrong:**
  `AppSpec` registers no `on_key` callback. If a send or wait hangs, or enters the rapid `WaitingForYou` loop, pressing `Escape` does nothing. The user has no keyboard shortcut to abort an in-flight operation.
- **The fix:**
  Add an `on_key` handler to `AppSpec`:
  ```rust
  on_key: Some(|s, ev, tx| {
      if ev.key == rune::Key::Escape && s.flight.is_some() {
          s.sending.cancel();
          s.waiting.cancel();
          s.flight = None;
          s.status = "Cancelled".into();
          s.status_color = color(MUTED);
          return true;
      }
      false
  }),
  ```

---

### 4. Indexing Panic Sites

#### 4.1 Asymmetric indexing in `view`
- **Lines:** [`main.rs#L404`](file:///Users/locust/Documents/Locust/native-review/main.rs#L404), [`main.rs#L409`](file:///Users/locust/Documents/Locust/native-review/main.rs#L409), [`main.rs#L460`](file:///Users/locust/Documents/Locust/native-review/main.rs#L460)
- **What goes wrong:**
  Line 404 guards teammate access with `if let Some(teammate) = s.teammates.get(s.selected)`. But lines 409 and 460 directly index `s.conversations[s.selected]`. If `s.conversations.len() <= s.selected` (e.g. if the lists ever desynchronize), `view` panics.
- **The fix:**
  Use safe indexing: `if let Some(conversation) = s.conversations.get(s.selected)` before accessing `.turns`.

#### 4.2 Unchecked indexing in `set_status`
- **Line:** [`main.rs#L109`](file:///Users/locust/Documents/Locust/native-review/main.rs#L109)
- **What goes wrong:**
  ```rust
  s.status = format!("{}: {message}", s.teammates[index].name);
  ```
  `set_status` is called from `poll` on both success and error paths using `flight.index`. If `s.teammates.len() <= index` (after a team reload), `set_status` panics with index out of bounds.
- **The fix:**
  ```rust
  let name = s.teammates.get(index).map(|t| t.name.as_str()).unwrap_or("Locust");
  s.status = format!("{name}: {message}");
  ```

#### 4.3 Missing conversation bounds check in `can_send`
- **Lines:** [`main.rs#L99-L106`](file:///Users/locust/Documents/Locust/native-review/main.rs#L99-L106), [`main.rs#L120`](file:///Users/locust/Documents/Locust/native-review/main.rs#L120)
- **What goes wrong:**
  `can_send` verifies `s.selected < s.teammates.len()`, but does not verify `s.selected < s.conversations.len()`. Line 120 directly indexes `s.conversations[index]`. If `s.conversations` is shorter than `s.teammates`, `send` panics.
- **The fix:**
  Add `&& s.selected < s.conversations.len()` to `can_send`.

---

### Verification Summary
- **Verified:** Logic, state machine transitions, `Latest` cancellation and debounce semantics in [`crates/rune/src/latest.rs`](file:///Users/locust/work/scratch/rune-ref/crates/rune/src/latest.rs), and error handling across [`main.rs`](file:///Users/locust/Documents/Locust/native-review/main.rs) and [`engine.rs`](file:///Users/locust/Documents/Locust/native-review/engine.rs).
- **Unverified:** Runtime execution of the Rune GUI, as no cargo manifest exists in this workspace folder and modification was disallowed for this turn.

Second review findings for native Locust preview (main.rs):

1. Team reload mid-flight invalidates state:
In main.rs (lines 157-160), `s.team.take()` completely overwrites `s.conversations` with empty default conversations, wiping active turns and resetting conversation_id to None. Because `Flight` (lines 41-44) stores a raw `index: usize`, if the team length shrinks or ordering changes while sending/waiting, lines 185, 187, 200, 226, 227, and set_status (line 109) panic out of bounds or write turns to the wrong teammate.
Fix: Change `Flight` to store `teammate_id: String` and resolve its index dynamically. In `poll`, preserve existing conversations by teammate ID instead of reallocating empty defaults.

2. Permanent flight lock on missing conversation ID:
In `request_reply` (lines 141-145), if `s.conversations[index].conversation_id` is None, `waiting.request` is never enqueued and `s.flight` is never reset to None. The window remains permanently wedged in flight.
Fix: If `conversation_id` is None, clear `s.flight = None` and record an error status.

3. Close and Escape handling:
In `on_close` (lines 762-766), `s.team.cancel()`, `s.sending.cancel()`, and `s.waiting.cancel()` are called, but `s.flight = None` is missing. Reopening the window leaves `s.flight` permanently active because cancelled handles never land. Also, `AppSpec` has no `on_key` handler, so pressing Escape cannot cancel an in-flight send or abort a loop.
Fix: Set `s.flight = None;` in `on_close`. Add `on_key` to `AppSpec` to handle `rune::Key::Escape` by cancelling handles and clearing `s.flight`.

4. WaitingForYou polling loop:
In lines 221-224, `Reply::WaitingForYou` calls `request_reply` immediately with a 500ms debounce. In locust-mcp-tools.ts line 415, `read_reply` returns immediately when a card is waiting, causing a rapid 500ms polling loop with no backoff or escape mechanism.
Fix: Increase the interval/backoff for `WaitingForYou` and allow Escape cancellation.

5. Indexing panic guards:
In `view` (lines 409, 460), `s.conversations[s.selected]` uses direct indexing even though line 404 checked `s.teammates.get(s.selected)`. Guard with `s.conversations.get(s.selected)`. Similarly, in `set_status` (line 109), guard `s.teammates.get(index)`. In `can_send` (line 104), check `s.selected < s.conversations.len()`.
