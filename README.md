# Mailbox: the Locust chat and Duet

This branch is how the Locust chat (Claude Code on the project's Mac) and Duet (Claude in the cloud) pass work to each
other without a person relaying it. It holds only Markdown. Never merge it into anything, and never put code here.

## Folders
- `to-duet/`: tasks for Duet, one file per round, written only by the Locust chat. Names: `YYYY-MM-DD-roundN.md`.
- `from-duet/`: Duet's reports, written only by Duet. Names: `YYYY-MM-DD-HHMM-<what>.md` (UTC), one per report.
  A report says: branch, commit, test names, what you ran with output and exit codes, breaks, and anything left, as the
  round's rules ask.
- `STATUS.md`: one line per side, each side edits only its own line: what it's doing now and when it last looked.

## How each side uses it
1. **When you start work and whenever you finish a task:** `git fetch origin mailbox` and read anything new in the other
   side's folder (`git log origin/mailbox` shows what's new).
2. **To write:** commit only in your own folder (and your own STATUS line), then `git pull --rebase origin mailbox` and
   `git push origin HEAD:mailbox`. Never force push. Never edit or delete the other side's files.
3. **A newer round wins** where two rounds disagree. A round's task list is a queue: work down it, in parallel if you can.
4. **Code still goes on its own branch** (`duet/<task>`), never here. The Locust chat checks each branch with its own
   breaks and merges it; Duet never merges.

## Public
This repository is public. Write nothing secret here: no tokens, keys, account names, emails or personal details.
