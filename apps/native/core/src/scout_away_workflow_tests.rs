//! Run the workflow's shipped Node entry against a fake relay and GitHub.
use std::io::Write;
use std::process::{Command, Stdio};

fn workflow_case(case: &str) {
    let node = std::env::var_os("LOCUST_TEST_NODE").unwrap_or_else(|| "node".into());
    let root = std::env::temp_dir().join(format!("locust-scout-workflow-{}", crate::relay_crypto::random_text(12)));
    std::fs::create_dir(&root).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let _cleanup = Remove(root.clone());
    let script = root.join("scout-away.mjs");
    std::fs::write(&script, super::PACKAGED_AWAY_SCRIPT).unwrap();
    let mut child = Command::new(node)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("SCOUT_TEST_CASE", case)
        .env("SCOUT_TEST_SCRIPT", &script)
        .env("GH_REPO", "acme/app")
        .current_dir(&root)
        .args(["--input-type=module", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().expect("run with Node on PATH or set LOCUST_TEST_NODE");
    child.stdin.take().unwrap().write_all(HARNESS.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{case}: {}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}

// case: does nothing more while Locust is open on the PC
#[test]
#[ignore = "requires Node; explicitly run scout_away_workflow_tests with --ignored"]
fn workflow_does_not_query_github_while_the_pc_is_open() { workflow_case("open"); }

// case: only notes what is open the first time, then tells and rings for a check that starts failing
#[test]
#[ignore = "requires Node; explicitly run scout_away_workflow_tests with --ignored"]
fn workflow_records_a_baseline_then_requests_one_check_alert() { workflow_case("first"); }

// case: does not tell again what the PC told before it went off
#[test]
#[ignore = "requires Node; explicitly run scout_away_workflow_tests with --ignored"]
fn workflow_uses_the_last_receipt_from_the_pc() { workflow_case("pc-told"); }

// case: keeps quiet hours, in the person’s own time zone
#[test]
#[ignore = "requires Node; explicitly run scout_away_workflow_tests with --ignored"]
fn workflow_keeps_quiet_hours_in_the_persons_time_zone() { workflow_case("quiet"); }

// case: brings the morning brief once a day from its hour on, and rings for it
#[test]
#[ignore = "requires Node; explicitly run scout_away_workflow_tests with --ignored"]
fn workflow_requests_the_morning_brief_alert_only_once() { workflow_case("brief"); }

// case: says GitHub could not be read, rings nothing, and keeps what was known for the next look
#[test]
#[ignore = "requires Node; explicitly run scout_away_workflow_tests with --ignored"]
fn workflow_preserves_its_previous_receipt_when_github_cannot_be_read() { workflow_case("unreadable"); }

// case: stops quietly once the away look is off in Locust
#[test]
#[ignore = "requires Node; explicitly run scout_away_workflow_tests with --ignored"]
fn workflow_stops_before_github_when_locust_turns_it_off() { workflow_case("off"); }

// case: reads its settings only as Locust wrote them
#[test]
#[ignore = "requires Node; explicitly run scout_away_workflow_tests with --ignored"]
fn workflow_rejects_settings_locust_did_not_write() { workflow_case("settings"); }

const HARNESS: &str = r#"
import assert from 'node:assert/strict'
import cp from 'node:child_process'
import { syncBuiltinESMExports } from 'node:module'
import { pathToFileURL } from 'node:url'
import { webcrypto } from 'node:crypto'

const repo = 'acme/app', url = 'https://github.com/acme/app/pull/7'
const title = 'Secret launch plan', room = 'r'.repeat(22), token = 't'.repeat(43)
const keyBytes = Buffer.alloc(32, 17)
const config = { v: 1, relay: 'https://relay.invalid', room, token, key: keyBytes.toString('base64url'),
  login: 'person', tz: 'America/New_York' }
let clock = '2026-10-23T14:00:00Z'
let pc = false, off = false, readable = true, checks = 'SUCCESS', issues = [], latest
const visits = [], reads = [], alerts = []
const RealDate = Date
globalThis.Date = class extends RealDate {
  constructor(...args) { super(...(args.length === 0 ? [clock] : args)) }
  static now() { return RealDate.parse(clock) }
}

const key = await webcrypto.subtle.importKey('raw', keyBytes, 'AES-GCM', false, ['encrypt', 'decrypt'])
const cryptoArgs = (from, iv) => ({ name: 'AES-GCM', iv,
  additionalData: Buffer.from('locust-away/v1|' + room + '|' + from) })
async function told(prs = [], words = [], extra = {}) {
  const nonce = Buffer.alloc(12, 3)
  const letter = { v: 1, kind: 'told', at: '2026-10-23T13:00:00Z', repo, read: true, found: [], prs, told: words, ...extra }
  const sealed = await webcrypto.subtle.encrypt(cryptoArgs('pc', nonce), key, Buffer.from(JSON.stringify(letter)))
  latest = { from: 'pc', n: nonce.toString('base64url'), c: Buffer.from(sealed).toString('base64url') }
}
async function newest() {
  assert.equal(latest.from, 'away')
  const body = await webcrypto.subtle.decrypt(cryptoArgs('away', Buffer.from(latest.n, 'base64url')), key,
    Buffer.from(latest.c, 'base64url'))
  return JSON.parse(Buffer.from(body).toString())
}
globalThis.fetch = async (address, init) => {
  assert.equal(address, config.relay + '/v1/room/' + room + '/away')
  assert.equal(init.headers.authorization, 'Bearer ' + token)
  visits.push(init.method)
  if (init.method === 'GET') return Response.json({ pc, latest }, { status: off ? 403 : 200 })
  assert.equal(init.method, 'POST')
  const { n, c, alert } = JSON.parse(init.body)
  assert.match(n, /^[A-Za-z0-9_-]+$/)
  assert.match(c, /^[A-Za-z0-9_-]+$/)
  assert.deepEqual(Object.keys(JSON.parse(init.body)).sort(), alert ? ['alert', 'c', 'n'] : ['c', 'n'])
  latest = { from: 'away', n, c }
  if (alert) alerts.push(alert)
  return Response.json({ rung: alert ? 1 : 0 })
}
cp.execFile = (program, args, opts, done) => {
  assert.equal(program, 'gh')
  assert.equal(opts.env.GH_REPO, repo)
  assert.equal(opts.env.GH_PROMPT_DISABLED, '1')
  assert.equal(opts.env.GH_TOKEN, undefined)
  reads.push(args)
  if (!readable) return done(new Error('gh: could not resolve to a repository'))
  let out = []
  if (args[0] === 'pr' && args.includes('--author')) {
    assert.equal(args[args.indexOf('--author') + 1], 'person')
    out = [{ number: 7, title, url, state: 'OPEN', isDraft: false, mergeable: 'MERGEABLE',
      reviewDecision: '', updatedAt: '2026-10-23T12:00:00Z',
      statusCheckRollup: [{ __typename: 'CheckRun', status: 'COMPLETED', conclusion: checks }] }]
  }
  if (args[0] === 'repo') return done(null, 'main\n')
  if (args[0] === 'run') out = [{ databaseId: 1, conclusion: 'success', status: 'completed',
    workflowName: 'Locust Scout', url: 'https://github.com/acme/app/actions/runs/1' }]
  if (args[0] === 'issue') out = issues
  done(null, JSON.stringify(out))
}
syncBuiltinESMExports()

const pr = state => ({ number: 7, title, url, state: 'open', checks: state, checkCount: 1,
  notPassed: state === 'failing' ? 1 : 0 })
const entry = pathToFileURL(process.env.SCOUT_TEST_SCRIPT).href
let call = 0
async function run(secret = JSON.stringify(config)) {
  if (secret === null) delete process.env.LOCUST_SCOUT
  else process.env.LOCUST_SCOUT = secret
  const said = [], saved = console.log
  console.log = (...parts) => said.push(parts.join(' '))
  try { await import(entry + '?run=' + call++) } finally { console.log = saved }
  assert.equal(process.exitCode ?? 0, 0)
  assert.equal(said.length, 1)
  return said[0]
}
switch (process.env.SCOUT_TEST_CASE) {
  case 'open':
    pc = true
    assert.equal(await run(), 'Locust is open on your computer, so Scout looks from there.')
    assert.deepEqual(visits, ['GET'])
    assert.deepEqual(reads, [])
    break
  case 'first': {
    checks = 'FAILURE'
    const first = await run()
    assert.match(first, /^Scout’s first away look: it noted what is open now/)
    assert.deepEqual((await newest()).found, [])
    assert.deepEqual(alerts, [])
    checks = 'SUCCESS'; clock = '2026-10-23T15:00:00Z'
    await run()
    checks = 'FAILURE'; clock = '2026-10-23T16:00:00Z'
    const second = await run()
    assert.equal(second, 'Scout looked: 1 new thing. It rang 1 phone.')
    assert.deepEqual((await newest()).found.map(f => [f.urgency, f.url]), [['now', url]])
    assert.deepEqual(alerts, ['found'])
    assert.ok(!(first + second).includes(title))
    clock = '2026-10-23T17:00:00Z'
    assert.equal(await run(), 'Scout looked: nothing new.')
    assert.deepEqual(alerts, ['found'])
    break
  }
  case 'pc-told':
    checks = 'FAILURE'
    await told([pr('failing')], ['pr:' + url + ':failing::'])
    assert.equal(await run(), 'Scout looked: nothing new.')
    issues = [{ number: 9, title: 'Crash on start', url: 'https://github.com/acme/app/issues/9',
      createdAt: '2026-10-23T13:30:00Z', author: { login: 'someone' } }]
    clock = '2026-10-23T15:00:00Z'
    assert.equal(await run(), 'Scout looked: 1 new thing.')
    assert.deepEqual((await newest()).found.map(f => f.key.slice(0, 6)), ['issue:'])
    assert.deepEqual(alerts, [])
    break
  case 'quiet':
    config.quiet = { from: 22, to: 7 }
    await told([pr('passing')])
    checks = 'FAILURE'; clock = '2026-10-24T03:00:00Z'
    assert.equal(await run(), 'Scout looked: 1 new thing. Quiet hours: no phone was rung.')
    assert.deepEqual(alerts, [])
    assert.equal((await newest()).found.length, 1)
    break
  case 'brief': {
    config.briefAt = 7
    await told([], [], { at: '2026-10-23T02:00:00Z', briefDay: '2026-10-22' })
    clock = '2026-10-23T10:00:00Z'
    assert.equal(await run(), 'Scout looked: nothing new.')
    clock = '2026-10-23T11:30:00Z'
    assert.equal(await run(), 'Scout looked: nothing new. It brought the morning brief. It rang 1 phone.')
    const brief = (await newest()).found.find(f => f.key === 'brief:2026-10-23')
    assert.ok(brief.lines.includes('Your computer is off, so Scout looked at GitHub only.'))
    assert.deepEqual(alerts, ['brief'])
    clock = '2026-10-23T12:30:00Z'
    assert.equal(await run(), 'Scout looked: nothing new.')
    assert.deepEqual(alerts, ['brief'])
    break
  }
  case 'unreadable': {
    await told([pr('passing')], ['x'])
    readable = false
    assert.equal(await run(), 'Scout could not read GitHub this time.')
    const receipt = await newest()
    assert.equal(receipt.read, false)
    assert.deepEqual(receipt.prs.map(p => p.number), [7])
    assert.deepEqual(receipt.told, ['x'])
    assert.deepEqual(alerts, [])
    break
  }
  case 'off':
    off = true
    assert.match(await run(), /^Scout’s away look is off in Locust\./)
    assert.deepEqual(visits, ['GET'])
    assert.deepEqual(reads, [])
    break
  case 'settings':
    pc = true
    config.tz = 'UTC'
    config.quiet = { from: 22, to: 7 }
    config.briefAt = 7
    assert.equal(await run(), 'Locust is open on your computer, so Scout looks from there.')
    for (const bad of [null, '', 'not json', JSON.stringify({ ...config, v: 2 }),
      JSON.stringify({ ...config, relay: 'https://evil.example/path' }),
      JSON.stringify({ ...config, login: 'a b' })]) {
      assert.match(await run(bad), /^The LOCUST_SCOUT secret is missing or cannot be read/)
    }
    assert.deepEqual(visits, ['GET'])
    assert.deepEqual(reads, [])
    break
  default: throw new Error('no fixture selected')
}
"#;
