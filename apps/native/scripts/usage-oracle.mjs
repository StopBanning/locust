// Read-only oracle: run the actual Electron pure helpers on synthetic receipts.
import { readFileSync } from 'node:fs';
import { stripTypeScriptTypes } from 'node:module';
// Electron's source in this repository (apps/desktop/src), or the checkout LOCUST_ELECTRON_CHECKOUT names.
const root = process.env.LOCUST_ELECTRON_CHECKOUT
  ? `${process.env.LOCUST_ELECTRON_CHECKOUT}/apps/desktop/src/`
  : new URL('../../desktop/src/', import.meta.url).pathname;
const url = source => `data:text/javascript;base64,${Buffer.from(source).toString('base64')}`;
const load = file => stripTypeScriptTypes(readFileSync(root + file, 'utf8'));
const spendUrl = url(load('shared/spend.ts'));
const usageUrl = url(load('shared/usage.ts'));
const viewUrl = url(load('renderer/src/usageView.ts')
  .replaceAll('../../shared/spend.js', spendUrl).replaceAll('../../shared/usage.js', usageUrl));
const usage = await import(usageUrl);
const view = await import(viewUrl);
const spend = await import(spendUrl);
const input = JSON.parse(readFileSync(0, 'utf8'));
const s = usage.usageSummary(input.turns, input.range, new Date(input.now));
console.log(JSON.stringify({turns:s.turns, conversations:s.conversations, tokens:s.tokens, pay:s.pay,
  activeDays:s.activeDays, rangeDays:s.rangeDays, streak:s.streak, days:s.days, heatmap:s.heatmap,
  models:s.models.map(m=>({model:m.model, turns:m.turns, paidBy:view.paidBy(m), cached:view.cachedText(m),share:view.shareOf(m,s)})),
  facts:view.usageLineFacts(s), spent:view.spentSentence(s), dollars:input.dollars.map(spend.dollars)}));
