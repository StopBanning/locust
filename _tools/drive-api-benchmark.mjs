// Fixed tasks, objective checks first, then a disjoint panel, then the built-in benchmark rerun.
// Free proof: node _tools/drive-api-benchmark.mjs --stand-in --tasks bug,html --runs 2
// Paid: LOCUST_SPEND=1 and LOCUST_OWN_MODEL_KEY from a masked prompt; never put a key in argv.
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { FREE_ROUTE, recordRoot, say, sleep, startDrive } from './drive-lib.mjs'
import { CONTESTANTS, API_JUDGES, PRICES, options, receipt, resultRow, spendStop, csv, summary, ownModelRoute, answerOf } from './api-benchmark.mjs'
import { TASKS, fixture, objective, run } from './api-benchmark-tasks.mjs'
import { parseScorecard, panelVote, panelPick } from '../apps/desktop/src/shared/compare-score.ts'
import { judgePickOf } from '../apps/desktop/src/shared/compare.ts'

const opts = options(process.argv.slice(2))
// Read at most once, and remove before ANY child can inherit it, including in stand-in mode.
const KEY = opts.standIn ? undefined : process.env.LOCUST_OWN_MODEL_KEY
delete process.env.LOCUST_OWN_MODEL_KEY
if (!opts.standIn && (!KEY || process.env.LOCUST_SPEND !== '1')) throw new Error('Paid mode needs LOCUST_SPEND=1 and LOCUST_OWN_MODEL_KEY from a masked prompt.')
if (new Set(opts.tasks).size !== opts.tasks.length || opts.tasks.some((id) => !TASKS[id])) throw new Error('Tasks: bug,feature,refactor,tests,html; no duplicates.')
if (opts.tag && !/^[a-z0-9_-]+$/i.test(opts.tag)) throw new Error('Use a simple tag without path separators.')
const out = join(recordRoot('api-benchmark'), opts.tag ?? new Date().toISOString().replace(/[:.]/g, '-'))
await mkdir(out, { recursive: true })
const meta = { standIn: !!opts.standIn, runs: opts.runs, tasks: opts.tasks, prices: PRICES, charges: [], bases: [], frames: [], scan: 'pending' }
const rows = [], judges = [], setup = []
let failures = 0
const check = (what, good) => { say(`[${good ? 'PASS' : 'FAIL'}] ${what}`); if (!good) failures += 1 }
const save = async () => {
  await writeFile(join(out, 'results.json'), JSON.stringify({ meta, rows, judges, setup }, null, 2))
  await writeFile(join(out, 'results.csv'), csv(rows))
  await writeFile(join(out, 'SUMMARY.md'), summary(rows, meta))
}
const budget = () => { const why = spendStop(meta.charges, opts.maximum); if (why) { meta.stop = why; say(why) }; return !why }
const cleanError = (error) => {
  const text = error instanceof Error ? error.message : String(error)
  return KEY ? text.replaceAll(KEY, '[redacted]') : text
}
const apiModels = [...CONTESTANTS, ...(opts.judges === 'api' ? API_JUDGES : [])]
if (!opts.standIn) {
  const ids = new Set()
  let after
  for (let page = 0; page < 20; page += 1) {
    const url = 'https://api.anthropic.com/v1/models?limit=100' + (after ? '&after_id=' + encodeURIComponent(after) : '')
    const response = await fetch(url, { redirect: 'error', headers: { 'x-api-key': KEY, 'anthropic-version': '2023-06-01' }, signal: AbortSignal.timeout(30_000) })
    if (!response.ok) throw new Error(`API model listing refused (${response.status}); nothing started.`)
    const listed = await response.json()
    for (const model of listed.data ?? []) ids.add(model.id)
    if (!listed.has_more) break
    if (!listed.last_id || listed.last_id === after) throw new Error('API pagination did not advance.')
    after = listed.last_id
  }
  const missing = apiModels.filter((model) => !ids.has(model))
  if (missing.length) throw new Error(`API does not list ${missing.join(', ')}; no turns started.`)
}

