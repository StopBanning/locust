# Fonts

The faces Electron's renderer uses (`renderer/src/tokens.css`), as TrueType, because Rune's text stack reads `.ttf`
and not the `.woff2` copies Electron ships. Compiled into the binary by `src/look/mod.rs` (`FONTS`), never fetched at
run time.

| File | Role | Source | SHA-256 |
| --- | --- | --- | --- |
| `Figtree-Variable.ttf` | The app's own voice: chrome, labels, controls | github.com/erikdkennedy/figtree at `032dfa7` (the commit Electron's woff2 came from), `fonts/variable/Figtree[wght].ttf` | `c8d9e77bb970c18f7b55fd2d8c91f86c9e9cc42696da9c3e7bc4fffba1d3ef5a` |
| `Figtree-Italic-Variable.ttf` | Its italic (two chrome states are set in italic) | the same commit, `fonts/variable/Figtree-Italic[wght].ttf` | `e9b7ab9c7c7eaed0ef891d163a7b4a055813cd2e8507fa727f2989f93acdca53` |
| `GeistMono-Variable.ttf` | The machine: paths, ids, commands | github.com/vercel/geist-font at `ef2597c`, `fonts/GeistMono/variable/GeistMono[wght].ttf` | `87c2aff9723544a9adaea19d92e42a33705c9723624801b6e0224c2206a6af0d` |

Both are under the SIL Open Font License 1.1; the licences sit beside the files. Fetched 11 Nov 2026.

Not here yet: Anthropic Sans, which Electron uses for a teammate's reply. Electron vendors it from a third party's MIT
repackage, so its licence is checked before it goes into a binary that ships. Replies use Figtree until then.
