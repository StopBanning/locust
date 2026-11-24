# Duet: round 9, rows 679 and 435 (24 Nov 2026)

**Short version.** Both rows are carried as you decided. Branch `duet/diagram-and-palette`, commit `6467405e`, on `exec/native-engine` 97c8c451. The breaks were caught. The full window run fails only the 6 known Linux tests. Two visible changes: an open Mermaid fence now shows as code until it closes, and the palette head has a focus-lit hairline. B batch 2 and C batch 2 are still in progress, and they now know round 9's file list.

## 679 a-diagram-in-a-reply-shows-as-one: all 3 cases
- Tests: `src/diagram_shows_as_one_tests.rs`, one per case, drawn headless through `reply_in_thread` (the call messages.rs makes for every reply), with `live` set as while a turn arrives:
  - `is_framed_as_a_diagram_once_its_block_has_closed_its_text_a_tab_away`
  - `is_text_while_it_is_still_being_written`
  - `leaves_every_other_block_as_the_code_it_was`
- Change: in `diagram::prepare_reply_markdown`, an open fence gets no language. So a fence still arriving, or a reply that ended with it unclosed, shows as a code block with its text, and it's drawn as a diagram once the fence closes. The dead "Drawing the diagram…" branch in `reply_actions.rs` was removed.
- Doesn't apply: Electron's "Drawing the diagram…" while its web view draws. rune-mermaid draws in the frame the fence closes, so there's no wait to show. The ledger note says so.
- Break (open fence given `mermaid` again): `is_text_while_it_is_still_being_written` failed, "a diagram was drawn while the fence was open".

## 435 palette-has-no-inner-ring: both cases
- main.rs: one line changed, the palette call, which is now `command_palette(s, &p, s.palette_query.focused)`. Nothing else in main.rs changed.
- `command_palette` takes `focused`. The head gets a 1px bottom edge: `border-hairline` at rest, `border-hover` while the search field is focused. In test builds the draw records the edge and the field's ring in `PALETTE_DRAWN`, using the same pattern as `context_menu.rs`'s `SUB_EDGE`. The field sets no ring.
- Tests: `src/ui/palette_has_no_inner_ring_tests.rs`: open with Ctrl K through `on_key`, draw through `crate::view`:
  - `turns_the_global_focus_ring_off_at_a_specificity_that_beats_it`
  - `keeps_the_heads_lit_edge_as_the_focus_it_shows`
  Expected colours are the look tokens Electron's CSS names.
- Break (edge never lit): `keeps_the_heads_lit_edge_as_the_focus_it_shows` failed (left alpha 0.0588, right 0.1608).

## Runs (Linux, shared target)
- `diagram::` and palette tests: 12 passed, exit 0.
- Full `cargo test -p locust-native --bin locust-native`: 1847 passed, 6 failed, exit 101. The 6 are the known Linux window_drive_tests.
- Ledger: both rows `carried`, with 5 cases.tsv cells flipped. case-map --write, recount, and `--check` on recount, marks and test-ledger all exit 0.

## Not run
On a Mac. No test is gated to macOS.
