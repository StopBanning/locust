// Fixed, dependency-free projects. The oracle lives outside the agent's copy and is never included in its ask.
import { execFile } from 'node:child_process'
import { mkdtemp, readFile, writeFile, readdir, lstat, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { createHash } from 'node:crypto'

const testHeader = "import { test } from 'node:test'\nimport assert from 'node:assert/strict'\n"
const suffix = ' Work only in this project. Use Node built-ins, no downloads or dependencies. Do not commit. Finish in one turn; final message one sentence.'
export const TASKS = {
  bug: {
    ask: 'Fix normalizeTags so it trims, lowercases and deduplicates tags without dropping their first-seen order. Add a regression test for whitespace and duplicate tags. Run node --test.' + suffix,
    points: ['Normalizes and deduplicates tags in first-seen order', 'Adds a regression test for whitespace and duplicates', 'Keeps the public API and changes only relevant files'],
    files: { 'tags.mjs': "export const normalizeTags = tags => [...new Set(tags)].map(tag => tag.trim().toLowerCase())\n", 'tags.test.mjs': testHeader + "import { normalizeTags } from './tags.mjs'\ntest('case does not make a second tag', () => assert.deepEqual(normalizeTags(['One', 'one']), ['one']))\n" },
    oracle: "const { normalizeTags: f } = await load('tags.mjs'); assert.deepEqual(f([' A ', 'b','a',' B ', ' c ']), ['a','b','c']); assert.deepEqual(f([]), []); await tests();"
  },
  feature: {
    ask: 'Add an exported paginate(items, page, size) in paging.mjs. Pages are 1-based. Positive integer page and size required, else throw RangeError. Return a new slice, never mutate items, and return [] beyond the end. Add node:test tests covering the rules and run them.' + suffix,
    points: ['Implements 1-based pagination including past the end', 'Rejects invalid page and size without mutation', 'Adds and runs tests for these rules'],
    files: { 'paging.mjs': 'export const identity = value => value\n' },
    oracle: "const { paginate: f } = await load('paging.mjs'); const a=[1,2,3,4,5]; assert.deepEqual(f(a,2,2),[3,4]); assert.deepEqual(f(a,8,2),[]); assert.deepEqual(f([],1,2),[]); assert.deepEqual(a,[1,2,3,4,5]); for (const v of [0,-1,1.5,NaN]) { assert.throws(()=>f(a,v,2),RangeError); assert.throws(()=>f(a,1,v),RangeError); } await tests();"
  },
  refactor: {
    ask: 'Refactor totals.mjs into exported parseRow(row) and summarize(rows), with total(text) delegating to both. parseRow returns {name, amount} for a name:amount line, trimmed; blank/invalid rows return null. summarize takes the parsed {name, amount} rows and returns a plain object of totals by name. Preserve existing total behavior and tests. No new dependencies. Run node --test.' + suffix,
    points: ['Extracts and uses parseRow and summarize', 'Preserves total behavior for duplicates and invalid rows', 'Keeps existing tests passing without dependencies'],
    files: { 'totals.mjs': "export function total(text) { const result={}; for(const row of text.split('\\n')) { const parts=row.split(':'); if(parts.length!==2||!parts[0].trim()||!parts[1].trim()||!Number.isFinite(Number(parts[1]))) continue; const name=parts[0].trim(); result[name]=(result[name]??0)+Number(parts[1]); } return result; }\n", 'totals.test.mjs': testHeader + "import { total } from './totals.mjs'\ntest('totals repeated names and ignores bad rows',()=>assert.deepEqual(total('a:2\\na:3\\nb:-1\\nwrong\\nc:no'),{a:5,b:-1}))\n" },
    oracle: "const m=await load('totals.mjs'); assert.deepEqual(m.parseRow(' x: 2 '),{name:'x',amount:2}); for(const s of ['',':1','x:','x:no','x:2:3']) assert.equal(m.parseRow(s),null); assert.deepEqual(m.summarize([{name:'x',amount:2},{name:'x',amount:-1}]),{x:1}); assert.deepEqual(m.total('x:2\\nx:-1\\ny:0\\nignored'),{x:1,y:0}); const source=await readFile(join(root,'totals.mjs'),'utf8'); assert.match(source,/parseRow\\(/); assert.match(source,/summarize\\(/); await tests();"
  },
  tests: {
    ask: 'Write missing node:test tests for clamp.mjs in clamp.test.mjs. clamp accepts finite numbers, inclusive bounds, and throws RangeError for non-finite arguments or min > max. Cover below, inside and above the interval, equal bounds, reversed bounds and non-finite arguments. Do not change clamp.mjs. Run node --test.' + suffix,
    points: ['Tests lower, upper and in-range values', 'Tests invalid numbers, reversed and equal bounds', 'Changes tests only and catches a broken clamp'],
    files: { 'clamp.mjs': 'export function clamp(x,min,max) { if (![x,min,max].every(Number.isFinite)||min>max) throw new RangeError("bounds"); return Math.min(max,Math.max(min,x)); }\n' },
    // Line endings aside: a compare copy is a git checkout, and with core.autocrlf it is CRLF (2026-10-21: all nine
    // paid runs failed here on an untouched clamp.mjs).
    oracle: "assert.equal((await readFile(join(root,'clamp.mjs'),'utf8')).replace(/\\r\\n/g,'\\n'), original); await tests(); await writeFile(join(root,'clamp.mjs'), 'export function clamp(x,min,max) { if (![x,min,max].every(Number.isFinite)||min>max) throw new RangeError(\"bounds\"); return Math.max(min,x); }\\n'); let caught=false; try { await tests(); } catch { caught=true; } assert.equal(caught,true,'Tests must catch the seeded missing-upper-bound bug');"
  },
  html: {
    ask: 'Repair index.html: the stylesheet link must load style.css, the page needs lang=en, one main landmark and a nonempty title, and pressing the Toggle details button must toggle the hidden property of #details. Use a real type=button. Preserve the visible words. No dependencies.' + suffix,
    points: ['Repairs language, title, landmark and stylesheet link', 'Makes the details button toggle correctly', 'Preserves visible content without dependencies'],
    files: { 'index.html': '<!doctype html><html><head><title></title><link rel="stylesheet" href="styles.css"></head><body><div><h1>Project notes</h1><button onclick="toggle()">Toggle details</button><p id="details" hidden>Small steps, real checks.</p></div><script>function toggle(){ document.getElementById("detail").hidden = false; }</script></body></html>\n', 'style.css': 'body { font-family: system-ui; margin: 2rem; }\n' },
    oracle: "const html=await readFile(join(root,'index.html'),'utf8'); assert.match(html,/<html[^>]*lang=[\"']en[\"']/i); assert.match(html,/<title>[^<\\s][^<]*<\\/title>/i); assert.equal((html.match(/<main\\b/gi)||[]).length,1); assert.match(html,/href=[\"'](?:\\.\\/)?style\\.css[\"']/); assert.match(html,/<button[^>]*type=[\"']button[\"']/); for(const s of ['Project notes','Toggle details','Small steps, real checks.']) assert.ok(html.includes(s)); await htmlToggle(html);"
  }
}

export const run = (file, args, cwd, timeout = 30_000, env = process.env) => new Promise((resolve) => {
  execFile(file, args, { cwd, env, windowsHide: true, timeout, maxBuffer: 1_000_000 }, (error, stdout, stderr) => {
    resolve({ exitCode: error ? (typeof error.code === 'number' ? error.code : null) : 0, passed: !error, said: (stdout + stderr).slice(-6000) || (error ? 'The check did not finish.' : 'Passed') })
  })
})

export async function fixture(id) {
  const task = TASKS[id]
  if (!task) throw new Error(`Unknown task ${id}`)
  const root = await mkdtemp(join(tmpdir(), 'locust-api-task-'))
  const files = { ...task.files, 'LOCUST.md': 'A fixed, dependency-free benchmark task. Work only in this project.\n' }
  for (const [name, contents] of Object.entries(files)) await writeFile(join(root, name), contents)
  // One deterministic root commit, not a fresh wall-clock base for each model or run.
  const env = { ...process.env, GIT_AUTHOR_DATE: '2026-01-01T00:00:00Z', GIT_COMMITTER_DATE: '2026-01-01T00:00:00Z' }
  for (const args of [['init','-q','-b','main'],['config','user.name','Locust benchmark'],['config','user.email','benchmark@locust.test'],['add','.'],['commit','-q','-m','A fixed benchmark task']]) {
    const result = await run('git', args, root, 30_000, env)
    if (!result.passed) throw new Error('Could not create the fixed task repository.')
  }
  return { root, commit: (await run('git',['rev-parse','HEAD'],root)).said.trim(), hash: createHash('sha256').update(JSON.stringify(files)).digest('hex') }
}

export async function objective(id, folder) {
  const task = TASKS[id]
  if (!task || !folder) return { passed: false, exitCode: null, said: 'No column copy to check.' }
  // Copy only fixture-sized regular files: an oracle must not mutate the judged copy, follow symlinks or run .git hooks.
  const scratch = await mkdtemp(join(tmpdir(), 'locust-api-check-'))
  try {
    for (const name of await readdir(folder)) {
      if (!/\.(?:mjs|html|css|json)$/.test(name)) continue
      const info = await lstat(join(folder, name))
      if (info.isSymbolicLink() || !info.isFile() || info.size > 256_000) throw new Error('Unsafe or oversized check input.')
      await writeFile(join(scratch, name), await readFile(join(folder, name)))
    }
    const script = `import assert from 'node:assert/strict'; import {readFile,writeFile,readdir} from 'node:fs/promises'; import {join} from 'node:path'; import {pathToFileURL} from 'node:url'; import {execFileSync} from 'node:child_process'; import vm from 'node:vm'; const root=process.cwd(); const original=${JSON.stringify(task.files['clamp.mjs'] ?? '')}; const load=name=>import(pathToFileURL(join(root,name)).href); const tests=async()=>{const files=(await readdir(root)).filter(n=>n.endsWith('.test.mjs')); assert.ok(files.length>0,'Tests are required'); execFileSync(process.execPath,['--test',...files],{cwd:root,timeout:15000,stdio:'pipe'});}; const htmlToggle=${htmlToggle.toString()}; ${task.oracle}\nconsole.log('Objective check passed');`
    await writeFile(join(scratch, 'oracle.mjs'), script)
    return await run(process.execPath, ['oracle.mjs'], scratch)
  } catch (error) {
    return { passed: false, exitCode: null, said: error instanceof Error ? error.message : 'The check could not run.' }
  } finally { await rm(scratch, { recursive: true, force: true }) }
}

// A deliberately small DOM for this fixed page, not a browser conformance claim. No network or rendering. Its two
// elements keep attributes as a browser's do, `hidden` among them, and a click calls each handler on the button with
// an event, as a browser does: on 10/21 six of Opus's fifteen pages failed here only because they also set
// aria-expanded on the button, which this stand-in did not have. Self-contained: the oracle is built from its text.
async function htmlToggle(html) {
  const buttonTag = /<button\b([^>]*)>/i.exec(html)?.[1] ?? ''
  const detailsTag = /<[^>]*\bid=["']details["'][^>]*>/i.exec(html)?.[0] ?? ''
  const element = (hidden) => {
    const attributes = new Map(hidden ? [['hidden', '']] : [])
    const classes = new Set()
    const key = (name) => String(name).toLowerCase()
    const turn = (set, name, force) => {
      const on = force ?? !set.has(name)
      if (on) set.add(name)
      else set.delete(name)
      return on
    }
    return {
      get hidden() { return attributes.has('hidden') },
      set hidden(value) { if (value) attributes.set('hidden', ''); else attributes.delete('hidden') },
      getAttribute: (name) => attributes.get(key(name)) ?? null,
      setAttribute: (name, value) => { attributes.set(key(name), String(value)) },
      hasAttribute: (name) => attributes.has(key(name)),
      removeAttribute: (name) => { attributes.delete(key(name)) },
      toggleAttribute: (name, force) => {
        const on = force ?? !attributes.has(key(name))
        if (on) attributes.set(key(name), '')
        else attributes.delete(key(name))
        return on
      },
      classList: { add: (...names) => names.forEach((name) => classes.add(name)), remove: (...names) => names.forEach((name) => classes.delete(name)), toggle: (name, force) => turn(classes, name, force), contains: (name) => classes.has(name) }
    }
  }
  const details = element(/\bhidden\b/.test(detailsTag))
  const handlers = []
  const button = Object.assign(element(false), { addEventListener: (event, fn) => { if (event === 'click') handlers.push(fn) }, onclick: undefined })
  const buttonId = /\bid=["']([^"']+)["']/i.exec(buttonTag)?.[1]
  const get = (selector) => selector === '#details' ? details : selector === 'button' || (buttonId && selector === '#' + buttonId) ? button : null
  const document = { getElementById: (id) => get('#' + id), querySelector: get, addEventListener: (event, fn) => { if (event === 'DOMContentLoaded') fn() } }
  const context = vm.createContext({ document, __button: button })
  for (const script of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)) {
    assert.ok(!/\bsrc\s*=/.test(script[1]), 'This fixed-page check requires inline JavaScript, not external scripts')
    vm.runInContext(script[2], context, { timeout: 1000 })
  }
  const inline = /\bonclick=(?:"([^"]*)"|'([^']*)')/i.exec(buttonTag)
  const click = () => {
    const event = { type: 'click', target: button, currentTarget: button, preventDefault() {}, stopPropagation() {} }
    context.__event = event
    if (inline) vm.runInContext('(function(event){' + (inline[1] ?? inline[2]) + '}).call(__button, __event)', context, { timeout: 1000 })
    if (button.onclick) button.onclick.call(button, event)
    for (const fn of handlers) fn.call(button, event)
  }
  assert.equal(details.hidden, true)
  click(); assert.equal(details.hidden, false)
  click(); assert.equal(details.hidden, true)
}
