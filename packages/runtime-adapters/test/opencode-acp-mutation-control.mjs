// Focused controls for the opt-in OpenCode ACP profile and its lifecycle seam.
// Run from the package: node test/opencode-acp-mutation-control.mjs [name filter]
// No installed agent, network, credentials, shell, or production build is used.
import { spawnSync } from 'node:child_process'
import { randomUUID } from 'node:crypto'
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const PROFILE = join(ROOT, 'src', 'opencode-acp-run.ts')
const ACP = join(ROOT, 'src', 'acp-run.ts')
const COMMANDS = join(ROOT, 'src', 'opencode-commands.ts')
const DESKTOP = join(ROOT, '..', '..', 'apps', 'desktop')
const BRIDGE = join(DESKTOP, 'src', 'main', 'app-server-process.ts')
const VITEST = join(ROOT, '..', '..', 'node_modules', 'vitest', 'vitest.mjs')
const REPORT = join(tmpdir(), `locust-opencode-acp-mutations-${process.pid}-${randomUUID()}.json`)
const FILES = [
  'test/opencode-acp-profile.test.ts',
  'test/acp-run.test.ts',
  'test/opencode-commands.test.ts',
  'test/an-acp-output-stream-closing-ends-the-turn.test.ts'
]

const MUTATIONS = [
  {
    file: COMMANDS,
    name: 'the V2 command menu starts the removed V1 server',
    from: 'if (openCodeUsesAcp(options.version)) return Promise.resolve([]);',
    to: 'if (false) return Promise.resolve([]);',
    expect: 'never starts the V1 command server when OpenCode 2 is installed'
  },
  {
    suite: 'native', file: BRIDGE,
    name: 'native: the pending UTF8 decoder is not flushed before EOF',
    from: 'for (const flush of flushDecoders) flush()',
    to: 'void flushDecoders',
    expect: 'flushes the pending decoder before reporting EOF once across end, close and error'
  },
  {
    suite: 'native', file: BRIDGE,
    name: 'native: repeated close events are not guarded',
    from: 'const endStdout = (error?: Error): void => {\n    if (stdoutEnded) return',
    to: 'const endStdout = (error?: Error): void => {\n    if (false) return',
    expect: 'flushes the pending decoder before reporting EOF once across end, close and error'
  },
  {
    suite: 'native', file: BRIDGE,
    name: 'native: an early stdout error loses its diagnostic',
    from: 'stdoutError = error', to: 'stdoutError = undefined',
    expect: 'reports stdout errors to a listener registered after the stream failed'
  },
  {
    suite: 'native', file: BRIDGE,
    name: 'native: a rejected tree release never stops the child',
    from: '}).catch(killChild)', to: '}).catch(() => undefined)',
    expect: 'falls back to killing the child when releasing its tree rejects'
  },
  {
    file: PROFILE,
    name: 'the ACP command drifts to another runtime identity',
    from: '...base,\n    runtime: "opencode",',
    to: '...base,\n    runtime: "copilot",',
    expect: 'keeps OpenCode identity, launch prefix and environment on the stdio command'
  },
  {
    file: PROFILE,
    name: 'the desktop permission environment is lost at spawn',
    from: 'createOpenCodeAcpCommand(options.executable, options.workspacePath, options.launch)',
    to: 'createOpenCodeAcpCommand(options.executable, options.workspacePath)',
    expect: 'passes the desktop launch environment and permission receipt through the profile'
  },
  {
    file: ACP,
    name: 'a model selection retains stale session config options',
    from: 'session = { ...session, ...set };',
    to: 'void set;',
    expect: 'reads effort options advertised after switching the selected model'
  },
  {
    file: PROFILE,
    name: 'a prompt write is not recorded as admitted',
    from: 'admitted = true;',
    to: 'admitted = false;',
    expect: 'loads an ACP v1 session without recording its replayed history'
  },
  {
    file: PROFILE,
    name: 'an admitted prompt is eligible for transport retry',
    from: 'if (!admitted && transportClosed && attempts <= maxRetries) {',
    to: 'if (transportClosed && attempts <= maxRetries) {',
    expect: 'never resends an admitted prompt after stdout closes during its tool call'
  },
  {
    file: PROFILE,
    name: 'the startup retry cap is ignored',
    from: 'const maxRetries = bound(options.maxStartupRetries, 2, 3, 0);',
    to: 'const maxRetries = bound(options.maxStartupRetries, 2, 100, 0);',
    expect: 'exhausts capped startup retries with a structured transport failure'
  },
  {
    file: PROFILE,
    name: 'a replacement starts before the startup backoff',
    from: 'await wait(Math.min(retryDelay * 2 ** (attempts - 1), maxRetryDelay));',
    to: 'await wait(1);',
    expect: 'retries a pre-prompt stdout close only after bounded startup backoff'
  },
  {
    file: PROFILE,
    name: 'a replacement is allowed without parent exit acknowledgement',
    from: 'if (!exited) { finish(receipt, "termination-unconfirmed",',
    to: 'if (false) { finish(receipt, "termination-unconfirmed",',
    expect: 'refuses replacement when the stopped process never acknowledges exit'
  },
  {
    file: PROFILE,
    name: 'the total startup deadline loses its failure classification',
    from: 'timedOut = true;\n    stop();',
    to: 'timedOut = false;\n    stop();',
    expect: 'bounds the total startup deadline even when retry backoff remains'
  },
  {
    file: PROFILE,
    name: 'the startup clock keeps running after prompt admission',
    from: 'admitted = true;\n                  clearTimeout(startupTimer);',
    to: 'admitted = true;\n                  void startupTimer;',
    expect: 'clears the startup deadline when a long-running prompt is admitted'
  },
  {
    file: PROFILE,
    name: 'an unbounded handshake option reaches the inner timer',
    from: 'handshakeTimeoutMs: startupBudget,',
    to: 'handshakeTimeoutMs: options.handshakeTimeoutMs,',
    expect: 'bounds a non-finite handshake option without overflowing the inner timer'
  },
  {
    file: PROFILE,
    name: 'an abandoned startup session receipt leaks into the record',
    from: 'else buffered.push(record);',
    to: 'else emit(record);',
    expect: 'keeps queued steering and excludes abandoned session receipts on a startup retry'
  },
  {
    file: PROFILE,
    name: 'a failed permission reply escapes the response write boundary',
    from: 'if (message.method === undefined) {',
    to: 'if (false) {',
    expect: 'settles a failed permission reply without an unhandled rejection or a second write'
  },
  {
    file: ACP,
    name: 'stdout EOF wins over the final response in its last data chunk',
    from: 'queueMicrotask(() => lose(error === undefined',
    to: '((work: () => void) => work())(() => lose(error === undefined',
    expect: 'keeps the final prompt answer in the last chunk before EOF'
  },
  {
    file: ACP,
    name: 'a closed stdout can advance the pending handshake',
    from: 'if (settled || cancelled || transportEnded) throw new Error(',
    to: 'if (settled || cancelled) throw new Error(',
    expect: 'does not begin a session from an initialize answer followed by EOF'
  },
  {
    file: ACP,
    name: 'a finished cancellation retains its grace timer',
    from: 'clearTimeout(cancelTimer);',
    to: 'void cancelTimer;',
    expect: 'clears the cancellation grace timer when the prompt answers'
  },
  {
    file: PROFILE,
    name: 'record overflow is not reported as an output limit',
    from: 'if (overflowed || receipt.outputLimitExceeded) { finish(receipt, "output-limit",',
    to: 'if (false) { finish(receipt, "output-limit",',
    expect: 'stops on record overflow without replacing the admitted process'
  },
  {
    file: PROFILE,
    name: 'an unadvertised required model silently falls back',
    from: 'strictConfig: true,',
    to: 'strictConfig: false,',
    expect: 'refuses an unadvertised model instead of silently changing the requested route'
  },
  {
    file: PROFILE,
    name: 'pre-prompt staged receipts lose their memory boundary',
    from: 'if (buffered.length >= queueLimit) { overflowed = true; stop(); }',
    to: 'if (false) { overflowed = true; stop(); }',
    expect: 'bounds staged pre-prompt permission receipts even when the inner queue keeps draining'
  }
]

