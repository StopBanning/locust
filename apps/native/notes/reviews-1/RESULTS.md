# reviews-1

Branch: exec/native-reviews-1, based on eac7cc69. The stopped Claude lane's uncommitted implementation was reviewed and retained, with its missing API/module connections completed.

The review brief has five named Electron cases plus three supporting tests. It carries the current request, reply, changed paths and commands, with earlier work labelled separately. This row remains partial: there is no Rust review action, reviewer picker, or integration assembling ReviewMaterial from the conversation. The pure builder is available, but a person cannot yet launch a review from the window.

The finished-conversation routine entrance uses the header's actions menu, matching current Electron App.tsx, and opens the existing routine editor. The empty shelf already offered finished conversations; six separate tests now cover its filter, order, cap, unreadable dates, empty result and turn wording. Five header window cases cover eligibility and the save action, including pending receipts.

The hand-out card lists each posted ask under its own turn, keeps peer exchanges, reads delivery from received links, shows the receiving turn's state, and opens that turn. Live state takes priority over a cached completed summary. Undelivered asks stay Starting without Open.

The live word uses the CSS sweep's four-width image, triangular highlight and two-second pass, sampled per character because Rune has no text clipping. The base word stays painted, reduced motion drops the highlight, and the label is 14px UI text at weight 500. No orb is scaled.

All 48 breaks were caught by test assertions (review brief 10, routine 15, hand-outs 15, live word 8). The hand-out placement break originally survived. The strengthened window test requires the card to follow its own reply and precede the next turn, and caught the rerun. All break targets were restored and checked.

Inspected the `reviews-save`, `reviews-handed`, and `reviews-live` headless snapshots. The focused run passed 19 core and 26 window tests. Workspace nextest passed 6,269 tests with 22 skipped. No real agent or packaged app was run.
