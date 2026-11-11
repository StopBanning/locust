// Execute the actual read-only Electron reducers on the same synthetic rows as Rust.
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
// Electron's source in this repository (apps/desktop/src), or the checkout LOCUST_ELECTRON_CHECKOUT names.
const root = process.env.LOCUST_ELECTRON_CHECKOUT
  ? `${process.env.LOCUST_ELECTRON_CHECKOUT}/apps/desktop/src/`
  : new URL('../../desktop/src/', import.meta.url).pathname;
const load = file => stripTypeScriptTypes(readFileSync(root + file, 'utf8'));
const url = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const list = url(load('renderer/src/conversationList.ts'));
const statusSource = load('renderer/src/status.ts');
const start = statusSource.indexOf('export function missionPhaseView(');
if (start < 0) throw new Error('Electron missionPhaseView moved');
const status = url(statusSource.slice(start, statusSource.indexOf('\n}', start) + 2));
const board = await import(url(load('renderer/src/conversationBoard.ts')
    .replaceAll('./conversationList.js', list).replaceAll('./status.js', status)));
const away = await import(url(load('shared/away.ts')));
const input = JSON.parse(readFileSync(0, 'utf8'));
const needsYou = new Set(input.needsYou);
const summary = away.sinceYouWereAway({...input, needsYou});
console.log(JSON.stringify({counts: summary ? away.awayCounts(summary) : null,
    columns: (board.conversationBoard(input.rows, [], new Set(input.trashed),
        {needsYou, toLookAt: new Set(input.toLookAt)}) ?? []).map(s => [s.key, s.members.map(c => c.row.missionId)])}));