function runSuite(suite) {
  rmSync(REPORT, { force: true })
  const run = spawnSync(process.execPath, [
    VITEST, 'run', ...(suite === 'native' ? ['src/main/an-app-server-output-close-is-reported-once.test.ts'] : FILES), '--maxWorkers=2', '--reporter=json', `--outputFile=${REPORT}`
  ], { cwd: suite === 'native' ? DESKTOP : ROOT, encoding: 'utf8', shell: false, timeout: 180_000, maxBuffer: 20 * 1024 * 1024 })
  const output = `${run.stdout ?? ''}\n${run.stderr ?? ''}`
  if (run.error !== undefined || run.signal !== null || !existsSync(REPORT)) {
    return { invalid: run.error?.message ?? `Vitest did not write a complete report (exit ${String(run.status)}).`, output }
  }
  let report
  try {
    report = JSON.parse(readFileSync(REPORT, 'utf8'))
  } catch {
    return { invalid: 'Vitest wrote an unreadable JSON report.', output }
  }
  const assertions = []
  for (const file of report.testResults ?? []) {
    const found = file.assertionResults ?? []
    if (found.length === 0 && file.status === 'failed') {
      return { invalid: `A test file failed before collecting assertions: ${file.name}`, output }
    }
    for (const assertion of found) {
      assertions.push({ ...assertion, identity: `${file.name}\u0000${assertion.fullName ?? assertion.title}` })
    }
  }
  if (assertions.length === 0 || assertions.length !== report.numTotalTests) {
    return { invalid: 'The report did not contain the full collected test inventory.', output }
  }
  if (assertions.some((assertion) => assertion.status !== 'passed' && assertion.status !== 'failed') ||
      (report.numPendingTests ?? 0) !== 0 || (report.numTodoTests ?? 0) !== 0) {
    return { invalid: 'A focused regression was skipped or did not run.', output }
  }
  const failed = assertions.filter((assertion) => assertion.status === 'failed')
  if ((run.status === 0) !== (failed.length === 0)) {
    return { invalid: 'The process exit and named assertion results disagree.', output }
  }
  return {
    status: run.status,
    assertions,
    failed,
    identities: assertions.map((assertion) => assertion.identity).sort(),
    output
  }
}

