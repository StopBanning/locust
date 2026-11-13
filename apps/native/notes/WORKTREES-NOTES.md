# Own-branch turns: a teammate's own copy of the project (12 Nov 2026)

Plan Part 3, "Port next" #1 (own-branch controls), on `exec/native-engine`. Built in three parts.

## What was built

1. **The copy** (`core/src/worktrees.rs`, from `main/worktrees.ts` and `shared/worktree-name.ts`):
   - the `locust/<name>` branch rule;
   - git's list, status and log readers;
   - a manager that makes a teammate's tree under `.locust/worktrees/<id>`. It is excluded from the person's checkout
     and idempotent, and a second teammate whose name folds to the same branch is kept apart;
   - a half-made tree is finished, never handed out;
   - a turn is committed on its branch as the teammate, past any hook. A new file over 50 MB is left out and named;
   - review of the branch, whole and turn by turn;
   - a copy is removed only when clean, or for exactly the changes agreed to. The branch always stays.
   - Every git call is bounded, and none runs a program the repository names to draw a diff.
2. **Turns in the copy** (`native::own_branch_folder`, `native::checkpoint_turn`):
   - A teammate set to `worktree` runs there. Antigravity is the exception, as in Electron.
   - When the copy cannot be made, the turn is refused: "X is set to work on its own branch, but ...".
   - Every turn is committed when it ends, with Electron's message:
     - the subject is the person's words, with a routine step's or a tag's host text cut off;
     - the body is the answer's first line;
     - the trailers name the teammate, the conversation, the route and the outcome.
   - The receipt goes in the stream as `host.turn_checkpoint`, and the thread shows it.
3. **The window:**
   - an **Own copy** switch in the teammate form, with Electron's hint naming the branch;
   - **OWN COPIES** on the project folder page (`src/ui/own_copies.rs`), with Electron's sentences. Each copy is
     listed with its branch and Remove. A copy with uncommitted work names it ("Codec's copy has 1 uncommitted change
     that is not on locust/codec: …") and offers Keep it or Delete the changes and remove. "In use" shows while a run
     is live there.

## How it was checked

- **19 tests from Electron's `worktrees.test.ts` and `a-turn-is-committed-on-its-own-branch.test.ts`,** against real
  git in temporary repositories. Dropping `--no-verify`, or the plain-diff flags, fails exactly the tests that guard
  them.
- **3 receipt tests** from `a-turn-on-its-own-branch-leaves-a-receipt.test.ts`: the subject rules, the route label,
  the sentences and what was left out.
- **End to end with the fake agent:**
  - the file is in the copy and not in the person's checkout;
  - the commit is on `locust/codec` with its trailers, and `main` is untouched;
  - the receipt shows in the thread;
  - Electron's reader accepts the ledger.
- **A teammate on its own branch in a folder that is not a repository** is refused in Electron's words, and nothing
  starts.
- **Window test:** the switch saves `worktree: true`; Remove names the change first; the copy goes only for it; the
  branch stays.
- **`scripts/check-m1-ledger.mjs`** now allows Locust's notes after a run's ending (a receipt, a carry-on), as
  Electron writes them.

## Left

- Review changes and landing the branch as one commit is its own parity row.
- The `.jsonl` ledgers of Electron comparisons and judges (slots of their own) are not run here.
