<!--
Draft What's new for the next Electron build, written from the subjects of the 8 commits after 0.881.0 (local on
Colin's Mac, not on GitHub when this was drafted). Not CHANGELOG.md: move the entry to the top of CHANGELOG.md, under
its preamble, when the build is cut, and put the date in its heading. Lines marked CHECK name a detail the commit
subjects do not give; confirm or cut it against the commit before moving the entry.
-->

## 0.882.0 - (date when cut)

### New

- **An app knows which computer it is talking to.** When Locust runs on two computers and an app is connected to both,
  the app is now told which computer each one is, so it can tell them apart.
  <!-- CHECK: how the computer is named to the app (its name as Settings shows it?). -->

### Improved

- **The model chip names the account only when you turn it on.** The chip shows just the model and its effort; the
  account it runs on is named only if you ask for it. <!-- CHECK: where it is turned on (the setting's name). -->
- **A usage card's windows read shorter.** An agent's usage card now says "5-hour" and "Weekly" over each bar, not
  "5-hour window" and "Weekly window".
- **Every time of day is 12-hour.** Times on screen, such as when a usage window resets, all read like 5:34 PM.
- **Removing an account asks right under it.** The question to remove an account now opens under that account, not
  at the foot of the panel, so you can see which one you are removing.
- **Apple Intelligence wears Apple's logo.** Its mark is now Apple's, like every other agent's is its maker's.

### Fixed

- **A long word stays inside its bubble.** A long word, link or path in a message you sent now breaks onto the next
  line inside the bubble instead of running past it. <!-- CHECK: links and paths, or words only. -->
- **The conversation source picker lines up.** It now sits right under the search, with no grey slab behind it.