function main() {
  const filter = process.argv[2]
  const selected = filter === undefined ? MUTATIONS : MUTATIONS.filter((mutation) => mutation.name.includes(filter))
  if (selected.length === 0) throw new Error('No mutation controls match the filter.')
  if (!existsSync(VITEST)) throw new Error('Install the workspace dependencies before running controls.')
  const originals = new Map([...new Set(selected.map((mutation) => mutation.file))].map((file) => [file, readFileSync(file, 'utf8')]))
  let problems = 0
  let caughtCount = 0
  try {
    const baselines = new Map()
    for (const mutation of selected) {
      const suite = mutation.suite ?? 'adapter'
      let baseline = baselines.get(suite)
      if (baseline === undefined) {
        baseline = runSuite(suite)
        if (baseline.invalid !== undefined || baseline.status !== 0 || baseline.failed.length !== 0) {
          throw new Error(`Baseline is not fully green: ${baseline.invalid ?? baseline.failed.map((assertion) => assertion.title).join(', ')}\n${baseline.output}`)
        }
        baselines.set(suite, baseline)
        console.error(`BASELINE ${suite}: ${baseline.assertions.length} tests passed, zero skips.`)
      }
      const original = originals.get(mutation.file)
      const occurrences = original.split(mutation.from).length - 1
      if (occurrences !== 1) {
        console.error(`[INVALID] ${mutation.name}: expected one source anchor, found ${occurrences}.`)
        problems += 1
        continue
      }
      if (baseline.assertions.filter((assertion) => assertion.title === mutation.expect).length !== 1) {
        console.error(`[INVALID] ${mutation.name}: required named test is missing or ambiguous: ${mutation.expect}`)
        problems += 1
        continue
      }
      let result
      try {
        writeFileSync(mutation.file, original.replace(mutation.from, mutation.to), 'utf8')
        result = runSuite(suite)
      } finally {
        writeFileSync(mutation.file, original, 'utf8')
      }
      if (result.invalid !== undefined || JSON.stringify(result.identities) !== JSON.stringify(baseline.identities)) {
        console.error(`[INVALID] ${mutation.name}: ${result.invalid ?? 'The collected test inventory changed.'}\n${result.output}`)
        problems += 1
        continue
      }
      const caught = result.status !== 0 && result.failed.some((assertion) => assertion.title === mutation.expect && (assertion.failureMessages?.length ?? 0) > 0)
      if (caught) caughtCount += 1
      else problems += 1
      console.error(`[${caught ? 'CAUGHT' : 'SURVIVED'}] ${mutation.name}`)
      if (!caught) {
        console.error(`Expected "${mutation.expect}" to fail; observed: ${result.failed.map((assertion) => assertion.title).join(', ') || 'none'}`)
      }
    }
  } finally {
    for (const [file, original] of originals) writeFileSync(file, original, 'utf8')
    rmSync(REPORT, { force: true })
  }
  console.error(`${caughtCount}/${selected.length} controls caught; ${problems} survived, missing, skipped or invalid.`)
  return problems === 0 ? 0 : 1
}

try {
  process.exitCode = main()
} catch (error) {
  rmSync(REPORT, { force: true })
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
}
