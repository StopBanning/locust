# Third-party notices

Locust includes work from the projects below, under their licences.

## Terminal

- xterm.js (`@xterm/xterm` 6.0.0, `@xterm/addon-fit` 0.11.0, `@xterm/addon-web-links` 0.12.0), MIT.
  https://github.com/xtermjs/xterm.js
- node-pty prebuilt distribution (`@lydell/node-pty` 1.2.0-beta.15), MIT.
  https://github.com/lydell/node-pty
  https://github.com/microsoft/node-pty
- Native binaries and their licences ship outside app.asar; xterm's licence ships with its package.

## Optional local voice typing

The Windows microphone's first-use Download retrieves whisper.cpp build b5454
and Whisper tiny.en Q5_1 weights. Neither the executable nor the model ships in
the installer. Their MIT notices are kept alongside the downloaded files in
`voice/LICENSE.txt`; the exact notice is in `apps/desktop/src/shared/voice-license.ts`.

- whisper.cpp / ggml: MIT, Copyright (c) 2023-2026 The ggml authors.
  https://github.com/ggml-org/whisper.cpp/blob/b5454/LICENSE
- Whisper model: MIT, Copyright (c) 2022 OpenAI.
  https://github.com/openai/whisper/blob/main/LICENSE

## PDF.js

- Project: https://github.com/mozilla/pdf.js
- Licence: Apache License, Version 2.0, Copyright Mozilla Foundation.
  https://www.apache.org/licenses/LICENSE-2.0
- Version: `pdfjs-dist` 6.3.289, unmodified.
- What Locust uses: the library, bundled into the page that opens an
  attached PDF for an agent (`apps/desktop/src/renderer/pdf.html`,
  `apps/desktop/src/renderer/src/pdfReader.ts`). It reads the PDF's text and
  draws its pages as pictures on this machine; nothing is fetched.

## Shiki

- Project: https://github.com/shikijs/shiki
- Licence: MIT, Copyright (c) 2021 Pine Wu, Copyright (c) 2023 Anthony Fu.
  https://github.com/shikijs/shiki/blob/main/LICENSE
- Version: `shiki` 4.4.3, unmodified, with its JavaScript regex engine and
  the css-variables theme.
- What Locust uses: the highlighter and 31 of its language grammars, bundled
  and loaded only when a reply holds code in that language
  (`apps/desktop/src/renderer/src/codeColors.ts`). The grammars are the
  TextMate grammars VS Code uses, collected by
  https://github.com/shikijs/textmate-grammars-themes, each under its own
  licence as listed there.

## Mermaid

- Project: https://github.com/mermaid-js/mermaid
- Licence: MIT, Copyright (c) 2014 - 2022 Knut Sveidqvist.
  https://github.com/mermaid-js/mermaid/blob/develop/LICENSE
- Version: `mermaid` 11.17.2, unmodified.
- What Locust uses: the library, bundled into the page that draws a diagram
  in a reply (`apps/desktop/src/renderer/diagram.html`,
  `apps/desktop/src/renderer/src/diagramPage.ts`), in a view of its own that
  fetches nothing. The libraries it is built on come with it, each under its
  own licence: D3 (ISC) and d3-sankey (BSD-3-Clause); dagre-d3-es,
  Cytoscape.js and its fcose and cose-bilkent layouts, khroma, Rough.js,
  marked, stylis, Day.js, lodash-es, es-toolkit, uuid, ts-dedent,
  @iconify/utils and @braintree/sanitize-url (MIT); Langium (MIT) and
  Chevrotain (Apache-2.0), built into `@mermaid-js/parser` (MIT); KaTeX (MIT),
  which Locust already carries; and DOMPurify (Apache-2.0, of its Apache-2.0
  or MPL-2.0 choice).

## OpenPets

- Project: https://github.com/OpenPetsHQ/openpets
- Licence: MIT, Copyright (c) 2026 OpenPets
- Commit: `2d14120cf027c9e80db7ff78e60711be08d39df4`
- What Locust uses:
  - The Hoodie Cat sprite sheet (`apps/desktop/assets/default-pet-spritesheet.webp`),
    shipped as `apps/desktop/resources/pets/hoodie-cat/spritesheet.webp`, with
    the licence beside it (`LICENSE` in the same folder).
  - The pet format and its rules, ported to TypeScript in
    `apps/desktop/src/shared/pets.ts` and `apps/desktop/src/main/pet-library.ts`:
    the sheet layouts, rows and timings (`codex-pets-core.ts`,
    `reaction-animation-mapping.ts`) and the catalog's address rules
    (`catalog-validation.ts`).

Pets in the openpets.dev gallery are not part of Locust. A person who adds one
downloads it from openpets.dev to their own computer; its rights stay with its
maker, as the gallery's terms say. Locust ships none of them.

```
MIT License

Copyright (c) 2026 OpenPets

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Lucide icons

Many of the shell's icons (`apps/desktop/src/renderer/src/components/Icon.tsx`, and the Rust app's
`apps/native/src/look/icons.rs` generated from it) are Lucide's drawings, redrawn in the set's 1.7 line.

- Project: https://github.com/lucide-icons/lucide
- Licence: ISC. Its notice, as published:

```
ISC License

Copyright (c) 2026 Lucide Icons and Contributors

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.

---

The following Lucide icons are derived from the Feather project:

airplay, alert-circle, alert-octagon, alert-triangle, aperture, arrow-down-circle, arrow-down-left, arrow-down-right, arrow-down, arrow-left-circle, arrow-left, arrow-right-circle, arrow-right, arrow-up-circle, arrow-up-left, arrow-up-right, arrow-up, at-sign, calendar, cast, check, chevron-down, chevron-left, chevron-right, chevron-up, chevrons-down, chevrons-left, chevrons-right, chevrons-up, circle, clipboard, clock, code, columns, command, compass, corner-down-left, corner-down-right, corner-left-down, corner-left-up, corner-right-down, corner-right-up, corner-up-left, corner-up-right, crosshair, database, divide-circle, divide-square, dollar-sign, download, external-link, feather, frown, hash, headphones, help-circle, info, italic, key, layout, life-buoy, link-2, link, loader, lock, log-in, log-out, maximize, meh, minimize, minimize-2, minus-circle, minus-square, minus, monitor, moon, more-horizontal, more-vertical, move, music, navigation-2, navigation, octagon, pause-circle, percent, plus-circle, plus-square, plus, power, radio, rss, search, server, share, shopping-bag, sidebar, smartphone, smile, square, table-2, tablet, target, terminal, trash-2, trash, triangle, tv, type, upload, x-circle, x-octagon, x-square, x, zoom-in, zoom-out

The MIT License (MIT) (for the icons listed above)

Copyright (c) 2013-present Cole Bemis

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
