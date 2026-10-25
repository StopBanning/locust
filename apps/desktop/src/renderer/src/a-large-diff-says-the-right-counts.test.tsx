import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'

import { ActivityCard } from './components/ActivityCard.js'
import type { ActivityDetail } from './missionView.js'

/**
 * A LARGE FILE'S CHANGE SAYS THE RIGHT COUNTS.
 *
 * When a large diff is truncated, the excerpt's cut lines must never be
 * shown as if they were the true count (e.g. +0 −747 for a 1,700-line rewrite
 * where lines were added). The counts shown for a LARGE change are either right
 * (from the tool's own reported record, or from full untruncated text) or not
 * shown at all (letting LARGE say it).
 */
;(globalThis as { window?: unknown }).window = { desktop: { platform: 'win32' } }

const cutLines = [
  '--- a/src/big.ts',
  '+++ b/src/big.ts',
  '@@ -1,1700 +1,1700 @@',
  ...Array.from({ length: 747 }, (_, i) => `-line ${String(i + 1)}`)
].join('\n')

const largeRewriteDetail: ActivityDetail = {
  kind: 'edit',
  name: 'src/big.ts',
  tool: 'edit',
  settled: true,
  patch: {
    text: cutLines,
    added: 1700,
    removed: 1700,
    truncated: true
  }
}

const multiFileCutDetail: ActivityDetail = {
  kind: 'edit',
  name: 'src/big.ts\nsrc/other.ts',
  tool: 'edit',
  settled: true,
  patch: {
    text: `${cutLines}\n--- a/src/other.ts\n+++ b/src/other.ts\n@@ -1,1 +1,1 @@\n-a\n+b\n`,
    added: 1701,
    removed: 1701,
    truncated: true
  }
}

const smallEdit = [
  '--- a/src/small.ts',
  '+++ b/src/small.ts',
  '@@ -1,2 +1,3 @@',
  ' unchanged',
  '-old',
  '+new 1',
  '+new 2'
].join('\n')

const smallDetail: ActivityDetail = {
  kind: 'edit',
  name: 'src/small.ts',
  tool: 'edit',
  settled: true,
  patch: {
    text: smallEdit,
    added: 2,
    removed: 1,
    truncated: false
  }
}

describe('counts on a large diff row', () => {
  it('never says +0 with a deletion count when lines were added in a recorded LARGE rewrite', () => {
    const html = renderToStaticMarkup(
      <ActivityCard
        summary="edited 1 file"
        details={[largeRewriteDetail]}
        runtimeName="Cursor"
        workspacePath="C:/work"
        openByDefault
      />
    )

    // The row marks the file as large.
    expect(html).toContain('is-large')
    expect(html).toContain('>large<')

    // It reports the true counts from the tool's record.
    expect(html).toContain('+1700')
    expect(html).toContain('−1700')

    // It NEVER says +0 with a deletion count when lines were added.
    expect(html).not.toMatch(/\+0\b/)
    expect(html).not.toContain('−747')
  })

  it('shows no counts and lets LARGE say it when true counts cannot be known', () => {
    const html = renderToStaticMarkup(
      <ActivityCard
        summary="edited 2 files"
        details={[multiFileCutDetail]}
        runtimeName="Cursor"
        workspacePath="C:/work"
        openByDefault
      />
    )

    // Truncated multi-file edit is marked large.
    expect(html).toContain('is-large')
    expect(html).toContain('>large<')

    // Neither partial counts nor +0 are shown on the truncated big.ts row.
    expect(html).not.toMatch(/\+0\b/)
    expect(html).not.toContain('−747')
  })

  it("leaves a small diff's counts unchanged", () => {
    const html = renderToStaticMarkup(
      <ActivityCard
        summary="edited 1 file"
        details={[smallDetail]}
        runtimeName="Cursor"
        workspacePath="C:/work"
        openByDefault
      />
    )

    // Small diff shows its exact counts.
    expect(html).toContain('+2')
    expect(html).toContain('−1')

    // Small diff is not marked LARGE.
    expect(html).not.toContain('is-large')
  })
})