const fill = (selector, value) => `(() => { const field = document.querySelector(${JSON.stringify(selector)}); if (!field) return false; Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(field, ${JSON.stringify(value)}); field.dispatchEvent(new Event('input', {bubbles:true})); return true; })()`
// Records are scanned strictly. The live profile is scanned too, but Chromium holds some of its files open while the
// app runs (EBUSY, 2026-10-21): those are listed as unscanned, never read as clean; the profile is deleted by finish().
async function scan(folder, locked) {
  let files = 0
  for (const entry of await readdir(folder, { withFileTypes: true })) {
    const path = join(folder, entry.name)
    if (entry.isDirectory()) { files += await scan(path, locked); continue }
    if (!entry.isFile()) continue
    let bytes
    try { bytes = await readFile(path) } catch (error) {
      if (locked && (error?.code === 'EBUSY' || error?.code === 'EPERM')) { locked.push(path); continue }
      throw error
    }
    if ((KEY && bytes.includes(Buffer.from(KEY))) || /sk-ant-[A-Za-z0-9_-]{20,}/.test(bytes.toString('utf8'))) throw new Error('Secret scan failed; records must not be shared.')
    files += 1
  }
  return files
}

for (const taskId of opts.tasks) {
  if (!budget()) break
  const task = TASKS[taskId], fixed = await fixture(taskId)
  meta.bases.push({ task: taskId, commit: fixed.commit, hash: fixed.hash })
  const drive = await startDrive({ name: 'api-benchmark', port: 9841, workspace: fixed.root, spends: !opts.standIn,
    outPath: join(out, taskId), ...(opts.packaged ? { packaged: opts.packaged } : {}),
    seed: { schemaVersion: 1, teammates: [{ teammateId: 'tm_wren', name: 'Wren', hue: 'lime', role: 'Code & Migrations', route: FREE_ROUTE, createdAt: '2026-10-21T12:00:00.000Z' }], missionOwners: {}, settings: { memoryMode: 'off', swarm: false, relay: false, autoMode: false } }
  })
  const call = async (method, ...args) => {
    const value = await drive.evaluate(`(async () => JSON.stringify(await window.desktop[${JSON.stringify(method)}](...${JSON.stringify(args)})))()`)
    if (value === undefined) throw new Error(`No answer to ${method}`)
    return JSON.parse(value)
  }
  const all = async () => (await call('listCompares')).data?.compares ?? []
  const compare = async (id) => (await all()).find((item) => item.compareId === id)
  const mission = async (id) => id ? (await call('readMission', id)).data?.mission : undefined
  const capture = async (name) => {
    // Even a failed Settings Test cannot put a filled password box into a frame.
    await drive.evaluate(`(() => { for (const field of document.querySelectorAll('input[type="password"]')) { Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(field,''); field.dispatchEvent(new Event('input',{bubbles:true})); } })()`)
    if (!(await drive.evaluate(`[...document.querySelectorAll('input[type="password"]')].every(field=>field.value==='')`))) throw new Error('A key field was not cleared; no frame taken.')
    await drive.capture(name, () => drive.evaluate(`document.querySelector('.lc-benchmark')?.innerText ?? document.querySelector('.lc-compare')?.innerText ?? document.body.innerText`))
  }
  const ended = (record) => record?.events?.some((event) => ['run.completed','run.failed','run.cancelled'].includes(event.type))
  const waitTurns = async (id, panel = false) => {
    const until = Date.now() + 600_000
    while (Date.now() < until) {
      const item = await compare(id)
      const ids = panel ? [item?.judge?.missionIds.at(-1), ...(item?.judge?.panel ?? []).map((member) => member.missionId)].filter(Boolean) : item?.slots.map((slot) => slot.missionIds.at(-1)).filter(Boolean) ?? []
      const reads = await Promise.all(ids.map(mission))
      if (panel ? ids.length === 3 && reads.every(ended) : item && item.slots.every((slot) => slot.refused || (slot.missionIds.length && ended(reads.find((read) => read?.missionId === slot.missionIds.at(-1)))))) return item
      await sleep(1500)
    }
    throw new Error(`Timed out waiting for ${panel ? 'judges' : 'columns'}; no more turns.`)
  }
  let originId
  const charged = new Set()
  const charge = (record, model) => {
    if (!record || charged.has(record.missionId)) return
    charged.add(record.missionId)
    const dollars = receipt(record, model).estimatedDollars
    // NEVER REACHED THE API, SO $0 (2026-10-21). OpenCode's server can die on its own "database is locked" while three
    // start at once, before it listens: no prompt was ever sent. Read as unknown spend, that stopped the first paid run
    // after one round. Anything else without usage still stops the run.
    const failed = [...(record.events ?? [])].reverse().find((event) => event.type === 'run.failed')?.payload
    const neverListened = record.phase === 'failed' && failed?.process?.recordCount === 0 && /server exited before the turn finished/.test(failed?.process?.stderr ?? '') && /database is locked/.test(failed?.process?.stderr ?? '')
    if (dollars === null && neverListened) { meta.unsentFailures = (meta.unsentFailures ?? 0) + 1; meta.charges.push(0); return }
    meta.charges.push(dollars)
  }
  try {
    await drive.ready()
    await drive.resize(1440, 900)
    const listed = await call('listModels')
    const free = (listed.data?.models ?? []).filter((model) => model.runtime === 'opencode' && model.id.endsWith('-free'))
    const preferred = ['deepseek-v4.1-flash','space-bunny','ling-3.1','muse-spark','nemotron','minimax']
    free.sort((a,b) => {
      const rank = (item) => { const at = preferred.findIndex((name) => item.id.includes(name)); return at < 0 ? 100 : at }
      return rank(a)-rank(b) || a.id.localeCompare(b.id)
    })
    const unique = [...new Map(free.map((item) => [item.id,item])).values()]
    if (unique.length < (opts.standIn ? 6 : 3) && opts.judges === 'free') throw new Error('Need six advertised free models in stand-in, three for a paid run with free judges.')
    const freeRoute = (item) => ({ runtime: 'opencode', model: item.id, label: item.label ?? item.id })
    let contestants, panel
    if (opts.standIn) { contestants = unique.slice(0,3).map(freeRoute); panel = unique.slice(3,6).map(freeRoute) }
    else {
      await drive.evaluate(`window.dispatchEvent(new KeyboardEvent('keydown',{key:'3',ctrlKey:true,bubbles:true}));1`)
      await drive.waitFor(`[...document.querySelectorAll('button')].some(b=>b.textContent.trim()==='Your own models')`, { what:'Settings' })
      await drive.evaluate(`[...document.querySelectorAll('button')].find(b=>b.textContent.trim()==='Your own models').click();1`)
      await drive.waitForSelector('.lc-ownmodel__form')
      for (const model of apiModels) {
        if (!budget()) break
        for (const [i,value] of [model,model,'https://api.anthropic.com/v1',KEY].entries()) {
          if (i === 3 && !(await drive.evaluate(`document.querySelector('.lc-ownmodel__form label:nth-of-type(4) input')?.type==='password'`))) throw new Error('The Key input is not masked.')
          await drive.evaluate(fill(`.lc-ownmodel__form label:nth-of-type(${i+1}) input`, value))
        }
        meta.charges.push(0.01)
        await drive.evaluate(`[...document.querySelectorAll('.lc-ownmodel__actions button')].find(b=>b.textContent.trim()==='Test').click();1`)
        await drive.waitFor(`(() => {const text=document.querySelector('.lc-ownmodel__form .lc-ownmodel__said')?.innerText;return text && !text.startsWith('Testing')})()`, {timeoutMs:90_000,what:'the model Test'})
        // Reached /models alone is insufficient; the one-token tool request must have worked too.
        const tested = await drive.evaluate(`(() => { const text=document.querySelector('.lc-ownmodel__form .lc-ownmodel__said')?.innerText;return (text??'').includes('It can use tools.') })()`)
        setup.push({ task:taskId, model, tested:!!tested, estimateAllowance:0.01 })
        if (!tested) throw new Error(`Test did not confirm tool support for ${model}; no benchmark started.`)
        await drive.evaluate(`[...document.querySelectorAll('.lc-ownmodel__actions button')].find(b=>b.textContent.trim()==='Add model').click();1`)
        await drive.waitFor(`document.querySelector('.lc-ownmodel__form input[type="password"]')?.value===''`,{what:'the saved key is cleared'})
      }
      if (setup.filter((item) => item.task === taskId && item.tested).length !== apiModels.length) break
      await capture('Settings - API models tested and key cleared')
      const saved = (await call('listOwnModels')).data?.models ?? []
      const ownRoute = (model) => { const own=saved.find((item)=>item.model===model); if(!own)throw new Error('A tested API model was not saved.');return ownModelRoute(own) }
      contestants = CONTESTANTS.map(ownRoute)
      panel = opts.judges === 'api' ? API_JUDGES.map(ownRoute) : unique.slice(0,3).map(freeRoute)
      await drive.evaluate(`window.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));1`)
    }
    check(`${taskId}: contestant and judge routes are disjoint`, !panel.some((judge) => contestants.some((one) => one.model===judge.model)))
    if (!budget()) break
    const start = await call('startCompare', {teammateId:'tm_wren',prompt:task.ask,changes:true,blind:true,routes:contestants})
    if (!start.ok) throw new Error('The comparison could not start: ' + start.error?.message)
    originId = start.data.compare.compareId
    await drive.evaluate('location.reload();1')
    await drive.waitFor(`[...document.querySelectorAll('.lc-convrow button.lc-conv')].some(row=>/^vs/.test(row.innerText.trim()))`,{what:'the comparison in history'})
    await drive.evaluate(`[...document.querySelectorAll('.lc-convrow button.lc-conv')].find(row=>/^vs/.test(row.innerText.trim())).click();1`)
    await drive.waitForSelector('.lc-compare')
    for (let number=1;number<=opts.runs;number+=1) {
      let item
      if (number === 1) item=await waitTurns(originId)
      else {
        if (!budget()) break
        const started=await call('rerunCompare',originId,1,true)
        if(!started.ok)throw new Error('Run once more refused: '+started.message)
        const until=Date.now()+600_000
        let progress
        while(Date.now()<until) { progress=(await call('compareReruns')).find(entry=>entry.originId===originId); if(progress?.step==='checking')break;await sleep(1500) }
        if(progress?.step!=='checking')throw new Error('Run once more did not reach its objective-check pause.')
        item=await waitTurns(progress.compareId)
      }
      check(`${taskId} run ${number}: same committed base, blind first run, Edit copies`, item.base===fixed.commit && (number!==1 || item.blind===true) && item.slots.every(column=>column.route.mode!=='auto' && column.folder))
      const currentRows=[]
      for(const column of item.slots) {
        const at=contestants.findIndex(route=>route.model===column.route.model)
        if(at<0)throw new Error('A column changed model.')
        const record=await mission(column.missionIds.at(-1))
        const checked=await objective(taskId,column.folder)
        charge(record,column.route.model)
        const row=resultRow({task:taskId,model:CONTESTANTS[at],actualModel:column.route.model,run:number,compareId:item.compareId,mission:record,check:checked,standIn:!!opts.standIn})
        rows.push(row);currentRows.push(row)
        await writeFile(join(drive.out,`run-${number}-${column.slot}-check.txt`),cleanError(checked.said))
        if(record)await writeFile(join(drive.out,`run-${number}-${column.slot}-mission.json`),cleanError(JSON.stringify(record,null,2)))
        check(`${taskId} run ${number} ${column.slot}: finished in Edit`,record?.phase==='completed'&&record.sandbox==='workspace-write')
      }
      await save()
      await capture(`${taskId} run ${number} - objective checks before judging`)
      if(!budget()) { if(number>1)await call('stopCompareRerun',originId);break }
      if(number===1) {
        const judged=await call('judgeCompare',{compareId:item.compareId,route:panel[0],panel:panel.slice(1),criteria:task.points.join('\n')})
        if(!judged.ok || judged.data.refused.length)throw new Error('The panel could not start.')
      } else if(!(await call('checkedCompareRerun',originId,item.compareId)))throw new Error('The checked run was not released to its original panel.')
      item=await waitTurns(item.compareId,true)
      const judgeIds=[item.judge.missionIds.at(-1),...item.judge.panel.map(member=>member.missionId)]
      const records=await Promise.all(judgeIds.map(mission))
      for(const [i,record] of records.entries()) {
        charge(record,panel[i].model)
        judges.push({task:taskId,run:number,model:panel[i].model,...receipt(record,panel[i].model),missionId:record?.missionId??null})
        await writeFile(join(drive.out,`run-${number}-judge-${i+1}.json`),cleanError(JSON.stringify(record??null,null,2)))
      }
      check(`${taskId} run ${number}: all three disjoint judges finish read-only`,records.every((record,i)=>record?.phase==='completed' && record.sandbox==='read-only' && record.model===panel[i].model))
      const texts=records.map(record=>record?.phase==='completed'?answerOf(record):'')
      const cards=texts.map(text=>parseScorecard(text,['a','b','c'],task.points))
      const vote=panelVote(cards)
      const pick=panelPick(texts.map(text=>judgePickOf(text,['a','b','c'])),records.filter(record=>record?.phase==='completed').length)
      for(const [i,row] of currentRows.entries()) {
        const slot=item.slots[i].slot, scores=vote?.verdicts[slot]
        row.pointsMet=row.phase==='completed' && scores?.length===3 && scores.every(Boolean) ? scores.filter(score=>score.met).length : null
        row.panelPick=row.phase==='completed' && pick ? pick===slot : null
      }
      check(`${taskId} run ${number}: the panel supplies complete three-point cards`,currentRows.every(row=>row.pointsMet!==null))
      await save()
      if(number===1) {
        await capture(`${taskId} run 1 - blind panel scorecards`)
        // Built-in benchmarks reveal names only after Keep. This changes only the disposable original, never its saved base.
        const keep=await call('keepCompare',originId,item.slots.find(slot=>slot.slot===pick)?.slot??item.slots.find(slot=>currentRows[item.slots.indexOf(slot)].checkPassed)?.slot??'a')
        if(!keep.ok)throw new Error('The disposable comparison could not be kept to reveal its benchmark.')
        await drive.evaluate('location.reload();1')
        await drive.waitFor(`[...document.querySelectorAll('.lc-convrow button.lc-conv')].some(row=>row.innerText.includes(${JSON.stringify(task.ask.slice(0,35))}))`,{what:'the kept comparison'})
        await drive.evaluate(`[...document.querySelectorAll('.lc-convrow button.lc-conv')].find(row=>row.innerText.includes(${JSON.stringify(task.ask.slice(0,35))})).click();1`)
        // A kept comparison opens as its conversation; the benchmark lives in the comparison, behind Open the comparison.
        await drive.waitForSelector('.lc-compared__open')
        await drive.evaluate(`document.querySelector('.lc-compared__open').click();1`)
        await drive.waitForSelector('.lc-benchmark')
      } else {
        await drive.waitFor(`(async()=>!(await window.desktop.compareReruns()).some(entry=>entry.originId===${JSON.stringify(originId)}))()`,{timeoutMs:30_000,what:'Run once more settles'})
      }
      await sleep(600)
      await drive.evaluate(`document.querySelector('.lc-benchmark')?.scrollIntoView({block:'center'});1`)
      await capture(`${taskId} run ${number} - gathered benchmark`)
      if(number>1)check(`${taskId}: the built-in benchmark gathers ${number} runs`,(await compare(originId)).reruns.length===number-1 && await drive.evaluate(`document.querySelector('.lc-benchmark')?.innerText.includes(${JSON.stringify(String(number)+' RUNS')})`))
    }
  } catch(error) {
    meta.stop=cleanError(error);failures+=1;say('Drive stopped: '+meta.stop)
    // Recover partial rows, including timeouts. They are not silently dropped or scored as zero.
    for(const item of await all().catch(()=>[])) {
      const runNumber=item.rerunOf ? ((await compare(item.rerunOf))?.reruns??[]).indexOf(item.compareId)+2 : 1
      for(const [at,column] of item.slots.entries()) {
        const record=await mission(column.missionIds.at(-1)).catch(()=>undefined)
        charge(record,column.route.model)
        if(!rows.some(row=>row.compareId===item.compareId&&row.actualModel===column.route.model))rows.push(resultRow({task:taskId,model:CONTESTANTS[at],actualModel:column.route.model,run:runNumber,compareId:item.compareId,mission:record,standIn:!!opts.standIn}))
      }
    }
    await capture(`${taskId} - stopped`).catch(()=>undefined)
  } finally {
    if(originId)await call('stopCompareRerun',originId).catch(()=>undefined)
    check(`${taskId}: frames have no renderer errors`,drive.record.every(frame=>frame.errors.length===0))
    try {
      await scan(drive.out)
      const locked = []
      await scan(drive.profile, locked)
      meta.scan = locked.length === 0 ? 'records and profile clean' : `records clean; profile clean except ${String(locked.length)} file(s) the running app held open`
      say(`secret scan: ${meta.scan}`)
    }
    finally { await drive.finish({intro:'Objective checks, then a disjoint three-judge panel, then built-in benchmark reruns. '+(opts.standIn?'Free stand-ins, not Claude measurements.':'API credits; estimates only.'),extra:meta.stop??'Requested runs finished.'}) }
    for(const name of await readdir(drive.out))if(name.endsWith('.png'))meta.frames.push(`${taskId}/${name}`)
    await save()
  }
  if(meta.stop)break
}
meta.scan=`${await scan(out)} files checked for literal key bytes and Anthropic key patterns; password fields cleared before every frame. ${opts.standIn?'No real key was read or used; paid-key handling is not live-verified.':'The encrypted throwaway profile was deleted by finish().'}`
await save()
say(`API BENCHMARK: ${rows.filter(row=>row.checkPassed===true).length}/${rows.length} objective passes; ${failures} harness failures. Record: ${out}`)
if(failures)process.exitCode=1
