# P14: Documents, diagrams and math (Area 15)

OpenCode researched crates then failed before writing Area 15 code. This copy started from that empty state (empty `~/work/scratch/p14-probe` stub only; no prior snaps or notes). Area 15 is advanced as far as the five requested ports go; ReplyPage was not in that order and stays missing. Recount: **29 done / 35 partial / 89 missing = 153**. `main.rs` changes are module + scene wiring only. Electron under `/Users/locust/work/locust/apps/desktop/src` was read only; Electron's data folder was not written. Native copies land under the native data folder (PDF page PNGs / text beside attachments; office words under `.locust/office-words/`).

## Sources

Read PLAN.md; PARITY.md Area 15; Electron `ReplyDiagram.tsx`, `shared/diagram.ts`, `main/diagrams.ts`, `main/diagram-view.ts`, `MathTex.tsx`, `agentText.ts` (math/dollar rules), `PdfPages.tsx`, `PdfTile.tsx`, `main/pdf-pages.ts`, `main/pdf-reading.ts`, `shared/attachments.ts` (`readablePdfNote`), `DocPreview.tsx`, `DocumentView.tsx`, `DocumentChange.tsx`, `SheetView.tsx`, `ZoomableImage.tsx`, `main/office-words.ts`, `main/office-text.ts`, `main/xlsx.ts`, `shared/office-document.ts`, `shared/file-menu.ts`, `main/reveal-file.ts`, `FileViewer.tsx`, and the `*.test.ts(x)` beside them. Rune: pinned `rune-kit` with `mermaid` → `rune-mermaid` at checkout `39301f6` (`~/.cargo/git/checkouts/rune-c9b805e9ed8a3730/39301f6/crates/rune-mermaid`). Crates.io: `ratex-render` 0.1.15 (+ layout/parser/types, `embed-fonts`), `calamine` 0.38, `zip` 2, `objc2-pdf-kit` 0.3.

## What landed

| Piece | Module | Crate / why |
| --- | --- | --- |
| Mermaid + Diagram/Code tabs | `src/diagram.rs` + reply chrome in `ui/reply_actions.rs` | `rune-kit` feature `mermaid` / `rune-mermaid` — already in the Rune tree; draws natively (no WebView, no network). |
| Rendered math | `src/math_tex.rs` | `ratex-render` 0.1.15 (+ layout/parser/types) with `embed-fonts` — KaTeX-compatible layout → PNG; fonts stay in-tree. Images sync via `ImageCache` + `image.wait()` so snap/first paint is not blank. |
| PDF pages / text / thumbs | `src/pdf.rs` | macOS **PDFKit** via `objc2-pdf-kit` 0.3 (same objc2 0.6 line Rune uses). Electron used sandboxed pdf.js; native uses the system framework, still offline and non-executable. Non-macOS refuses with Electron's "Locust could not read that PDF." words. |
| Word / PPT / XLSX / CSV | `src/office.rs` | `calamine` 0.38 for `.xlsx` (read-only, no formula exec; pairs with zip 2). `zip` 2 + XML text for `.docx`/`.pptx` (same approach as Electron `office-text.ts`). CSV/TSV split locally with Electron bounds. |
| File menu / reveal | `src/file_menu.rs` | No new crate. Finder reveal via `/usr/bin/open -R` through `runtime::process::own_group`. |

Scenes (dark + light, `--size 900x900 --scale 1`): `p14-diagram`, `p14-math`, `p14-docs` → `/Users/locust/work/scratch/p14-snaps/`. Inspected: flowchart draws with Diagram/Code + "Flowchart"; matrix PNG paints dark and light; `$5`/`$10` stay prose; docs pane shows Copy path / relative / file name + Close on the file reader.

## What rune-mermaid can and cannot draw

**Source (rune-mermaid `lib.rs` at 39301f6):** documents flowcharts, sequence, state, class, ER, gantt, pie, mindmap, timeline, journey, gitGraph, quadrant, XY, sankey, block, requirement, and C4. `render` returns `None` for types it does not know (**kanban, packet, architecture**, …), when nothing reads, or past size bounds (400 items, nesting 8). Streaming rebuild keeps keyed nodes.

**Conclusion from that + our tests/snaps:** closed `graph`/`flowchart` fences draw in-reply with Electron's Diagram/Code words. Unsupported or empty fences stay as source; `prepare_reply_markdown` attaches Electron's undrawn marker (`Locust could not draw it` / `Not drawn as a diagram (…). Its text is above, as written.`). The p14-diagram scene's kanban fence correctly stays text; the failure caption can miss when fence-index matching in the reply walker is off — remaining gap on the Mermaid partial row.

## Row status (Area 15)

