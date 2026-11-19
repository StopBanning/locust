// Records what Electron's cost words return (cost.ts: costLine, moneyLine), for the Rust port to replay
// (parity/recorded/cost.jsonl). Run through record.sh. Inputs: a grid of costs, each field present or not, with edge
// values (rounding at a cent, tokens at the thousand and million). (connectorLabel lives in a React component, so the
// connector words wait for a recorder that can load one.)
import { costLine, moneyLine, type RunCost } from '../../../desktop/src/renderer/src/cost.ts'

const out: string[] = []
const record = (fn: string, args: unknown[], value: unknown) => out.push(JSON.stringify({ fn, args, out: value === undefined ? null : value }))

const USD = [undefined, 0, 0.001, 0.004999, 0.005, 0.00999, 0.01, 0.015, 0.125, 0.995, 1, 1.005, 2.675, 12.3456, 1234.5]
const PREMIUM = [undefined, 0, 1, 2, 1.5, 300]
const TOKENS = [undefined, 0, 999, 1000, 1499, 1500, 9999, 12345, 999999, 1000000, 2500000]
const costs: (RunCost | undefined)[] = [undefined]
for (const usd of USD) for (const premiumRequests of PREMIUM) for (const plan of [undefined, true, false]) {
  costs.push({ ...(usd === undefined ? {} : { usd }), ...(premiumRequests === undefined ? {} : { premiumRequests }), ...(plan === undefined ? {} : { plan }) })
}
for (const inputTokens of TOKENS) for (const outputTokens of TOKENS) for (const plan of [undefined, true]) {
  costs.push({ ...(inputTokens === undefined ? {} : { inputTokens }), ...(outputTokens === undefined ? {} : { outputTokens }), ...(plan === undefined ? {} : { plan }) })
}
for (const cost of costs) {
  record('costLine', [cost ?? null], costLine(cost))
  record('moneyLine', [cost ?? null], moneyLine(cost))
}

process.stdout.write(out.join('\n') + '\n')
