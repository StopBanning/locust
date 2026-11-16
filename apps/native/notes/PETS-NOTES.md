# Pets (parity row 14, "Pet gallery, kept pets, sprites/puppets and bulk removal")

Electron sources: `main/pet-library.ts` (668), `shared/pet-picks.ts`, `shared/pets.ts`, `main/webp-size.ts`,
`shared/avatar.ts` (`PetRef`, `isPetId`), `renderer/src/pets.ts`, `petMotion.ts`, `petPuppets.ts`, `petScreens.ts`,
`components/PetPicks.tsx`, `PetSprite.tsx`, `PetPuppet.tsx`. Tests: `main/a-pet-sheet-is-checked-before-it-is-kept.test.ts`,
`renderer/src/a-pet-plays-what-its-teammate-is-doing.test.ts`, `a-teammate-can-be-a-pet.test.tsx`, `the-kept-pets-are-puppets.test.tsx`.

## What a pet is
`pet.json` beside `spritesheet.webp`: an atlas of 8 columns of 192 x 208 frames, 9 rows (version 1) or 11 (version 2; the last
two rows are a head turned toward each of 16 directions, and column 6 of row 0 is the resting pose). A pet on a teammate is a
`PetRef` (source `bundled` | `gallery` | `codex`, an id, an optional `screen`). The window never names a path or an address.

## Three sources
1. **Bundled**: Hoodie Cat (`resources/pets/hoodie-cat`, MIT, OpenPets' own), the only pet in the installer.
2. **Gallery** (openpets.dev): never shipped (community art, rights stay with makers). Downloaded on the person's click, one
   file (`spritesheet.webp`; Locust writes its own `pet.json`). Only the 21 PET_PICKS (Colin's kept list, 2026-10-03) are offered;
   any other id is refused before anything is fetched (`That pet is not one Locust offers.`).
3. **Codex**: `~/.codex/pets`, read where it is, never written, at most 100.

## Rules ported exactly (core/src/pet_library.rs, pets.rs)
- Limits: pet.json 128 KB, sheet 24 MB, thumbnail 1 MB, catalog JSON 4 MB, catalog fresh 6 h, 400 thumbnails kept, 30 s timeout.
- `parsedPetMeta`: id = folder, name 1-120, description <= 500 (may be absent/empty), `spritesheetPath` exactly `spritesheet.webp`,
  version absent = 1, exactly the number 2 = 2; each refusal worded as Electron words it. Lengths are counted in UTF-16 units.
- `catalogAddress`: https, host `openpets.dev`, path under `/pets/` (`/pets/catalog.v3/` for catalog pages), no port, no
  credentials, no `..`, 2048 chars at most.
- Gallery search index and catalog page: an entry that does not read is left out, never the whole page.
- Disk: no link is followed (a linked root, a linked pet folder, a linked file are all refused), a file's size is checked before
  and after reading, its resolved path must stay inside the root, the sheet's first 30 bytes are read for size and alpha.
- `petRowsFor`: version 2 exact (1536 x 2288, transparent); version 1 eight across, nine down, frames within 2% of 192:208
  and between half and twice the usual size; an animated or opaque sheet is refused.
- Removal: a pick taken out of the picker is remembered in `pets-removed.json` (`{schemaVersion:1, ids}`, one write at a time,
  temp file then rename); a pet a teammate wears keeps its files (`keepFiles`); adding a pick again brings it back; a list that
  does not read is "nothing removed".

## Built here (this file's commit)
Pure core: `core/src/pets.rs` (WebP header reader, sheet rows, state rows and timings, gaze cells), `core/src/pet_library.rs`
(picks, meta, addresses, gallery/catalog parsing, the on-disk library: list, sheet, removed picks, remove, bring back),
`core/src/pet_library_tests.rs` (one case each, Electron's values).

## Built second: the download half (core/src/pet_gallery.rs, pet_gallery_tests.rs)
`PetGallery`: `add`, `thumbnail`, `gallery_index`, behind a `PetFetch` trait (`UreqFetch`: no redirects, 30 s; a 3xx is
"sent somewhere else"). Size counted as it arrives (reads at most max+1), declared length refused first, final-URL check,
catalog kept 6 h in memory and in `pets-cache/gallery.json` and the last read used (stale) when the gallery is gone, catalog
pages cached 6 h, thumbnails kept and touched, at most 400 (oldest by mtime go), a pet written beside then renamed, one added
meanwhile by another window stands, picking a removed pick brings it back. Every refusal in Electron's words. 15 tests, 11
break checks all caught. Differences from Electron, on purpose: the kept catalog writes `spriteVersionNumber` (Electron
writes `rows`, which its own reader ignores, so a reloaded index said 9 rows for every pet); adds and thumbnails are not
de-duplicated per id across threads (an add holds one lock; a thumbnail read twice at once downloads twice).

## Not built yet (in order)
1. ~~The window side of `PetGallery`~~ and 2. ~~the picker row~~: built 16 Nov in `src/ui/pet_picks.rs` (three
   `rune::Latest` jobs: the list, the 21 thumbnails decoded from WebP by the `image` crate, an add; pets added go to
   `<native>/pets`, never Electron's folder; Codex's read where they are). Still to do from PetPicks.tsx: Remove some... /
   Remove N and Show removed (`usePetRemoval`), and reading Electron's own added pets.
3. A pet playing its teammate's activity (`petMotion.ts`: activity -> state map, frame at time, fade, glance cell) and drawing a
   sheet frame (WebP decode needs an image decoder in the window crate; Codex Buddy is the only pet drawn today, as a screen).
4. Puppets (`petPuppets.ts`, `petScreens.ts`): the kept pets drawn as rigs; large.
5. Bulk removal UI.