| Feature | Status after P14 |
| --- | --- |
| Coloured code / copy / path controls | **partial** (unchanged this round) |
| File viewer + context menu / reveal | **partial** — menu words, copy path/relative/name, attach, Show in Finder, rooted reveal; Markdown mode, version history, save, Open in VS Code/Cursor wiring, default-app open remain |
| Mermaid diagram/source view | **partial** — tabs + flowchart draw; undrawable → source; caption index gap; no Electron `diagram-view` window |
| PDF pages / thumbnail / text | **partial** — PDFKit read, reading text (+ BOM), page PNGs, `readablePdfNote` / refusal words; no full PdfPages/PdfTile card chrome in the thread |
| Word/PPT, sheets, image zoom | **partial** — docx/pptx/xlsx/csv reading + `documentWords` / brief `office_words_section` (Electron 40 MiB / 12-sheet caps); DocumentChange, SheetView grid UI, ZoomableImage not ported |
| Runnable HTML reply pages | **missing** (out of the requested five) |
| Rendered math in replies | **done** — `$…$` / `$$…$$` / `\(...\)` / `\[…\]`, fence skip, money stays prose, RaTeX PNG via ImageCache |

## TypeScript test disposition

“Partial” means applicable assertions remain unported, not silently passed. Rust tests live in `diagram.rs`, `math_tex.rs`, `pdf.rs`, `office.rs`, `file_menu.rs`.

| Electron test under desktop/src | Disposition |
| --- | --- |
| `shared/diagram.ts` behavior (via `main/a-diagram-in-a-reply-is-drawn.test.ts`, `renderer/.../a-diagram-in-a-reply-shows-as-one.test.tsx`) | Partial: `is_diagram`, kind words, undrawn reason/caption, mmd→mermaid, flowchart `can_draw`, kanban unsupported; Electron Mermaid.js full type matrix / diagram-view window / a11y not claimed |
| `renderer/src/math-in-a-reply-is-drawn.test.tsx` (+ agentText dollar rules) | Covered for delimiters, fence skip, money-as-prose, display/inline PNG path; KaTeX DOM/CSS and every agentText edge not claimed |
| `main/a-pdf-arrives-readable.test.ts`, `main/a-pdf-shows-its-pages.test.ts`, `renderer/.../a-pdf-is-a-card.test.tsx`, `a-pdf-page-reads-in-order.test.ts` | Partial: reading text/BOM, picture names, note wording, path refusal, macOS PDFKit text+PNG on a minimal fixture; Electron pdf.js sandbox, PdfTile card, window, full page-order UI remain |
| `main/an-office-file-is-read-for-its-words.test.ts`, `an-office-file-reads-in-any-mode.test.ts`, `a-workbook-is-read-for-its-cells.test.ts`, `renderer/.../a-spreadsheet-opens-as-a-grid.test.tsx` | Partial: `document_words`, docx zip+XML, xlsx refuse, csv bounds, office-words write under native folder; SheetView grid, DocumentChange, every mode/lock-file UI path remain |
| `shared/file-menu.test.ts`, `main/reveal-file.test.ts`, `renderer/.../the-file-viewer-renders-and-never-runs.test.tsx` | Partial: menu labels, containment, decide_reveal, relative/system paths; live Finder open, editor launch, “never runs” full viewer matrix remain (reveal uses `/usr/bin/open -R` only when invoked; scenes do not call it) |
| ZoomableImage / DocumentChange / ReplyPage tests | Not ported (image zoom and ReplyPage out of done claim) |

## Validation (11 November 2026)

From `/Users/locust/Documents/Locust/native-review/p14/native` with:

```
PATH=$HOME/.cargo/bin:$PATH
LOCUST_TEST_TS_READER=/Users/locust/work/locust/packages/mission-store/dist/index.js
CARGO_TARGET_DIR=$HOME/work/scratch/native-target-p14
```

No live window, no real agent turn, no Finder/editor launch during verification.

- Three sequential `cargo test --release`: **472 passed, 0 failed, 13 ignored** each (~10.8 s / 10.8 s / 10.6 s). No new ignored tests.
- `cargo build --release`: pass, **zero warnings**.
- `cargo clippy --release`: **four existing `too_many_arguments`** in prior modules (`approval` / `ledger_write` / `native`). No findings in the new Area 15 modules; no new allow/suppress.
- `python3 scripts/test-m1-cli.py` and `test-m3a-cli.py` on the release binary: pass (synthetic only).
- Snaps under `/Users/locust/work/scratch/p14-snaps/` (`p14-diagram`, `p14-diagram-light`, `p14-math`, `p14-math-light`, `p14-docs`, `p14-docs-light`), looked at after the `image.wait()` fix.
- Only tests use unwrap; no production unwrap or `allow(dead_code)` introduced for this round.

## Remaining — not silently skipped

1. Mermaid undrawn caption can miss fence index; Electron Mermaid.js parity for every type and `diagram-view` window.
2. PDF PdfPages/PdfTile thread chrome; non-macOS PDF engine.
3. SheetView grid UI, DocumentChange, ZoomableImage.
4. Open in VS Code/Cursor / default app from the file menu; version history / Markdown mode / save.
5. ReplyPage (HTML source/preview) — still **missing**.
6. Live window with Electron quit and `LOCUST_REAL_*` turns were not run.
