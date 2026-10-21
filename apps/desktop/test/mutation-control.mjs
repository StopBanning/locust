// Mutation control for the renderer's status derivations and the approval layer.
//
// These functions decide whether the shell may call a runtime live, whether a
// teammate reads as blocked, and whether a receipt prints `verified`. The
// product's entire claim is that those words are trustworthy, so a green suite
// over them is not enough -- each invariant must be shown to fail when broken.
//
//   node test/mutation-control.mjs
//
// Breaks one behaviour at a time, requires the NAMED test to fail, rejects any
// mutation that stops the file running, and restores every file it touches.

import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { dirname, join } from 'node:path'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))
const STATUS = join(ROOT, 'src', 'renderer', 'src', 'status.ts')
const APPROVALS = join(ROOT, 'src', 'main', 'app-server-mission.ts')
const HANDOFF = join(ROOT, 'src', 'main', 'handoff.ts')
const MISSIONS = join(ROOT, 'src', 'main', 'codex-mission.ts')
const PEERS = join(ROOT, 'src', 'main', 'peer-exchange.ts')
const BRIEFING = join(ROOT, 'src', 'main', 'workroom-briefing.ts')
const SHARE = join(ROOT, 'src', 'shared', 'peer-share.ts')
const DECISION = join(ROOT, 'src', 'shared', 'decision.ts')
const RESUME = join(ROOT, 'src', 'renderer', 'src', 'resume.ts')
const CMDLEN = join(ROOT, 'src', 'main', 'command-length.ts')
const CMDREC = join(ROOT, 'src', 'main', 'command-record.ts')
const VIEW = join(ROOT, 'src', 'renderer', 'src', 'missionView.ts')
const TEAMMATES = join(ROOT, 'src', 'main', 'teammate-store.ts')
const CATALOG = join(ROOT, 'src', 'main', 'model-catalog.ts')
const HISTORY = join(ROOT, 'src', 'main', 'mission-history.ts')
const RUNTIMES = join(ROOT, 'src', 'shared', 'runtimes.ts')
const ROSTER = join(ROOT, 'src', 'main', 'teammate-store.ts')
const UPDATES = join(ROOT, 'src', 'main', 'updates.ts')
const RELAY = join(ROOT, 'src', 'main', 'relay.ts')
const FACES = join(ROOT, 'src', 'renderer', 'src', 'faceState.ts')
const ANTIGRAVITY = join(ROOT, 'src', 'main', 'antigravity-mission.ts')
const COST = join(ROOT, 'src', 'renderer', 'src', 'cost.ts')
const AGENT_TEXT = join(ROOT, 'src', 'renderer', 'src', 'agentText.ts')
const TEAMMATE_WORK = join(ROOT, 'src', 'renderer', 'src', 'teammateWork.ts')
const CATALOG_MODELS = join(ROOT, 'src', 'main', 'model-catalog.ts')
const ROUTINE_RUNNER = join(ROOT, 'src', 'main', 'routine-runner.ts')
const ROUTINE_STORE = join(ROOT, 'src', 'main', 'routine-store.ts')
const ROUTINES_VIEW = join(ROOT, 'src', 'renderer', 'src', 'routines.ts')
const STEERING = join(ROOT, 'src', 'renderer', 'src', 'steering.ts')
const WHO_DECIDES = join(ROOT, 'src', 'shared', 'who-decides.ts')
const APPROVAL_RULES = join(ROOT, 'src', 'shared', 'approval-rules.ts')

const MCP_TOOLS = join(ROOT, 'src', 'main', 'locust-mcp-tools.ts')
const MEMORY_STORE = join(ROOT, 'src', 'main', 'memory-store.ts')
const MEMORY_READER = join(ROOT, 'src', 'main', 'memory-reader.ts')
const NAMED_FILES = join(ROOT, 'src', 'main', 'named-files.ts')
const REVEAL = join(ROOT, 'src', 'main', 'reveal-file.ts')
const HANDOVER = join(ROOT, 'src', 'shared', 'handover.ts')
const ORG_POLICY = join(ROOT, 'src', 'main', 'organization-policy.ts')
const ORG_RULES = join(ROOT, 'src', 'shared', 'organization-policy.ts')
const ORG_SPEND = join(ROOT, 'src', 'main', 'organization-spend.ts')
const ORG_MCP = join(ROOT, 'src', 'main', 'locust-mcp-host.ts')
const ORG_PICKER = join(ROOT, 'src', 'renderer', 'src', 'components', 'RoutePicker.tsx')
const ORG_COMPOSER = join(ROOT, 'src', 'renderer', 'src', 'components', 'Composer.tsx')
const ORG_FILE_TEST = 'src/main/an-organization-policy-is-read-before-work-starts.test.ts'
const ORG_START_TEST = 'src/main/a-managed-turn-is-refused-before-an-agent-starts.test.ts'
const ORG_SPEND_TEST = 'src/main/a-person-cannot-lift-the-organizations-spend-limit.test.ts'
const ORG_VIEW_TEST = 'src/renderer/src/managed-controls-say-who-set-them.test.tsx'

const MUTATIONS = [
  ...[
    ['own model route loses its prefix', '../../_tools/api-benchmark.mjs', 'model: `own-${model.ownId}/${model.model}`', 'model: `${model.ownId}/${model.model}`', 'refuses paid judges in stand-in mode and invalid limits before starting anything', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['replacement messages are duplicated', '../../_tools/api-benchmark.mjs', "operation === 'replace' ? text", "operation === 'never' ? text", 'reads normalized replacement text once and measures wall time even when no run-start event was emitted', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['missing run-start loses the clock', '../../_tools/api-benchmark.mjs', "started?.occurredAt ?? mission?.createdAt ?? ''", "started?.occurredAt ?? ''", 'reads normalized replacement text once and measures wall time even when no run-start event was emitted', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['a foreign frame releases checks', 'src/main/index.ts', "return fromOwnWindow(event) && typeof originId === 'string' && typeof compareId === 'string' && compareReruns.checked(originId, compareId)", "return typeof originId === 'string' && typeof compareId === 'string' && compareReruns.checked(originId, compareId)", 'refuses a check release from another frame or malformed ids before touching the runner', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['prices change', '../../_tools/api-benchmark.mjs', 'input: 4, output: 20', 'input: 40, output: 20', 'uses explicit standard API rates, the conservative Haiku tier and separately priced caches', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['spend equality starts more work', '../../_tools/api-benchmark.mjs', 'return total >= maximum', 'return total > maximum', 'stops before another group at the spend boundary and fails closed on unknown paid usage', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['unknown paid usage is free', '../../_tools/api-benchmark.mjs', 'if (charges.some((charge) => !finite(charge)))', 'if (false)', 'stops before another group at the spend boundary and fails closed on unknown paid usage', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['missing counts become zero', '../../_tools/api-benchmark.mjs', 'inputTokens: finite(usage?.inputTokens) ? usage.inputTokens : null', 'inputTokens: finite(usage?.inputTokens) ? usage.inputTokens : 0', 'keeps objective checks before scores and failed or unreported measurements null, not zero', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['stand-in allows API judges', '../../_tools/api-benchmark.mjs', "(picked.standIn && judges !== 'free')", 'false', 'refuses paid judges in stand-in mode and invalid limits before starting anything', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['an oracle accepts broken work', '../../_tools/api-benchmark-tasks.mjs', 'return await run(process.execPath, [\'oracle.mjs\'], scratch)', "return { passed: true, exitCode: 0, said: 'passed' }", 'fixes the files and base for every run, and each oracle rejects broken work but accepts a known solution', 'src/main/an-api-benchmark-prices-and-checks-each-run.test.mjs'],
    ['reruns skip the check pause', 'src/main/compare-reruns.ts', 'if (answered && !batch.stopped && batch.waitForChecks)', 'if (false)', 'holds a measured runs copies before judging, accepts only that run once, and skips automatic retries', 'src/main/a-comparison-runs-again.test.ts'],
    ['reruns retry without a budget check', 'src/main/compare-reruns.ts', '!batch.stopped && !batch.waitForChecks', '!batch.stopped', 'holds a measured runs copies before judging, accepts only that run once, and skips automatic retries', 'src/main/a-comparison-runs-again.test.ts'],
    ['wrong run can release a pause', 'src/main/compare-reruns.ts', 'batch.progress.compareId !== compareId', 'false', 'holds a measured runs copies before judging, accepts only that run once, and skips automatic retries', 'src/main/a-comparison-runs-again.test.ts']
  ].map(([name, file, from, to, expected, test]) => ({ name: `api-benchmark: ${name}`, file: join(ROOT, file), from, to, expect: expected, tests: [test] })),
  ...[
    ['column evidence is dropped', 'main/index.ts', '...(diff === undefined ? {} : { diff })', '...{}', 'collects changes once before starting any judge and gives the panel the same material'],
    ['boundary: every letter reads one column', 'main/index.ts', 'folder: workspacePath, compareId: compare.compareId, slot: column.slot,', "folder: workspacePath, compareId: compare.compareId, slot: 'a',", 'collects changes once before starting any judge and gives the panel the same material'],
    ['boundary: binary bodies are inlined', 'main/compare-copies.ts', "patch.includes('\\0') || /^(?:Binary files |GIT binary patch|Binary file;)/m.test(patch)", 'false', 'names tracked and new binary files without their bodies, even when attributes force text'],
    ['boundary: a failed read is an empty change', 'main/compare-copies.ts', "failed.exitCode === 1 && typeof failed.stdout === 'string' && failed.stdout.length > 0 && (failed.stderr ?? '').trim().length === 0", "failed.exitCode === 1 && typeof failed.stdout === 'string'", 'refuses a failed diff or an unsafe path instead of passing partial output to the judge'],
    ['diffs start at HEAD instead of the saved base', 'main/compare-copies.ts', "await git(['diff', ...flags, input.base, '--',", "await git(['diff', ...flags, 'HEAD', '--',", 'includes committed, staged, unsaved, new and deleted work against the saved base, never the moving project HEAD'],
    ['copy paths reveal the model', 'main/compare-copies.ts', 'JSON.stringify(path)}${change.added', 'JSON.stringify(file)}${change.added', 'includes committed, staged, unsaved, new and deleted work against the saved base, never the moving project HEAD'],
    ['a long diff is unlimited', 'main/compare-copies.ts', 'return boundedCopyDiff(text.trim())', 'return text.trim()', 'cuts a large real diff per column, rather than leaking the copy path or an unbounded file'],
    ['oversized copy files disappear', 'main/compare-copies.ts', 'hashTree(target, true)', 'hashTree(target)', 'names an oversized file rather than silently declaring a plain copy unchanged'],
    ['filed answers lose the work', 'main/index.ts', 'judgeAnswerText(answer, true)', 'answer.text', 'cuts a long diff with an explicit limit, including when the whole answer is filed'],
    ['a panel rebuilds an answers-only brief', 'main/index.ts', 'codexMissions.start(\n        brief,', 'codexMissions.start(\n        judgePrompt({ ...asked, answers: answers.map(({ letter, text }) => ({ letter, text })) }),', 'collects changes once before starting any judge and gives the panel the same material'],
    ['shared-folder claims appear verified', 'main/compare-judge.ts', "input.changes === 'folder'", 'false', 'judges a shared-folder comparison on answers only, without verifying its file claims'],
    ['an unchanged column loses its disclosure', 'main/compare-judge.ts', 'This column made no changes.', 'Work complete.', 'reads each column\'s diff under its own letter and says when one made no changes'],
    ['claims no longer have to match the diff', 'main/compare-judge.ts', 'Claiming a fix in code the diff does not touch is a defect.', 'Trust the final message about what was fixed.', 'checks the task, constraints, truthful claims, consistency, usability and completeness in that order, not polish'],
    ['a moving source is accepted as the original', 'main/compare-copies.ts', 'before !== undefined && !change.added && await hashOf(source).catch(() => undefined) !== before[path]', 'false', 'reads a plain copy against verified originals and refuses a source that moved, instead of judging the person\'s changes']
  ].map(([name, file, from, to, expected]) => ({ name: `judge-work: ${name}`, file: join(ROOT, 'src', file), from, to, expect: expected, tests: ['src/main/a-judge-reads-each-columns-work.test.ts'] })),
  ...[
    ['empty inherits saved words', 'main/compare-instructions.ts', 'return override === undefined ? current : override', 'return override || current', 'replace only the column’s own words, with empty distinct from omitted', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['the run drops its column override', 'main/codex-mission.ts', '...(slot?.instructions === undefined ? {} : { instructions: slot.instructions }),', '...{},', 'passes each column’s own instructions to its brief, including empty, without changing an ordinary turn', 'main/a-comparison-runs-one-teammate-on-two-models.test.ts'],
    ['the brief ignores the override', 'main/index.ts', 'columnInstructions(own?.instructions, conversation?.instructions)', 'columnInstructions(own?.instructions, undefined)', 'threads the override through each start, rather than reading mutable compare state in the brief builder', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['drafting ignores the teammate’s runtime', 'main/index.ts', 'teammate.route.runtime !== turn.metadata.runtime || ', '', 'drafts only from an owned finished conversation on the teammate’s own route and effort', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['a restart loses the override', 'main/compare-store.ts', '...(route.instructions === undefined ? {} : { instructions: instructionsText(route.instructions)! }),', '', 'keeps replacements through a restart, a follow-up and Try again', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['the cap is ignored', 'shared/teammate-instructions.ts', 'text.length <= MAX_TEAMMATE_INSTRUCTIONS', 'true', 'use the teammate’s own validation and cap, without silently turning a bad override into inheritance', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['a teammate is no longer required', 'main/compare-instructions.ts', '&& teammateId === undefined', '&& false', 'lets columns share a model for instruction trials, but requires a teammate and still refuses ordinary duplicate models', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['Current always takes A', 'main/compare-instructions.ts', 'const other = pick(index + 1)', 'const other = index', 'shuffles Current with the other candidates rather than always making it A', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['Run again loses the setups', 'main/compare-reruns.ts', 'routes: origin.slots.map((column) => column.route)', 'routes: origin.slots.map((column) => ({ runtime: column.route.runtime, model: column.route.model }))', 'keeps each replacement when the whole comparison runs again', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['the menu offers tune everywhere', 'renderer/src/slashCommands.ts', "return input.tune === true && !input.running", 'return true', 'offers /tune only with a finished teammate conversation', 'renderer/src/slashCommands.test.ts'],
    ['invalid JSON loses Current', 'shared/tune.ts', 'how: { instructions: current, why }, knows: { instructions: current, why }', "how: { instructions: '', why }, knows: { instructions: '', why }", 'bounds JSON drafts and leaves copies of Current on missing, malformed or untrusted shapes', 'renderer/src/tuning-never-applies-instructions-by-itself.test.tsx'],
    ['Current is offered as a change', 'shared/tune.ts', "instructions === undefined || instructions === (teammate.instructions ?? '')", 'instructions === undefined', 'offers the kept draft, never an unkept column, Current or another teammate’s words', 'renderer/src/tuning-never-applies-instructions-by-itself.test.tsx'],
    ['instruction trials skew the model leaderboard', 'shared/compare.ts', 'compare.kept === undefined || hasColumnInstructions(compare)', 'compare.kept === undefined', 'does not turn a model’s votes against its own setups into model leaderboard results', 'renderer/src/tuning-never-applies-instructions-by-itself.test.tsx'],
    ['instruction trials skew the model record table', 'renderer/src/compareRecord.ts', 'compare.kept === undefined || hasColumnInstructions(compare)', 'compare.kept === undefined', 'does not turn a model’s votes against its own setups into model leaderboard results', 'renderer/src/tuning-never-applies-instructions-by-itself.test.tsx'],
    ['opening the offer saves automatically', 'renderer/src/components/KeptInstructions.tsx', '  return <div className="lc-tuneoffer">', '  void onUse(teammate, instructions)\n  return <div className="lc-tuneoffer">', 'never applies instructions on render or Not now; only the explicit button calls the save path', 'renderer/src/tuning-never-applies-instructions-by-itself.test.tsx'],
    ['editing the ask overwrites a completed draft', 'renderer/src/components/TuneTeammate.tsx', 'onChange((current) => ({ ...current, prompt }))', 'onChange((_current) => ({ ...card, prompt }))', 'does not lose a completed draft when the person edits the ask at the same moment', 'renderer/src/tuning-never-applies-instructions-by-itself.test.tsx'],
    ['the draft no longer needs a specific moment', 'main/tune-drafts.ts', 'name a specific moment in this conversation', 'give a general suggestion about the role', 'asks for two distinct angles grounded in a moment, not task facts or instructions found in the conversation', 'main/a-comparison-keeps-each-columns-instructions.test.ts'],
    ['the trials start on opening the card', 'renderer/src/App.tsx', 'void window.desktop?.draftTune(of).then((answer) => {', 'void window.desktop?.startCompare({ teammateId: teammate.teammateId, prompt: liveRun!.prompt, routes: [route, route, route].map((one) => ({ runtime: one.runtime, model: one.model, instructions: current })) }).then(() => window.desktop!.draftTune(of)).then((answer) => {', 'starts one disclosed draft turn on /tune, while only the button starts the three trials', 'renderer/src/tuning-never-applies-instructions-by-itself.test.tsx']
  ].map(([name, file, from, to, expected, test]) => ({ name: `tune: ${name}`, file: join(ROOT, 'src', file), from, to, expect: expected, tests: [`src/${test}`] })),
  ...[
    ['the waiting words are omitted', 'renderer/src/components/Screens.tsx', 'const question = waiting === undefined ? undefined : waitingLine(waiting)', 'const question = undefined', 'uses the same waiting words as Home, for a question, a command and another approval'],
    ['the wrong conversation opens', 'renderer/src/components/Screens.tsx', 'onOpenMission(waiting.missionId)', 'onOpenMission(teammate.teammateId)', 'opens the waiting conversation, not whichever conversation was most recent'],
    ['the shell drops the waiting list', 'renderer/src/App.tsx', 'needsYou={needsYouItems}', 'needsYou={[]}', 'opens the waiting conversation, not whichever conversation was most recent'],
    ['the question takes more than two lines', 'renderer/src/shell.css', '-webkit-line-clamp: 2;\n  overflow: hidden;\n  overflow-wrap: anywhere;', '-webkit-line-clamp: 3;\n  overflow: hidden;\n  overflow-wrap: anywhere;', 'shows up to two lines of the question and leaves the full words available'],
    ['another teammate supplies the wait', 'renderer/src/components/Screens.tsx', "needsYou.find((item) => item.kind !== 'memory' && item.teammateId === teammate.teammateId)", "needsYou.find((item) => item.kind !== 'memory')", 'keeps Home’s priority, ignores memory and other teammates, and offers one Answer button'],
    ['Last run clips', 'renderer/src/shell.css', '  overflow-wrap: anywhere;\n  color: var(--lc-text-primary);\n}\n\n/* A wait is one button', '  overflow: hidden;\n  text-overflow: ellipsis;\n  white-space: nowrap;\n  color: var(--lc-text-primary);\n}\n\n/* A wait is one button', 'never cuts off Last run and gives it more space than Runs'],
    ['actions disappear at rest', 'renderer/src/shell.css', 'opacity: 0.45;\n  transition: opacity 120ms ease;', 'opacity: 0;\n  transition: opacity 120ms ease;', 'leaves the actions visible at rest, and full on hover or keyboard focus'],
    ['Team never marks the current screen', 'renderer/src/components/Sidebar.tsx', "aria-current={screenKey === 'teammates' ? 'page' : undefined}", 'aria-current={undefined}', 'is current only on the Team screen, with the existing selected surface']
  ].map(([name, file, from, to, expected]) => ({ name: `team-board: ${name}`, file: join(ROOT, 'src', file), from, to, expect: expected, tests: ['src/renderer/src/a-team-card-says-what-it-waits-on.test.tsx'] })),
  ...[
    ['Auto references are omitted', 'shared/references.ts', "if (mode === 'auto') return { include: true", "if (mode === 'auto') return { include: false", 'full-access modes keep the reference with a plain write warning'],
    ['a restricted extra folder becomes writable', 'shared/references.ts', "return { include: false, note: 'Not shared in this mode", "return { include: true, note: 'Not shared in this mode", 'a restricted runtime omits a grant that would make the reference writable and says why'],
    ['an unextended runtime loses a harmless brief reference', 'shared/references.ts', "return { include: true, note: 'Named in the brief", "return { include: false, note: 'Named in the brief", 'names references without changing an unextended runtime or inventing a write-access reason to omit them'],
    ['the size cap is ignored', 'main/conversation-references.ts', 'if (bytes > limit)', 'if (false)', 'refuses an oversized clone and recycles it, never the local folder'],
    ['a reference is kept per turn rather than per conversation', 'main/conversation-references.ts', 's.turns[mission] = root', 's.turns[mission] = mission', 'survives a restart and later turns, but not another conversation of the same teammate'],
    ['unused clones are not recycled', 'main/conversation-references.ts', 'if (item.repository !== undefined) {', 'if (false) {', 'only recycles a clone when its last conversation and live run release it'],
    ['a failed start discards the unsent reference', 'main/conversation-references.ts', 'await collect(ids.filter((id) => pendingCollection.has(id)))', 'await collect(ids)', 'keeps an unsent reference after a failed start releases its lease, until the person removes it'],
    ['the host accepts arbitrary clone addresses', 'main/conversation-references.ts', 'if (match === null) return undefined', 'if (match === null) return value', 'accepts only repository identities, not arbitrary git URLs, paths, options or shell fragments']
  ].map(([name, file, from, to, expected]) => ({ name: `references: ${name}`, file: join(ROOT, 'src', file), from, to, expect: expected, tests: ['src/main/a-reference-stays-with-its-conversation.test.ts'] })),
  {
    name: 'references: the every-turn reference brief is dropped', file: MISSIONS,
    from: "if (!bare && options.references !== undefined) runtimePrompt = [runtimePrompt, referencesSection(referenceFolders)].join('\\n\\n')",
    to: "if (false) runtimePrompt = [runtimePrompt, referencesSection(referenceFolders)].join('\\n\\n')",
    expect: 'briefs the same reference on later turns and never adds it to another conversation',
    tests: ['src/main/a-reference-is-briefed-on-every-turn.test.ts']
  },
  ...[
    ['foreign OpenCode replies are accepted', 'if (info.sessionID !== sessionId || !record(info.time)) continue', 'if (!record(info.time)) continue', 'reads OpenCode list and export output without tools, reasoning, foreign replies or unfinished exchanges'],
    ['OpenCode tool steps finish exchanges', "info.finish === 'stop'", "info.finish === 'tool-calls'", 'reads OpenCode list and export output without tools, reasoning, foreign replies or unfinished exchanges'],
    ['Cursor messages lose their order', 'for (const id of messages)', 'for (const id of [...messages].reverse())', 'reads Cursor chat metadata and ordered messages without changing the chat store or opening configuration'],
    ['Cursor tool calls finish exchanges', "p.type === 'tool-call'", "p.type === 'never-a-tool'", 'reads Cursor chat metadata and ordered messages without changing the chat store or opening configuration'],
    ['Cursor query wrappers are kept', "const text = query?.[1] ?? content", 'const text = content', 'reads Cursor chat metadata and ordered messages without changing the chat store or opening configuration'],
  ].map(([name, from, to, expect]) => ({ file: join(ROOT, 'src/main/agent-session-import.ts'), name: `session-import: ${name}`, from, to, expect, tests: ['src/main/past-opencode-and-cursor-chats-can-be-imported.test.ts'] })),
  { file: join(ROOT, 'src/main/session-import.ts'), name: 'session-import: imports choose a model', from: "const route = { model: 'account-default', requestedRouteId: runtime", to: "const route = { model: 'chosen-model', requestedRouteId: runtime", expect: 'imports cursor as a resumable conversation on account default and refuses duplicate or missing-folder imports', tests: ['src/main/past-opencode-and-cursor-chats-can-be-imported.test.ts'] },
  { file: join(ROOT, 'src/main/session-import.ts'), name: 'session-import: missing folders disappear', from: 'kept.push({ ...session, ...(exists ? {} : { folderMissing: true }) })', to: 'if (exists) kept.push(session)', expect: 'lists huge Cursor stores from metadata and keeps missing folders visible while excluding old and held sessions', tests: ['src/main/past-opencode-and-cursor-chats-can-be-imported.test.ts'] },
  { file: join(ROOT, "src", "main", "agent-accounts.ts"), name: "agent-accounts: default environment is replaced", from: "if (account === undefined) return launch", to: "if (account === undefined) return { ...launch, env: { ...launch.env, CODEX_HOME: '/wrong' } }", expect: "preserves the exact launch and environment when no account is selected", tests: ["src/main/an-added-account-has-only-its-own-home.test.ts"] },
  { file: join(ROOT, "src", "main", "agent-accounts.ts"), name: "agent-accounts: Codex home is dropped", from: "CODEX_HOME: home, CODEX_SQLITE_HOME: home", to: "CODEX_HOME: '/wrong', CODEX_SQLITE_HOME: home", expect: "gives each supported agent and account a separate home under Locust data", tests: ["src/main/an-added-account-has-only-its-own-home.test.ts"] },
  { file: join(ROOT, "src", "main", "agent-accounts.ts"), name: "agent-accounts: containment is bypassed", from: "const home = await ownedHome(a)\n        // Persist removal", to: "const home = homeFor(a)\n        // Persist removal", expect: "removes only the home Locust created and refuses a junction pointing elsewhere", tests: ["src/main/an-added-account-has-only-its-own-home.test.ts"] },
  { file: join(ROOT, "src", "shared", "route-at-start.ts"), name: "agent-accounts: saved account is dropped", from: "...(saved.account === undefined ? {} : { account: saved.account }),\n    model: saved.model", to: "model: saved.model", expect: "keeps the saved account for CLI/editor starts and an explicit default for a picker override", tests: ["src/main/an-added-account-has-only-its-own-home.test.ts"] },
  { file: join(ROOT, "src", "main", "mission-history.ts"), name: "agent-accounts: account usage is pooled", from: "accountUsageKey(event.sourceAdapter, accountFromRoute(mission.metadata.resolvedRouteId))", to: "event.sourceAdapter", expect: "keeps usage and exhausted limits separate for each account", tests: ["src/main/an-added-account-has-only-its-own-home.test.ts"] },
  { file: join(ROOT, "src", "main", "agent-account-login.ts"), name: "agent-accounts: login output is exposed", from: "stdio: 'ignore'", to: "stdio: 'pipe'", expect: "runs each documented login command hidden with its home and discards all process output", tests: ["src/main/an-account-sign-in-belongs-to-its-agent.test.ts"] },
  { file: join(ROOT, "src", "renderer", "src", "components", "RoutePicker.tsx"), name: "agent-accounts: policy leaves added routes visible", from: "policy?.policy.accounts === false ? []", to: "false ? []", expect: "hides added routes under organization policy while saying why Add is unavailable", tests: ["src/renderer/src/an-added-account-is-named-in-the-picker.test.tsx"] },

  {
    file: ORG_POLICY, name: 'org-policy: packaged apps accept the development override',
    from: 'if (!options.packaged && env.LOCUST_POLICY_FILE !== undefined)',
    to: 'if (env.LOCUST_POLICY_FILE !== undefined)',
    expect: 'uses the machine-wide path on each platform and ignores the test override in a packaged app', tests: [ORG_FILE_TEST]
  },
  {
    file: ORG_POLICY, name: 'org-policy: a malformed known switch is accepted',
    from: "for (const key of ['otherApps', 'phone', 'cloud']) if (value[key] !== undefined && typeof value[key] !== 'boolean') bad(key)",
    to: "for (const key of ['otherApps', 'phone', 'cloud']) if (false) bad(key)",
    expect: 'validates every optional setting and rejects a malformed known key', tests: [ORG_FILE_TEST]
  },
  {
    file: ORG_POLICY, name: 'org-policy: a broken file becomes unrestricted',
    from: 'return { path, present: true, policy: {}, error }', to: 'return { path, present: true, policy: {} }',
    expect: 'fails closed for broken JSON and names its file in every refusal', tests: [ORG_FILE_TEST]
  },
  {
    file: ORG_RULES, name: 'org-policy: every runtime is allowed',
    from: 'state.policy.allowedAgents.includes(runtime)', to: 'true',
    expect: 'allows only listed agents, including all own ACP agents under one runtime id', tests: [ORG_FILE_TEST]
  },
  {
    file: ORG_RULES, name: 'org-policy: every mode is allowed',
    from: 'return highest === undefined || POLICY_MODE_ORDER.indexOf(mode) <= POLICY_MODE_ORDER.indexOf(highest)', to: 'return true',
    expect: 'enforces every mode ceiling in the documented authority order', tests: [ORG_FILE_TEST]
  },
  {
    file: ORG_RULES, name: 'org-policy: a default escapes the model allowlist',
    from: "return id !== undefined && id !== 'account-default' && patterns.some((pattern) => policyModelMatches(pattern, id))",
    to: "return id === undefined || id === 'account-default' || patterns.some((pattern) => policyModelMatches(pattern, id))",
    expect: 'cannot use an unresolved default to escape a model allowlist', tests: [ORG_FILE_TEST]
  },
  {
    file: ORG_RULES, name: 'org-policy: a personal limit can raise the managed cap',
    from: 'Math.min(cap, personal)', to: 'Math.max(cap, personal)',
    expect: 'caps an absent or higher personal limit while keeping a lower one', tests: [ORG_FILE_TEST]
  },
  {
    file: ORG_SPEND, name: 'org-policy: a damaged unowned receipt allows spending',
    from: 'usageTurns(options.ledger, options.policy.policy.monthlyLimitUsd !== undefined)', to: 'usageTurns(options.ledger, false)',
    expect: 'fails closed if even one saved receipt has become unreadable', tests: [ORG_SPEND_TEST]
  },
  {
    file: ORG_MCP, name: 'org-policy: a saved MCP opt-in overrides the company',
    from: 'if (options.refusal?.() !== undefined) return state()', to: 'if (false) return state()',
    expect: 'ignores a saved MCP opt-in, removes the old connection, and refuses forged enabling', tests: [ORG_START_TEST]
  },
  {
    file: MISSIONS, name: 'org-policy: the central start skips the policy gate',
    from: "if (managed !== undefined) return error('POLICY_REFUSED', managed) as CodexMissionStartResponse",
    to: "if (false) return error('POLICY_REFUSED', managed) as CodexMissionStartResponse",
    expect: 'refuses agents, models, modes, cloud turns and broken policies before discovery or recording', tests: [ORG_START_TEST]
  },
  {
    file: MISSIONS, name: 'org-policy: a handoff skips the target policy before stopping',
    from: 'if (managed !== undefined) return managed', to: 'if (false) return managed',
    expect: 'keeps a running mission untouched when organization policy refuses the handoff target', tests: ['src/main/codex-mission.test.ts']
  },
  {
    file: ORG_PICKER, name: 'org-policy: the picker offers forbidden fallback models',
    from: '.filter((row) => policyModelAllowed(policy, row.runtime, row.model))', to: '',
    expect: 'filters the picker including account-default fallbacks', tests: [ORG_VIEW_TEST]
  },
  {
    file: ORG_COMPOSER, name: 'org-policy: the mode menu ignores the ceiling',
    from: 'MODES.filter((option) => policyModeAllowed(policy, option.mode)).map(', to: 'MODES.map(',
    expect: 'applies the ceiling to the mode menu, slash commands and the keyboard cycle', tests: [ORG_VIEW_TEST]
  },
  {
    file: NAMED_FILES,
    name: 'written-elsewhere: a declined step names its target again',
    from: '    if (command === undefined || (status !== undefined && DID_NOT_HAPPEN.test(status))) continue',
    to: '    if (command === undefined) continue',
    expect: 'names nothing for a step that was declined, failed, or never finished',
    tests: ['src/main/a-file-written-elsewhere-opens.test.ts']
  },
  {
    file: REVEAL,
    name: 'written-elsewhere: a network path is looked up like any other',
    from: '  if (/^[\\\\/]{2}/.test(requested)) return decision\n',
    to: '',
    expect: 'never looks up a network path, a relative one, or one inside the folder',
    tests: ['src/main/a-file-written-elsewhere-opens.test.ts']
  },
  {
    file: REVEAL,
    name: 'written-elsewhere: a file the teammate named outside the folders is refused again',
    from: '  return (await named(decision.path)) ? { ok: true, path: decision.path, named: true } : decision',
    to: '  return decision',
    expect: 'opens the one the teammate wrote and handed over, in any spelling of it',
    tests: ['src/main/a-file-written-elsewhere-opens.test.ts']
  },
  {
    file: HISTORY,
    name: 'written-elsewhere: the history forgets what a turn named',
    from: '      ...(named.length === 0 ? {} : { named }),\n',
    to: '',
    expect: 'finds a file handed over while the turn is still running',
    tests: ['src/main/a-file-written-elsewhere-opens.test.ts']
  },
  {
    file: HANDOVER,
    name: 'written-elsewhere: a network path is drawn as a file',
    from: "  if (path.startsWith('//')) return false\n",
    to: '',
    expect: 'drops a network or device path, which would reach another computer just by being looked at',
    tests: ['src/shared/handover.test.ts']
  },
  {
    file: HANDOVER,
    name: 'written-elsewhere: a whole path is joined onto the folder',
    from: '  return isWholePath(path) ? path : `${folder}/${path}`',
    to: '  return `${folder}/${path}`',
    expect: "is in the conversation's folder for a path in it, and the path itself for a whole one (0.787)",
    tests: ['src/shared/handover.test.ts']
  },
  {
    file: MCP_TOOLS,
    name: 'mcp-startup: a conversation claim is released before its turn starts',
    from: '          return await started({ prompt: args.message, teammateId: owner, followUpOf: mission.missionId }, id)',
    to: '          return started({ prompt: args.message, teammateId: owner, followUpOf: mission.missionId }, id)',
    expect: 'keeps a conversation claimed until its next turn has started',
    tests: ['src/main/other-apps-can-only-ask-locust-teammates.test.ts']
  },
  {
    file: MCP_TOOLS,
    name: 'mcp-startup: a rejected start leaves its conversation claimed',
    from: "      } finally { if (name === 'send_message') starting.delete(id) }",
    to: "      } finally { /* Deliberately leave the conversation claimed. */ }",
    expect: 'releases a conversation claim when starting its next turn rejects',
    tests: ['src/main/other-apps-can-only-ask-locust-teammates.test.ts']
  },
  {
    file: MEMORY_STORE,
    name: 'forget-nothing: a forget where nothing is remembered is a miss again',
    from: "        if (reachable.length === 0) return { removed: [], refusal: 'nothing-remembered', candidates: [] }",
    to: '        // (no answer of its own when nothing is remembered)',
    expect: 'is answered as nothing remembered, by the store',
    tests: ['src/main/a-forget-with-nothing-remembered-says-nothing.test.ts']
  },
  {
    file: MEMORY_READER,
    name: 'forget-nothing: a forget where nothing is remembered says it could not',
    from: "            } else if (result.refusal === 'nothing-remembered') {",
    to: '            } else if (false) {',
    expect: 'says nothing in the thread',
    tests: ['src/main/a-forget-with-nothing-remembered-says-nothing.test.ts']
  },
  {
    file: MISSIONS,
    name: 'relay-delivery: process launch consumes a waiting message',
    from: '        scheduleConsume(mission)',
    to: '        scheduleConsume(mission)\n        await peerExchange?.markDelivered(missionId, delivered)',
    expect: 'leaves an instantly refused relay message waiting and quotes it on the next run',
    tests: ['src/main/a-message-waits-until-its-runtime-starts.test.ts']
  },
  {
    file: MISSIONS,
    name: 'relay-delivery: a started process never marks its messages',
    from: '      await mission.deliverMessages(events)',
    to: '      // Deliberately skip the delivery mark.',
    expect: 'keeps delivery after a relay started and failed, without quoting it again',
    tests: ['src/main/a-message-waits-until-its-runtime-starts.test.ts']
  },
  {
    file: ANTIGRAVITY,
    name: 'relay-delivery: opening a hub conversation consumes a waiting message',
    from: '        }, pollMs)\n        void poll(run)',
    to: '        }, pollMs)\n        void poll(run)\n        await peerExchange?.markDelivered(missionId, delivered)',
    expect: 'leaves a silent hub run undelivered and quotes its message on the next run',
    tests: ['src/main/a-message-waits-until-its-runtime-starts.test.ts']
  },
  {
    file: ANTIGRAVITY,
    name: 'relay-delivery: a hub step never marks its messages',
    from: '        await run.deliverMessages(fresh)',
    to: '        // Deliberately skip the delivery mark.',
    expect: 'marks a hub message on its first step and keeps it delivered when stopped',
    tests: ['src/main/a-message-waits-until-its-runtime-starts.test.ts']
  },
  {
    file: VIEW,
    name: 'relay-delivery: a host briefing is retried through the person composer',
    from: "  if (kind === 'person') return { onRunAgain: retry }",
    to: '  if (true) return { onRunAgain: retry }',
    expect: 'withholds Run it again for a relay and never makes its briefing your bubble',
    tests: ['src/renderer/src/a-host-started-run-is-not-retried-as-your-message.test.tsx']
  },
  {
    file: VIEW,
    name: 'relay-delivery: a person loses the never-started retry',
    from: "  if (kind === 'person') return { onRunAgain: retry }",
    to: '  if (false) return { onRunAgain: retry }',
    expect: 'still offers Run it again for your own never-started run',
    tests: ['src/renderer/src/a-host-started-run-is-not-retried-as-your-message.test.tsx']
  },
  {
    file: VIEW,
    name: 'relay-delivery: handoffs are treated as person turns',
    from: "  if (turn.handoff !== undefined || turn.restoredMission?.continuesFrom?.reason === 'route-switch'",
    to: '  if (false',
    expect: 'withholds a composer retry for live and recovered handoffs',
    tests: ['src/renderer/src/a-host-started-run-is-not-retried-as-your-message.test.tsx']
  },
  {
    file: VIEW,
    name: 'a final rate-limit failure keeps its matching temporary warning too',
    from: "if (event.payload.kind === 'temporary-rate-limit' && endedOn !== undefined && sameSentence(event.payload.message, endedOn.payload.message)) break",
    to: "if (event.payload.kind === 'temporary-rate-limit' && endedOn === undefined && false) break",
    expect: 'does not repeat a final rate-limit failure as a temporary warning'
  },
  {
    file: VIEW,
    name: 'a matching error diagnostic no longer suppresses the duplicate failure card',
    from: "(item.type === 'diagnostic' && item.level === 'error')) && sameSentence(error, item.message)",
    to: "(item.type === 'diagnostic' && item.level === 'warning')) && sameSentence(error, item.message)",
    expect: 'keeps a failed turn readable when its diagnostic was never recorded'
  },
  {
    file: VIEW,
    name: 'the exchange link opens the run that WROTE the message, not the one it reached',
    from: "    mission.peerMessages.some((message) => message.messageId === messageId && message.direction === 'received')",
    to: '    mission.peerMessages.some((message) => message.messageId === messageId)',
    expect: 'finds the run a peer message was delivered into, so the exchange can be opened'
  },
  {
    file: RELAY,
    name: 'a one-to-one exchange that ends in silence says nothing at all',
    from: '      if (exchange !== undefined) {',
    to: '      if (false) {',
    expect: 'says so when a one-to-one reply never comes back, and where it went'
  },
  {
    file: VIEW,
    name: "a host-written briefing is drawn as the person's own words",
    from: '  if (turn.startedBy === undefined) return turn.prompt',
    to: '  if (true) return turn.prompt',
    expect: "never draws a turn the HOST briefed as the person's own words"
  },
  {
    file: MISSIONS,
    name: 'the plan mode is given a sandbox that can write',
    from: "      const sandbox: MissionSandbox = mode === 'accept-edits' ? 'workspace-write' : 'read-only'",
    to: "      const sandbox: MissionSandbox = mode === 'accept-edits' || mode === 'plan' ? 'workspace-write' : 'read-only'",
    expect: 'plans in the plan MODE, and only where the sandbox already refuses writes'
  },
  {
    file: STATUS,
    name: 'Plan is offered on a route that cannot be held read-only',
    from: "  if (mode === 'plan') return modeRunsOn('ask', runtime, platform)",
    to: "  if (mode === 'plan') return true",
    expect: 'withholds Plan wherever read-only is not real, for the same reason as Ask'
  },
  {
    file: STEERING,
    name: 'a queued message is sent after a run that did not complete',
    from: "  if (input.phase !== 'completed') {",
    to: '  if (false) {',
    expect: 'is HELD when the run did not complete, and says which way it ended'
  },
  {
    file: STEERING,
    name: 'a queued message is sent into whatever conversation is on screen now',
    from: '  if (!input.onScreen) return',
    to: '  if (false) return',
    expect: 'is held when the person has moved to another conversation'
  },
  {
    file: ROUTINE_RUNNER,
    name: 'a routine runs its next step even when the one before did not complete',
    from: "      if (phase !== 'completed') {",
    to: '      if (false) {',
    expect: 'a step that failed, was stopped or was interrupted ends the routine there and says which'
  },
  {
    file: ROUTINE_RUNNER,
    name: 'a routine step starts a stranger instead of continuing the same conversation',
    from: '      const response = await startStep(routine, peer, next, mission.missionId)',
    to: '      const response = await startStep(routine, peer, next, undefined)',
    expect: 'each later step follows up the one before it, and only after it COMPLETED'
  },
  {
    file: ROUTINE_RUNNER,
    name: 'two routines run at once for one teammate',
    from: '      if (already !== undefined) {',
    to: '      if (false) {',
    expect: 'one routine at a time per teammate'
  },
  {
    file: ROUTINE_STORE,
    name: 'an empty step is saved, so a run would start with nothing to do',
    from: '      && step.trim().length > 0\n',
    to: '',
    expect: "steps are the person's words: newlines allowed, empties and control bytes refused, count bounded"
  },
  {
    file: ROUTINE_STORE,
    name: 'editing a routine rewrites the teammate and route it was learned on',
    from: '        const next: PublicRoutine = { ...held, name: input.name.trim(), steps: [...input.steps] }',
    to: "        const next: PublicRoutine = { ...held, name: input.name.trim(), steps: [...input.steps], teammateId: 'tm_other' }",
    expect: 'creates, lists, gets, corrects, counts runs and removes'
  },
  {
    file: ROUTINES_VIEW,
    name: "a host-written turn is saved as one of the person's steps",
    from: '    if (held?.startedBy !== undefined) continue',
    to: '    if (false) continue',
    expect: 'drops turns the HOST wrote, whatever started them'
  },
  {
    file: CMDLEN,
    name: 'a command that cannot fit is launched anyway, to fail as cmd.exe sees fit',
    from: '  if (length + QUOTING_RESERVE <= WINDOWS_COMMAND_LINE_LIMIT) return undefined',
    to: '  if (true) return undefined',
    expect: 'refuses one that would hit the limit'
  },
  {
    file: CMDLEN,
    name: 'a stdin runtime is limited by a ceiling that does not apply to it',
    from: "  if (spec.stdin !== 'none') return undefined",
    to: '  if (false) return undefined',
    expect: 'never limits a runtime that reads the prompt from input'
  },
  {
    file: CMDREC,
    name: 'the prompt is recorded into the command, where a shared ledger carries it',
    from: "    if (trimmed.length > 0 && argument.trim() === trimmed) {",
    to: '    if (false) {',
    expect: 'never records the prompt, whatever flag introduced it'
  },
  {
    file: VIEW,
    name: 'the waiting clock restarts on every event, reading as a stuck loop',
    from: '      const since = options.startedAt ?? events[0]?.occurredAt ?? events.at(-1)?.occurredAt',
    to: '      const since = events.at(-1)?.occurredAt ?? options.startedAt',
    expect: 'counts from the start of the turn, so it climbs instead of looping'
  },
  {
    file: FACES,
    name: 'a live run with nothing open is called working, so a bobbing face wears the dots',
    from: "  return 'thinking'\n}",
    to: "  return 'working'\n}",
    expect: 'calls a live run with nothing open thinking, not working'
  },
  {
    file: FACES,
    name: 'a named turn step is called thinking, so its dotless line wears a thinking face',
    from: "  if (turnOpen) return 'working'",
    to: "  if (false) return 'working'",
    expect: 'calls a named turn step working, because its line shows no dots'
  },
  {
    file: STATUS,
    name: 'the footer counts a runtime this build cannot run',
    from: "  return runtimes.filter((runtime) => runtimeIsUsable(runtime) && integrationOf(runtime.id) !== 'planned')",
    to: '  return runtimes.filter((runtime) => runtimeIsUsable(runtime))',
    expect: 'does not count a runtime this build cannot actually run'
  },
  {
    file: RESUME,
    name: 'a resume is offered from a record the app cannot vouch for',
    from: "  if (point.resumeSafety === 'unsafe' || mission.integrityIssueCount > 0) {",
    to: "  if (point.resumeSafety === 'unsafe' && false) {",
    expect: 'refuses when recovery reported an issue, whatever the checkpoint concluded'
  },
  {
    file: RESUME,
    name: 'an unknown outcome is folded into an ordinary resume',
    from: "  if (point.resumeSafety === 'approval-required') {",
    to: '  if (false) {',
    expect: 'offers it WITH the doubt named when an action never reported back'
  },
  {
    file: RESUME,
    name: 'a resume starts from the first checkpoint rather than the newest',
    from: '    if (newest === undefined || checkpoint.epoch > newest.epoch) newest = checkpoint',
    to: '    if (newest === undefined) newest = checkpoint',
    expect: 'takes the newest, not the first'
  },
  {
    file: RESUME,
    name: 'a person who pressed Stop is offered a resume, arguing with their decision',
    from: "  const stoppedByShutdown = mission.phase === 'cancelled' && point?.reason === 'shutdown'",
    to: "  const stoppedByShutdown = mission.phase === 'cancelled'",
    expect: 'offers nothing to someone who pressed Stop'
  },
  {
    file: RESUME,
    name: 'a run the app closed underneath is not offered a resume at all',
    from: "  if (mission.phase !== 'interrupted' && !stoppedByShutdown) return undefined",
    to: "  if (mission.phase !== 'interrupted') return undefined",
    expect: 'offers a resume for a run the app closed underneath'
  },
  {
    file: DECISION,
    name: 'one option is offered as a decision, which is an approval in disguise',
    from: '  if (options.length < MIN_OPTIONS || options.length > MAX_OPTIONS) return undefined',
    to: '  if (options.length > MAX_OPTIONS) return undefined',
    expect: 'refuses a single option, which is an announcement rather than a decision'
  },
  {
    file: DECISION,
    name: 'the same option twice is offered as two choices',
    from: '  if (seen.size !== options.length) return undefined',
    to: '  if (false) return undefined',
    expect: 'refuses two options that say the same thing'
  },
  {
    file: DECISION,
    name: 'prose after the options is folded into the question',
    from: '      if (options.length === 0) questionLines.push(trimmed)',
    to: '      questionLines.push(trimmed)',
    expect: 'ignores prose that arrives after the options'
  },
  {
    file: DECISION,
    name: 'a raw ask tag is left in the reply bubble',
    from: "  return text.replace(BLOCK, '').replace(/\\n{3,}/gu, '\\n\\n').trimEnd()",
    to: '  return text.trimEnd()',
    expect: 'is taken out, because the card asks it answerably'
  },
  {
    file: VIEW,
    name: 'an already-answered question is offered again on an earlier turn',
    from: '  if (!options.running && options.latestTurn === true) {',
    to: '  if (!options.running) {',
    expect: 'is not offered on an earlier turn, where the answer already exists'
  },
  {
    file: VIEW,
    name: 'the card claims nothing changed for a run that could write',
    from: "  if (input.sandbox === 'read-only') return 'stopped here · nothing was changed'",
    to: "  if (true) return 'stopped here · nothing was changed'",
    expect: 'never claims nothing changed for a run that was allowed to write'
  },
  {
    file: VIEW,
    name: 'a relayed run is named by the briefing the host wrote to a runtime',
    from: "  if (mission.startedBy?.kind !== 'relay') return undefined",
    to: '  if (false) return undefined',
    expect: 'leaves a mission a person started alone'
  },
  {
    file: VIEW,
    name: 'a title is invented for a message the workroom no longer holds',
    from: '  if (asked === undefined || asked.text === null) return undefined',
    to: '  if (asked === undefined) return undefined',
    expect: 'shows the briefing rather than inventing a title when the message is gone'
  },
  {
    file: VIEW,
    name: 'a relayed run is named by what it SENT rather than what it was asked',
    from: "  const asked = mission.peerMessages.find((message) => message.direction === 'received')",
    to: '  const asked = mission.peerMessages[0]',
    expect: 'ignores what the run SENT and names it by what it was asked'
  },
  {
    file: VIEW,
    name: 'typedPrompt hands back the host briefing for a relayed run',
    from: '    const relayed = relayedTitle(current)\n    if (relayed !== undefined) return relayed\n',
    to: '',
    expect: 'does not let typedPrompt hand back the briefing either'
  },
  {
    file: VIEW,
    name: 'a waiting line says Working above the approval it is stopped on',
    from: "    } else if (!streaming && options.awaitingDecision !== true) {",
    to: '    } else if (!streaming) {',
    expect: 'says nothing above an approval the run is stopped on'
  },
  {
    file: VIEW,
    name: 'a freshly sent mission shows nothing until its first event',
    from: "    } else if (!streaming && options.awaitingDecision !== true) {",
    to: '    } else if (false) {',
    expect: 'shows a line the moment a run starts, before any event arrives'
  },
  {
    file: VIEW,
    name: 'a live line lingers after the run is over',
    from: '  if (options.running) {',
    to: '  if (true) {',
    expect: 'draws no live line once the run is over'
  },
  {
    file: VIEW,
    name: 'only the newest turn of a teammate exchange is drawn',
    from: '  turns.forEach((messages, turnIndex) => {',
    to: '  turns.slice(-1).forEach((messages, turnIndex) => {',
    expect: 'keeps every turn of a long conversation, not just the last two'
  },
  {
    file: VIEW,
    name: 'the message a teammate SENT is dropped from the thread',
    from: "        placement: group.received ? 'before-work' : 'after-work',",
    to: "        placement: 'before-work',",
    expect: 'puts what a turn sent after its work and what it was handed before it'
  },
  {
    file: VIEW,
    name: 'a two-message exchange stays collapsed and goes unread',
    from: '  return messageCount > 0 && messageCount <= PEER_EXCHANGE_OPEN_LIMIT',
    to: '  return false',
    expect: 'opens a short exchange in place'
  },
  {
    file: COST,
    name: 'the running turn is left out of what the conversation has cost',
    from: '  return sumCosts([...earlierTurns.map((turn) => runCostOf(turn.events)), runCostOf(events)])',
    to: '  return sumCosts(earlierTurns.map((turn) => runCostOf(turn.events)))',
    expect: 'adds every turn that reported a number, including the one still running'
  },
  {
    file: STATUS,
    name: 'the Recent group resurrects a route the runtime no longer offers',
    from: '    if (row === undefined) continue',
    to: "    if (row === undefined) { picked.push({ key, group: 'Recent' }); continue }",
    expect: 'never resurrects a route the runtime has stopped offering'
  },
  {
    file: STATUS,
    name: 'a single recent route is offered as a shortcut to where you already are',
    from: '  if (picked.length < 2) return []',
    to: '  if (false) return []',
    expect: 'withholds the group for a single recent route, which is the one you are on'
  },
  {
    file: TEAMMATE_WORK,
    name: 'a card counts every mission, not only the ones the host says are theirs',
    from: '    .filter((mission) => missionOwners[mission.missionId] === teammateId)',
    to: '    .filter(() => true)',
    expect: 'counts only the missions the host recorded as theirs'
  },
  {
    file: TEAMMATE_WORK,
    name: 'the last run is taken from the list order rather than the newest mission',
    from: '    .sort((left, right) => Date.parse(right.lastUpdatedAt) - Date.parse(left.lastUpdatedAt))',
    to: '',
    expect: 'takes the last run from the newest mission, not the first in the list'
  },
  {
    file: STATUS,
    name: 'a roster card prints read-only for every teammate again',
    from: "  if (mode === 'accept-edits') return 'accept edits'",
    to: "  if (mode === 'accept-edits') return 'ask · read-only'",
    expect: 'names the mode the teammate actually last ran in'
  },
  {
    file: VIEW,
    name: 'a run in trouble stays hidden until a tool has run, so a retrying mission looks frozen',
    from: '        if (!workBegan && !/\\.runtime_error$/.test(event.payload.code)) break',
    to: '        if (!workBegan) break',
    expect: 'shows a run in trouble even before any tool has run'
  },
  {
    file: VIEW,
    name: 'a mission from another day is labelled with a bare time',
    from: '  if (sameDay) return time',
    to: '  if (true) return time',
    expect: 'carries the date once the mission is not from today'
  },
  {
    file: VIEW,
    name: 'a target that only arrives with the completion never reaches the row',
    from: "              ...(event.payload.command === undefined || open.name !== open.tool",
    to: "              ...(true || event.payload.command === undefined || open.name !== open.tool",
    expect: 'takes the target from the completion when the start had none'
  },
  {
    file: AGENT_TEXT,
    name: 'an unterminated fence is dropped instead of drawn as code still arriving',
    from: '  if (open !== undefined) {',
    to: '  if (false) {',
    expect: 'treats an unclosed fence as code still being written, not as prose'
  },
  {
    file: AGENT_TEXT,
    name: 'a closing fence is accepted even when it carries a language, swallowing the block after it',
    from: "    if (fence !== null && fence[2]!.trim().length === 0 && fence[1]!.length >= open.ticks.length) {",
    to: '    if (fence !== null) {',
    expect: 'keeps a longer fence inside a shorter one as code'
  },
  {
    file: STATUS,
    name: 'a refused send is filed in the mission list as though it were a mission',
    from: '  return run.missionId !== undefined || run.active',
    to: '  return true',
    expect: 'drops a refused send, which settled without ever being given a mission id'
  },
  {
    file: VIEW,
    name: 'a failure card drops the runtime own-stderr line and shows only the host sentence',
    from: '  if (said === undefined) return payload.message',
    to: '  if (said !== undefined) return payload.message',
    expect: "shows the runtime's own last word, because the host's sentence names only the shape"
  },
  {
    file: MISSIONS,
    name: 'a follow-up is only linked when the earlier turn left a session to resume',
    from: '              ? resumedMissionId === undefined',
    to: '              ? resumedMissionId === undefined || resumeThreadId === undefined',
    expect: 'continues a conversation whose earlier turn recorded no session, cold rather than not at all'
  },
  {
    file: UPDATES,
    name: 'install goes back to quitAndInstall, which the shutdown handler cancels',
    from: '      options.requestQuit()\n      return { ok: true, data: state }',
    to: '      options.updater.quitAndInstall(false, true)\n      return { ok: true, data: state }',
    expect: 'installs by asking the app to quit, so the shutdown flush runs first and the updater installs on the real quit'
  },
  {
    file: VIEW,
    name: 'a multi-file edit is counted as one file',
    from: "    .reduce((sum, detail) => sum + Math.max(1, detail.name.split('\\n').filter((line) => line.length > 0).length), 0)",
    to: '    .length',
    expect: 'counts files, not edit calls: one Codex file_change can name several'
  },
  {
    file: COST,
    name: 'a receipt that reported no cost is shown as free',
    from: "  return Object.keys(cost).length === 0 ? undefined : cost",
    to: "  return cost",
    expect: 'says nothing when the receipt carries nothing, rather than zero'
  },
  {
    file: COST,
    name: 'a dollar figure the runtime priced is dropped for a token count',
    from: "  if (cost.usd !== undefined) return cost.usd === 0 ? '$0.00' : cost.usd < 0.01 ? '< $0.01' : `$${cost.usd.toFixed(2)}`",
    to: "",
    expect: 'prefers dollars, then premium requests, then tokens'
  },
  {
    file: ANTIGRAVITY,
    name: "the agent's final answer never ends the watch",
    from: '      if (run.normalizer.latestFinal) {',
    to: '      if (false) {',
    expect: 'opens the conversation with the project id, records the mission, and follows the transcript to the final answer'
  },
  {
    file: ANTIGRAVITY,
    name: "a follow-up replays the earlier turns as its own work",
    from: "          fed = existing === undefined ? 0 : existing.split('\\n').filter((line) => line.trim().length > 0).length",
    to: '          fed = 0',
    expect: 'a follow-up sends into the same conversation and reads only the lines after the earlier turns'
  },
  {
    file: ANTIGRAVITY,
    name: 'a folder Antigravity has not opened is started anyway',
    from: '        if (projectId === undefined) {',
    to: '        if (false) {',
    expect: 'refuses, naming the folder, when Antigravity has not opened the workspace'
  },
  {
    file: ANTIGRAVITY,
    name: 'a mission starts with Antigravity closed',
    from: '        if (host === undefined) {\n          throw new AntigravityStartError',
    to: '        if (false) {\n          throw new AntigravityStartError',
    expect: 'refuses, recording nothing, when Antigravity is not open'
  },
  {
    file: RELAY,
    name: "the asker's turn starts on the first reply instead of waiting for everyone",
    from: '          if (meeting.awaiting.size > 0) {\n            notify(',
    to: '          if (false) {\n            notify(',
    expect: "holds the first reply, and starts the asker's turn once after the last, briefed with everyone"
  },
  {
    file: RELAY,
    name: 'a single recipient opens a meeting',
    from: '      if (started.length >= 2) {',
    to: '      if (started.length >= 1) {',
    expect: 'one recipient is an ordinary exchange, not a meeting'
  },
  {
    file: FACES,
    name: 'an idle face moves',
    from: "  idle: {}\n}",
    to: "  idle: { chip: 'lcBob 2.8s ease-in-out infinite' }\n}",
    expect: 'idle and blocked are the only motionless states, and every other state moves'
  },
  {
    file: FACES,
    name: 'a reasoning step is drawn as working',
    from: "  if (reasoning) return 'thinking'",
    to: "  if (reasoning) return 'working'",
    expect: 'thinks during an open reasoning step'
  },
  {
    file: FACES,
    name: 'waiting on you falls through to the live state or to still',
    from: "  if (input.waitingOnYou) return 'waiting'\n",
    to: '',
    expect: 'waiting on you never resolves to still, and outranks live work'
  },
  {
    file: RELAY,
    name: "a teammate replies on the sender's route instead of their own",
    from: '    const own = recipient.self.route',
    to: '    const own = undefined',
    expect: "starts the recipient's run on the recipient's OWN route, owned by the recipient"
  },
  {
    file: RELAY,
    name: 'teammates reply on their own even when the setting is off',
    from: "  if (!input.enabled) {\n    return { start: false, reason: 'Teammate replies are switched off in Settings; the message waits for their next run.' }\n  }\n",
    to: '',
    expect: 'starts nothing when off, and stays quiet about it'
  },
  {
    file: RELAY,
    name: 'the hop cap lets one extra hop through',
    from: '  if (input.hop >= MAX_RELAY_HOPS) {',
    to: '  if (input.hop > MAX_RELAY_HOPS) {',
    expect: 'stops after the cap and says so in the thread that shared'
  },
  {
    file: RELAY,
    name: 'a relayed run is left free to ask a person who is not there',
    from: '    noPersonHere(input.sender.name),\n',
    to: '',
    expect: 'tells a relayed run to ask the teammate, not a person'
  },
  {
    file: RELAY,
    name: 'the reply back starts a stranger instead of the next turn of the thread that asked',
    from: '    const followUpOf = input.origin.lastMissionOf[recipient.self.teammateId]',
    to: '    const followUpOf = undefined',
    expect: 'the reply back follows up the mission that asked, so it lands in that thread'
  },
  {
    file: RELAY,
    name: 'a relayed run runs with write access whatever the sender had',
    from: "      mode: from.sandbox === 'workspace-write' ? ('accept-edits' as const) : ('ask' as const)",
    to: "      mode: 'accept-edits' as const",
    expect: 'a read-only sender gets a read-only reply when the recipient has no route'
  },
  {
    file: VIEW,
    name: 'the activity total is taken from the runtime header instead of the rows',
    from: '    if (entry.kind !== \'file\') continue\n    added += entry.counts.added\n    removed += entry.counts.removed',
    to: '    if (entry.kind !== \'file\') continue\n    added += entry.reported?.added ?? entry.counts.added\n    removed += entry.reported?.removed ?? entry.counts.removed',
    expect: 'sums the card total from the rows it will actually draw'
  },
  {
    file: VIEW,
    name: 'an edit with no recorded patch is dropped from the list',
    from: '      entries.push({\n        kind: detail.kind === \'edit\' ? \'unreported\' : \'tool\',',
    to: '      if (detail.kind === \'edit\') return\n      entries.push({\n        kind: \'tool\',',
    expect: 'keeps an edit whose runtime reported no patch, as a row that says so'
  },
  {
    file: VIEW,
    name: 'a large file still opens by default and buries the rest',
    from: '  return first.large ? undefined : first.key',
    to: '  return first.key',
    expect: 'opens the first file, unless opening it would bury everything after it'
  },
  {
    file: VIEW,
    name: 'the completion patch never reaches the activity row',
    from: '              ...(patch === undefined ? {} : { patch, kind: \'edit\' }),\n',
    to: '',
    expect: 'attaches a completion patch to the tool that opened, and names the runtime that reported it'
  },
  {
    file: STATUS,
    name: 'a read-only Cursor mode is offered where nothing can enforce it',
    from: "  if (mode === 'ask' && runtime === 'cursor' && platform === 'win32') return false",
    to: '  void platform',
    expect: 'does not offer Cursor a read-only mode on Windows, where nothing enforces it'
  },
  {
    file: STATUS,
    name: 'a read-only Cursor mode is refused even where the sandbox is real',
    from: "  if (mode === 'ask' && runtime === 'cursor' && platform === 'win32') return false",
    to: "  if (mode === 'ask' && runtime === 'cursor') return false",
    expect: 'offers it where the sandbox exists'
  },
  {
    file: STATUS,
    name: 'the search field filters nothing, as it did when it was inert',
    from: '  if (needle.length === 0) return rows',
    to: '  return rows; if (needle.length === 0) return rows',
    expect: 'matches the words a person can see, whatever the case'
  },
  {
    file: STATUS,
    name: 'a search that matches nothing shows everything',
    from: '  return rows.filter((row) =>',
    to: '  return rows.length > 0 ? rows : rows.filter((row) =>',
    expect: 'shows nothing when nothing matches, rather than everything'
  },
  {
    file: STATUS,
    name: 'the curated shortlist outranks what this person actually ran',
    from: '    if (used !== undefined) return used',
    to: '    if (used !== undefined) return 3_000_000 + used',
    expect: 'still puts what this person ran above the curated list'
  },
  {
    file: STATUS,
    name: 'the curated shortlist does nothing at all',
    from: '    return flagship === undefined ? 2_000_000 : 1_000_000 + flagship',
    to: '    void flagship; return 2_000_000',
    expect: 'puts the flagship families above the rest when nothing has been run'
  },
  {
    file: VIEW,
    name: 'the picker forgets the order routes were last run in',
    from: '    .sort((left, right) => right[1] - left[1])',
    to: '    .sort((left, right) => left[1] - right[1])',
    expect: 'lists each route once, newest first'
  },
  {
    file: STATUS,
    name: 'a mode the route cannot run is still offered',
    from: "  if (mode === 'approve-each') return runtime === 'codex'",
    to: "  if (mode === 'approve-each') return true",
    expect: 'keeps per-action approvals to the runtime that can stop and ask'
  },
  {
    file: STATUS,
    name: 'the picker forgets what this person has actually run',
    from: '    const used = rank.get(row.key)',
    to: '    const used = undefined',
    expect: 'puts what this person has run first, newest first, inside its own runtime'
  },
  {
    file: STATUS,
    name: 'ordering moves rows out of their own runtime',
    from: '    const byGroup = groups.indexOf(left.group) - groups.indexOf(right.group)',
    to: '    const byGroup = 0',
    expect: 'never moves a row out of its runtime'
  },
  {
    file: CATALOG_MODELS,
    name: 'every effort Cursor lists is offered as its own model again',
    from: '    if (effort === undefined) {',
    to: '    if (true) {',
    expect: 'collapses the efforts Cursor lists as separate models into one model with efforts'
  },
  {
    file: CATALOG_MODELS,
    name: 'an id is split on the shortest effort suffix, losing the rest',
    from: "  'xhigh-fast',\n  'high-fast',\n  'medium-fast',\n  'low-fast',\n",
    to: '',
    expect: 'splits an id into the model and the effort, longest suffix first'
  },
  {
    file: UPDATES,
    name: 'an update installs itself while a mission is running',
    from: '      if (options.liveMissionCount() > 0) {',
    to: '      if (false) {',
    expect: 'refuses to install while a mission is running, and says why'
  },
  {
    file: UPDATES,
    name: 'auto-install at quit is switched back off, so the install never runs',
    from: '    options.updater.autoInstallOnAppQuit = true',
    to: '    options.updater.autoInstallOnAppQuit = false',
    expect: 'downloads on its own, and installs only on a quit the app itself makes'
  },
  {
    file: UPDATES,
    name: 'a check that could not run reports the app as up to date',
    from: "            phase: 'failed',\n            currentVersion: options.currentVersion,\n            message: 'The update check could not complete.'",
    to: "            phase: 'current',\n            currentVersion: options.currentVersion",
    expect: 'says up to date only after a check that finished'
  },
  {
    file: UPDATES,
    name: 'a build that cannot update itself reports itself up to date',
    from: '      if (!options.supported) {',
    to: '      if (false) {',
    expect: 'says a build that cannot update itself cannot, rather than that it is current'
  },
  {
    file: MISSIONS,
    name: 'a command that cannot be built still writes a mission file first',
    from: '        } catch {\n          return error(\n            \'RUNTIME_START_FAILED\',',
    to: '        } catch {\n          command = createCodexExecCommand(chosen.executable, { workspacePath: options.workspacePath })\n        }\n        if (false) {\n          return error(\n            \'RUNTIME_START_FAILED\',',
    expect: 'records nothing when the chosen options cannot be turned into a command'
  },
  {
    file: VIEW,
    name: 'a reply is shown the opening line instead of the words typed for it',
    from: "    if (current.continuesFrom?.reason !== 'route-switch') return current.prompt",
    to: '    if (current.continuesFrom === undefined) return current.prompt',
    expect: 'shows a reply the words that were typed for it, not the opening line'
  },
  {
    file: VIEW,
    name: 'a handed-over mission shows its briefing as typed words',
    from: '    const prior = byId.get(current.continuesFrom.missionId)',
    to: '    const prior = undefined',
    expect: 'shows a handed-over mission the words a person typed, not the briefing written for it'
  },
  {
    file: STATUS,
    name: 'the active tag is drawn on a runtime that is not there',
    from: "  return isActive && status.selectable ? 'ACTIVE' : status.tag",
    to: "  return isActive ? 'ACTIVE' : status.tag",
    expect: 'does not put ACTIVE on a runtime that is not there'
  },
  {
    file: STATUS,
    name: 'a teammate with no work of their own reads as blocked',
    from: '  if (input.runtime !== undefined && !runtimeIsUsable(input.runtime)) {',
    to: '  if (input.runtime === undefined || !runtimeIsUsable(input.runtime)) {',
    expect: 'is idle, not blocked, when something could run'
  },
  {
    file: STATUS,
    name: 'a roster with nothing signed in still says everyone is idle',
    from: '  if (input.anyRuntimeUsable === false) {',
    to: '  if (false) {',
    expect: 'is blocked when nothing is signed in at all'
  },
  {
    file: ROSTER,
    name: 'mission assignments grow without bound, until the file empties itself',
    from: '        if (\n          file.missionOwners[missionId] === undefined\n          && Object.keys(file.missionOwners).length >= MAX_MISSION_OWNERS\n        ) {\n          throw new Error(\'Too many mission assignments\')\n        }\n',
    to: '',
    expect: 'refuses a new assignment past its cap, instead of growing until the file empties itself'
  },
  {
    file: ROSTER,
    name: 'the roster reads more assignments than it would ever write',
    from: '      if (Object.keys(owners).length >= MAX_MISSION_OWNERS) break',
    to: '      void owners',
    expect: 'reads no more assignments than it would write'
  },
  {
    file: MISSIONS,
    name: 'a read-only Cursor mission runs where nothing can hold it read-only',
    from: '          && !cursorCanEnforceReadOnly(hostPlatform)',
    to: '          && false',
    expect: 'refuses a read-only Cursor mission where its sandbox cannot run, rather than mislabelling it'
  },
  {
    file: VIEW,
    name: 'a run that never opened a session still claims one a reply can resume',
    from: "    if (typeof held === 'string' && held.length > 0) return held",
    to: "    return typeof held === 'string' ? held : 'thread-7'",
    expect: 'has nothing to resume when the run failed before its runtime started'
  },
  {
    file: STATUS,
    name: 'a prune preview counts the deletions and stays quiet about what it kept',
    from: "  return `Delete ${missions(preview.deleted.length)} for good${kept.length === 0 ? '' : `, with ${kept.join(' and ')}`}.`",
    to: '  return `Delete ${missions(preview.deleted.length)} for good.`',
    expect: 'states the count that would go, and names everything held back'
  },
  {
    file: STATUS,
    name: 'a prune that would do nothing does not say why',
    from: "    return because.length === 0\n      ? 'Nothing is old enough to delete.'\n      : `Nothing would be deleted: ${because.join(', and ')}.`",
    to: "    return 'Nothing is old enough to delete.'",
    expect: 'says why nothing would go, rather than just saying nothing'
  },
  {
    file: STATUS,
    name: 'a long picker group is truncated without saying so',
    from: '    hidden.set(row.group, (hidden.get(row.group) ?? 0) + 1)\n  }\n  return { rows: kept, hiddenByGroup: hidden }',
    to: '  }\n  return { rows: kept, hiddenByGroup: hidden }',
    expect: 'caps a long group and counts every row it is not showing'
  },
  {
    file: STATUS,
    name: 'the cap hides the route the person is actually on',
    from: "    if (row.tag === 'ACTIVE') {",
    to: '    if (false) {',
    expect: 'never hides the route you are on, and still shows the cap'
  },
  {
    file: STATUS,
    name: 'a search result is capped a second time, under the person',
    from: '  if (searching) return { rows, hiddenByGroup: new Map() }',
    to: '  void searching',
    expect: 'lifts the cap entirely once someone is searching'
  },
  {
    file: MISSIONS,
    name: 'a Cursor mission is launched as a Codex command',
    from: "            : runtime === 'cursor'\n              ? createCursorPrintCommand(executable, {\n                  workspacePath: options.workspacePath,\n                  sandbox: effectiveSandbox,\n                  ...choice\n                })\n",
    to: '',
    expect: 'runs a Cursor mission under its own command and its own normalizer'
  },
  {
    file: MISSIONS,
    name: "a Cursor mission's stream is read by the Codex normalizer",
    from: "          : runtime === 'cursor'\n            ? createCursorEventNormalizer(normalizerContext)\n",
    to: '',
    expect: 'runs a Cursor mission under its own command and its own normalizer'
  },
  {
    file: RUNTIMES,
    name: 'a runtime whose events the host cannot read is started anyway',
    from: "  return runtime === 'codex' || runtime === 'claude' || runtime === 'cursor' || runtime === 'opencode' || runtime === 'copilot'\n",
    to: '  return true\n',
    expect: 'refuses a runtime whose event stream it cannot read yet, by name, recording nothing'
  },
  {
    file: VIEW,
    name: 'a turn opening counts as work, so setup notices land in the thread',
    from: "        if (event.payload.stepKind !== 'turn') workBegan = true",
    to: '        workBegan = true',
    expect: 'keeps a notice raised before any work out of the thread, as setup talk'
  },
  {
    file: HISTORY,
    name: 'a live mission can be deleted out from under its own process',
    from: '  if (isLive(missionId)) {',
    to: '  if (false) {',
    expect: 'refuses to delete a mission that is still running, and names the remedy'
  },
  {
    file: MISSIONS,
    name: 'a reply starts a blank run instead of resuming the conversation',
    from: '            ...(resumeThreadId === undefined ? {} : { resumeThreadId })',
    to: '',
    expect: 'resumes the earlier mission’s own session, and records what it continued'
  },
  {
    file: MISSIONS,
    name: 'a reply crosses runtimes without a handoff',
    from: '          if (prior.metadata.runtime !== runtime) {',
    to: '          if (false) {',
    expect: 'refuses to continue another runtime’s conversation, and says which'
  },
  {
    file: VIEW,
    name: 'a handoff divider is drawn across an ordinary reply',
    from: "  if (link === undefined || link.reason !== 'route-switch') return undefined",
    to: '  if (link === undefined) return undefined',
    expect: 'draws no handoff divider across an ordinary reply'
  },
  {
    file: VIEW,
    name: 'a reply shows only itself, losing the conversation above it',
    from: "    if (link === undefined || link.reason !== 'follow-up') break",
    to: '    break',
    expect: 'walks a reply back to every earlier turn, oldest first'
  },
  {
    file: VIEW,
    name: 'an older mission overwrites the newest resolved model name',
    from: '    if (held === undefined || (Number.isFinite(at) && at > held.at)) {',
    to: '    if (true) {',
    expect: 'prefers the newest mission, so a new release replaces an old name'
  },
  {
    file: VIEW,
    name: 'the alias echoed back is shown as if it were a resolved name',
    from: "    if (typeof name !== 'string' || name.length === 0 || name === mission.model) continue",
    to: "    if (typeof name !== 'string' || name.length === 0) continue",
    expect: 'says nothing about an alias nobody has run, or one that taught it nothing'
  },
  {
    file: CATALOG,
    name: 'a signed-out Claude still offers its models',
    from: "  if (claude?.readiness !== 'ready' || hints === undefined) return []",
    to: '  if (hints === undefined) return []',
    expect: 'offers nothing for a Claude that is not ready, or that advertised nothing'
  },
  {
    file: CATALOG,
    name: 'a Codex failure hides the Claude models too',
    from: "      return advertisedModels.length > 0\n        ? { ok: true, data: { models: advertisedModels } }\n        : { ok: false, error: { code: 'MODELS_UNAVAILABLE', message: 'Codex CLI is not ready.' } }",
    to: "      return { ok: false, error: { code: 'MODELS_UNAVAILABLE', message: 'Codex CLI is not ready.' } }",
    expect: 'lists Claude models even when Codex cannot be read'
  },
  {
    file: APPROVALS,
    name: 'stopping one approve-each run kills every run',
    from: "    run.client?.dispose(why)\n    run.process?.kill()\n  }",
    to: "    run.client?.dispose(why)\n    for (const each of [...runs.values(), run]) each.process?.kill()\n  }",
    expect: 'stops one approve-each run by its id and leaves the other going'
  },
  {
    file: APPROVALS,
    name: 'stopping one approve-each run refuses every pending approval',
    from: '      if (pending.runId !== runId) continue\n',
    to: '',
    expect: 'answers only the stopped run’s approvals with a refusal'
  },
  {
    file: VIEW,
    name: 'the runtime’s setup talk lands in the thread',
    from: '        if (!workBegan && !/\\.runtime_error$/.test(event.payload.code)) break\n        items.push({',
    to: '        items.push({',
    expect: 'keeps a notice raised before any work out of the thread, as setup talk'
  },
  {
    file: TEAMMATES,
    name: 'an edit hands the teammate a new id, orphaning their missions',
    from: '          teammateId: existing.teammateId,\n          name: input.name.trim(),',
    to: "          teammateId: `tm_${randomUUID().replace(/-/g, '').slice(0, 24)}`,\n          name: input.name.trim(),",
    expect: 'changes name, hue, role and face while the id and its missions stay'
  },
  {
    file: VIEW,
    name: 'every running step is filed as action, so thought draws as work',
    from: "          kind: event.payload.stepKind\n        }",
    to: "          kind: 'turn'\n        }",
    expect: 'carries the step kind, so thought and action draw differently'
  },
  {
    file: STATUS,
    name: 'a teammate with no live run is drawn as working',
    from: "    live: input.hasRunningMission ? (input.liveActivity ?? 'working') : 'idle',",
    to: "    live: 'working',",
    expect: 'decides the face from the same inputs as the label, so the two agree'
  },
  {
    file: TEAMMATES,
    name: 'a face is seeded from the name, so a rename would change it',
    from: '          avatar: input.avatar ?? seedAvatar(teammateId),',
    to: '          avatar: input.avatar ?? seedAvatar(input.name.trim()),',
    expect: 'seeds a face from the id, never the name'
  },
  {
    file: TEAMMATES,
    name: 'a persisted face is thrown away on read',
    from: '    avatar: isAvatarSpec(record.avatar) ? record.avatar : seedAvatar(record.teammateId),',
    to: '    avatar: seedAvatar(record.teammateId),',
    expect: 'keeps a persisted face rather than re-seeding it, and seeds one for a record without'
  },
  {
    file: MISSIONS,
    name: 'a handoff waits for every live run, not its own',
    from: '      await settling.get(previous.runId)',
    to: '      await Promise.allSettled([...consumeOperations])',
    expect: 'hands off one teammate’s run without waiting for another teammate’s to finish'
  },
  {
    file: MISSIONS,
    name: 'a teammate may run two missions at once',
    from: '          starting.has(owner) || [...active.values()].some((mission) => ownerKeyOf(mission.peer) === owner)',
    to: '          false',
    expect: 'refuses a second live mission for the same teammate, by name'
  },
  {
    file: MISSIONS,
    name: 'the live-mission cap is never reached',
    from: '        if (starting.size + active.size >= MAX_LIVE_MISSIONS) {',
    to: '        if (starting.size + active.size >= 99) {',
    expect: 'caps how many missions can be live at once, and says the number'
  },
  {
    file: VIEW,
    name: 'a reopened continuation shows the briefing as what the person said',
    from: "    const priorId = current.continuesFrom?.missionId",
    to: "    const priorId = undefined",
    expect: 'shows the words the person typed, not the briefing the host wrote'
  },
  {
    file: VIEW,
    name: 'a reopened divider forgets what was left in doubt',
    from: "    unsettledCount: checkpoint?.unsettledActions.length ?? 0,",
    to: "    unsettledCount: 0,",
    expect: 'rebuilds the divider from the route-switch checkpoint it resumed from'
  },
  {
    file: APPROVALS,
    name: 'an approval-mode run is sent the bare prompt, never the briefing',
    from: "        input: [{ type: 'text', text: runtimePrompt }]",
    to: "        input: [{ type: 'text', text: prompt }]",
    expect: "briefs an approval-mode mission with its teammates' waiting messages"
  },
  {
    file: APPROVALS,
    name: 'an approval-mode run shares before it has completed',
    from: '    if (!wasComplete && run.transcript.completed && run.peer !== undefined && peerExchange !== undefined) {',
    to: '    if (run.peer !== undefined && peerExchange !== undefined) {',
    expect: 'shares from an approval-mode run once it completes, and not before'
  },
  {
    file: BRIEFING,
    name: 'a received message is quoted with its share tags intact',
    from: '  const body = sanitizeInbound(message.text).replace(/\\n/g, \'\\n  \')',
    to: '  const body = message.text.replace(/\\n/g, \'\\n  \')',
    expect: 'defangs a share tag inside a received message so it cannot be echoed as a share'
  },
  {
    file: BRIEFING,
    name: 'messages that do not fit are sent anyway',
    from: '  while (prompt.length > MAX_RUNTIME_PROMPT_LENGTH && delivered.length > 0) {',
    to: '  while (false) {',
    expect: 'leaves out messages that do not fit, from the newest end, and counts them as still waiting'
  },
  {
    file: BRIEFING,
    name: 'a teammate with nobody to share with is still told how',
    from: '  const trailer = input.peer.others.length > 0 ? rosterSection(input.peer) : undefined',
    to: '  const trailer = rosterSection(input.peer)',
    expect: 'lists the other teammates and the exact share form, and only when there is someone to share with'
  },
  {
    file: SHARE,
    name: 'an upper-case share tag in a received message is left armed',
    from: "  return text.replace(/<(\\/?)locust-share/gi, '‹$1locust-share')",
    to: "  return text.replace(/<(\\/?)locust-share/g, '‹$1locust-share')",
    expect: 'defangs an inbound share tag so a received message cannot be echoed as a share'
  },
  {
    file: SHARE,
    name: 'the bubble keeps its share blocks',
    from: "  return text.replace(BLOCK, '').replace(/\\n{3,}/g, '\\n\\n').trimEnd()",
    to: '  return text.trimEnd()',
    expect: 'removes the blocks from the transcript and leaves the prose'
  },
  {
    file: PEERS,
    name: 'a share addressed to a stranger goes to the first teammate instead',
    from: '        const target = peer.others.find((entry) => entry.name.toLowerCase() === block.to.toLowerCase())',
    to: '        const target = peer.others[0]',
    expect: 'refuses a share addressed to someone who is not on the roster, out loud'
  },
  {
    file: MISSIONS,
    name: 'a run that did not complete still shares',
    from: '    if (mission.transcript.completed && mission.peer !== undefined && peerExchange !== undefined) {',
    to: '    if (mission.peer !== undefined && peerExchange !== undefined) {',
    expect: 'shares nothing from a run that did not complete'
  },
  {
    file: MISSIONS,
    name: 'the runtime is sent the bare prompt, never the briefing',
    from: '          process = options.runner.start(command, runtimePrompt, { signal: controller.signal })',
    to: '          process = options.runner.start(command, prompt, { signal: controller.signal })',
    expect: 'quotes a waiting message into the prompt as a claim, records it, and marks it delivered only once the run is live'
  },
  {
    file: MISSIONS,
    name: 'the ledger records the briefing as what the person said',
    from: '            runId,\n            prompt,\n            runtime,',
    to: '            runId,\n            prompt: runtimePrompt,\n            runtime,',
    expect: "keeps the person's own words as the recorded prompt, not the briefing"
  },
  {
    file: MISSIONS,
    name: 'a mission runs on messages its ledger could not record',
    from: '            await peerExchange.recordReceived(missionId, delivered, createdAt)',
    to: '            await Promise.resolve()',
    expect: 'refuses to run on messages the ledger cannot record, and says so'
  },
  {
    file: VIEW,
    name: 'the agent bubble shows the share block too',
    from: '    const text = stripDecisionBlocks(stripShareBlocks(message.text))',
    to: '    const text = stripDecisionBlocks(message.text)',
    expect: 'hides a share block from the agent bubble, keeping the prose'
  },
  {
    file: VIEW,
    name: 'a received exchange is filed as sent-only',
    from: "    if (message.direction === 'received') group.received = true\n",
    to: '',
    expect: 'groups an exchange by the other party and marks whether anything was received'
  },
  {
    file: STATUS,
    name: 'a stale ready flag alone is enough to call a runtime usable',
    from: '  return runtime.ready && runtime.status === \'ready\'',
    to: '  return runtime.ready',
    expect: 'never issues ACTIVE or READY for a runtime that is not ready'
  },
  {
    file: STATUS,
    name: 'a ready probe alone is enough, ignoring the readiness flag',
    from: '  return runtime.ready && runtime.status === \'ready\'',
    to: '  return runtime.status === \'ready\'',
    expect: 'never issues ACTIVE or READY for a runtime that is not ready'
  },
  {
    file: STATUS,
    name: 'a half-built adapter is advertised as ready',
    from: '      tag: \'PREVIEW\',',
    to: '      tag: \'READY\',',
    expect: 'never calls a half-built adapter live, even when its runtime is ready'
  },
  {
    file: STATUS,
    name: 'a planned runtime becomes selectable',
    from: '      tag: \'PLANNED\',\n      selectable: false,',
    to: '      tag: \'PLANNED\',\n      selectable: true,',
    expect: 'keeps a planned runtime non-interactive whatever discovery says'
  },
  {
    file: STATUS,
    name: 'every discovered runtime counts as connected',
    from: "  return runtimes.filter((runtime) => runtimeIsUsable(runtime) && integrationOf(runtime.id) !== 'planned')",
    to: '  return runtimes.filter(() => true)',
    expect: 'counts only usable runtimes as connected'
  },
  {
    file: STATUS,
    name: 'a blocked runtime is hidden behind an optimistic running mission',
    from: '  if (input.runtime !== undefined && !runtimeIsUsable(input.runtime)) {',
    to: '  if (false) {',
    expect: 'reports a blocked runtime even while a mission looks like it is running'
  },
  {
    file: STATUS,
    name: 'a completed mission prints clean over an unreadable ledger',
    from: '  if (hasIntegrityIssues) {',
    to: '  if (false) {',
    expect: 'will not present a completed mission as clean when its ledger is not'
  },
  {
    file: STATUS,
    name: 'the receipt says verified regardless of integrity issues',
    from: '  return integrityIssueCount === 0 ? \'verified\' : \'incomplete\'',
    to: '  return \'verified\'',
    expect: 'will not present a completed mission as clean when its ledger is not'
  },
  {
    file: APPROVALS,
    name: 'an unrecognized approval request is approved rather than refused',
    from: "            return { decision: 'reject' }",
    to: "            return { decision: 'accept' }",
    expect: 'refuses a request it does not understand rather than guessing'
  },
  {
    file: APPROVALS,
    name: 'always-allow becomes a durable grant instead of a session one',
    from: "  if (decision === 'approve-always') return 'acceptForSession'",
    to: "  if (decision === 'approve-always') return 'acceptForever'",
    expect: 'maps the product answers onto the protocol'
  },
  {
    file: APPROVALS,
    name: 'a dead runtime leaves approvals pending forever',
    from: "        child.onExit(() => {\n          stop(run, 'transport-lost', 'The runtime exited.')\n        })",
    to: "        child.onExit(() => {\n          runs.delete(run.runId)\n        })",
    expect: 'releases a pending approval when the runtime dies'
  },
  {
    file: APPROVALS,
    name: 'an already-answered approval can be answered again',
    from: '      if (pending === undefined) return false',
    to: '      if (pending === undefined) return true',
    expect: 'ignores a decision for an unknown or already-answered approval'
  },
  {
    file: APPROVALS,
    name: 'an undescribed command is presented as an ordinary one',
    from: "      summary: command.length > 0 ? 'Run a command' : 'Run a command it did not describe',",
    to: "      summary: 'Run a command',",
    expect: 'says so plainly when the runtime described nothing'
  },
  {
    file: STATUS,
    name: 'the control offers a handoff before the mission has a runId',
    from: "  return hasRunId ? 'available' : 'starting'",
    to: "  return 'available'",
    expect: 'offers a handoff only once the mission has a runId to address'
  },
  {
    file: STATUS,
    name: 'a second switch may race one already in flight',
    from: "  if (switching) return 'switching'",
    to: '',
    expect: 'refuses a second switch while one is in flight'
  },
  {
    file: STATUS,
    name: 'a finished mission still offers a handoff',
    from: "  if (!running) return 'idle'",
    to: '',
    expect: 'offers nothing when no mission is running'
  },
  {
    file: STATUS,
    name: 'a control that cannot do its job stays silent about why',
    from: "  if (availability === 'starting') return 'Waiting for the mission to start before it can be handed over'",
    to: '',
    expect: 'says why whenever the control cannot do its job'
  },
  {
    file: HANDOFF,
    name: 'the briefing tells the next runtime an unsettled action is done',
    from: "        'These actions STARTED and never reported back. Whether each took effect is unknown. '",
    to: "        'These actions were completed. '",
    expect: 'tells the new runtime to verify actions that never reported back'
  },
  {
    file: HANDOFF,
    name: 'the optional sections are ordered by size rather than by risk',
    from: '  return sections\n}',
    to: '  return sections.slice(0, 1).concat(sections.slice(1).reverse())\n}',
    expect: 'separates what finished from what did not'
  },
  {
    file: HANDOFF,
    name: 'the reserve is too small for the notice it has to hold',
    from: 'export const NOTICE_BUDGET = 120',
    to: 'export const NOTICE_BUDGET = 40',
    expect: 'reserves room for the longest notice every optional section could produce'
  },
  {
    file: HANDOFF,
    name: 'the omission notice is dropped, so a trimmed brief reads as complete',
    from: '    : `${kept.join(\'\\n\\n\')}\\n\\n${omissionNotice(omitted)}`',
    to: '    : kept.join(\'\\n\\n\')',
    expect: 'says so when detail was left out, rather than reading as complete'
  },
  {
    file: HANDOFF,
    name: 'the notice budget is reserved only after the first drop',
    from: '  const budget = MAX_HANDOFF_PROMPT_LENGTH - (sections.length > 1 ? NOTICE_BUDGET : 0)',
    to: '  const budget = MAX_HANDOFF_PROMPT_LENGTH',
    expect: 'reserves enough room for the longest possible omission notice'
  },
  {
    file: MISSIONS,
    name: 'the checkpoint is taken before the stopped run has settled',
    from: '      previous.controller.abort()\n      await settling.get(previous.runId)',
    to: '      previous.controller.abort()',
    expect: 'reconciles only after the stopped run has settled'
  },
  {
    file: MISSIONS,
    name: 'a handoff to the same runtime restarts the run instead of refusing',
    from: '      if (previous.runtime === runtime) {',
    to: '      if (false) {',
    expect: 'refuses a handoff to the runtime already running it, without stopping anything'
  },
  {
    file: MISSIONS,
    name: 'an unreconcilable ledger is handed off anyway',
    from: "      if (checkpoint.resumeSafety === 'unsafe') {",
    to: '      if (false) {',
    expect: 'refuses when the ledger cannot be reconciled, and says the run is stopped'
  },
  {
    file: MISSIONS,
    name: 'the continuation is not recorded, so the new mission looks unrelated',
    from: '              : { continuesFrom: continuation })',
    to: '              : {})',
    expect: 'starts a NEW mission that records what it continues from'
  },
  // ONE DECISION PATH (0.616, the PRD's R8): deny first, whatever was allowed before.
  {
    file: WHO_DECIDES,
    name: 'an earlier Always answers before the saved rules are read',
    from: "  // 2 and 3. The rules decide first, deny before allow (approval-rules.ts).\n",
    to: "  if (context.remembered) return { verdict: 'allow', by: 'earlier-always' }\n  // 2 and 3. The rules decide first, deny before allow (approval-rules.ts).\n",
    expect: 'denies by a saved rule, whatever Always was given earlier in the run'
  },
  {
    file: APPROVAL_RULES,
    name: 'a saved rule that says no is not checked first',
    from: "  if (denied !== undefined) return { decision: 'deny', rule: denied }",
    to: "  if (denied !== undefined && Date.now() < 0) return { decision: 'deny', rule: denied }",
    expect: 'denies by a saved rule, whatever Always was given earlier in the run'
  },
  {
    file: WHO_DECIDES,
    name: 'an earlier Always covers a command that reaches other programs',
    from: "    if (reach === undefined) return { verdict: 'allow', by: 'earlier-always' }",
    to: "    if (reach === undefined || reach !== undefined) return { verdict: 'allow', by: 'earlier-always' }",
    expect: 'asks again about a command that reaches other programs, though Always was given, and says why'
  },
  // Point at a part of the browser page. A focused run: node test/mutation-control.mjs page-point
  {
    file: join(ROOT, 'src', 'shared', 'browser-point.ts'),
    name: 'page-point: form values are quoted',
    from: '  return omitFormValues(html)',
    to: '  return html',
    expect: 'form values never appear in a pick',
    tests: ['src/main/a-part-of-the-browser-can-be-pointed-at.test.ts']
  },
  {
    file: join(ROOT, 'src', 'shared', 'browser-point.ts'),
    name: 'page-point: trimmed code is shown as if it were the whole of it',
    from: '  if (left > 0) lines.push(`(${String(left)} more characters of its code not shown)`)',
    to: '  if (false) lines.push(`(${String(left)} more characters of its code not shown)`)',
    expect: 'a trimmed HTML says it was trimmed',
    tests: ['src/main/a-part-of-the-browser-can-be-pointed-at.test.ts']
  },
  {
    file: join(ROOT, 'src', 'shared', 'browser-point.ts'),
    name: 'page-point: removing a chip keeps its picture',
    from: '  return current.filter((chip) => chip.id !== id)',
    to: '  return current',
    expect: 'removing a chip removes its picture',
    tests: ['src/main/a-part-of-the-browser-can-be-pointed-at.test.ts']
  },
  {
    file: join(ROOT, 'src', 'shared', 'browser-point.ts'),
    name: 'page-point: a colour is left off the styles',
    from: "  push('color', side(raw, 'color'))",
    to: "  push('color', undefined)",
    expect: 'lists the styles that matter for a visual fix',
    tests: ['src/main/a-part-of-the-browser-can-be-pointed-at.test.ts']
  },
  {
    file: join(ROOT, 'src', 'main', 'browser-host.ts'),
    name: 'page-point: organization policy does not turn pointing off',
    from: '      if (refused !== undefined) return refused',
    to: '      if (false) return refused',
    expect: 'browser: false turns pointing off too',
    tests: ['src/main/a-part-of-the-browser-can-be-pointed-at.test.ts']
  },
  {
    file: join(ROOT, 'src', 'main', 'browser-point.ts'),
    name: 'page-point: a password field is photographed',
    from: '    if (read.point.pictureNote !== undefined) return { ok: true, picked: read.point }',
    to: '    if (false) return { ok: true, picked: read.point }',
    expect: 'a password field is not pictured with its contents',
    tests: ['src/main/a-part-of-the-browser-can-be-pointed-at.test.ts']
  },
  {
    file: join(ROOT, 'src', 'main', 'browser-ipc.ts'),
    name: 'page-point: the window can hand a script to the picker',
    from: '    return host.point()',
    to: '    return host.point(event)',
    expect: 'the picker request goes through main only',
    tests: ['src/main/a-part-of-the-browser-can-be-pointed-at.test.ts']
  }
]

const REPORT = join(ROOT, 'mutation-result.json')

function runSuite() {
  // Delete the report first: a run that dies before writing one would otherwise
  // leave the previous report in place and read as "this mutation broke
  // nothing" -- a check that cannot go red, inside the tool that exists to
  // prove checks can.
  rmSync(REPORT, { force: true })
  try {
    // Focus only when every selected mutation names its proving files. Older
    // controls and an unfiltered sweep still run the entire desktop suite.
    const tests = selected.every((mutation) => mutation.tests !== undefined)
      ? [...new Set(selected.flatMap((mutation) => mutation.tests))] : []
    execFileSync('npx', ['vitest', 'run', ...tests, '--reporter', 'json', '--outputFile', 'mutation-result.json'], {
      cwd: ROOT,
      stdio: 'pipe',
      shell: true
    })
  } catch {
    // A red suite exits non-zero; the report is what we read, not the status.
  }
  if (!existsSync(REPORT)) return { failed: [], unparseable: true, total: -1 }
  const report = JSON.parse(readFileSync(REPORT, 'utf8'))
  const failed = []
  let unparseable = false
  for (const file of report.testResults ?? []) {
    if (file.status === 'failed' && (file.assertionResults ?? []).length === 0) unparseable = true
    for (const assertion of file.assertionResults ?? []) {
      if (assertion.status === 'failed') failed.push(assertion.title)
    }
  }
  return { failed, unparseable, total: report.numTotalTests ?? 0 }
}

let problems = 0

// An optional substring filter, so one new invariant can be checked in
// seconds instead of re-running the whole sweep. The full sweep is what CI
// runs; this is for the person writing a mutation. Added after a filtered
// run turned out to be impossible and two timed-out full runs each left a
// mutated file behind in the working tree -- one of which briefly looked
// like a real defect.
const filter = process.argv[2]
const selected =
  filter === undefined
    ? MUTATIONS
    : MUTATIONS.filter((mutation) => mutation.name.toLowerCase().includes(filter.toLowerCase()))
if (selected.length === 0) {
  console.error(`no mutation name contains "${filter}"`)
  process.exit(1)
}
if (filter !== undefined) console.error(`filtered to ${selected.length} of ${MUTATIONS.length} mutations`)

// A focused control reads and restores only its targets. An unrelated retired
// module must not prevent a new invariant from being checked.
const originals = new Map([...new Set(selected.map((mutation) => mutation.file))]
  .map((file) => [file, readFileSync(file, 'utf8')]))

try {
  const baseline = runSuite()
  if (baseline.unparseable || baseline.total <= 0 || baseline.failed.length > 0) {
    console.error(`baseline is not green: ${baseline.failed.join(', ')}`)
    process.exit(1)
  }
  console.error(`baseline green (${baseline.total} tests)\n`)

  for (const mutation of selected) {
    const target = mutation.file
    const original = originals.get(target)
    if (original === undefined) {
      // A selected target must have a snapshot so it can be restored.
      console.error(`  [SKIP] ${mutation.name} -- ${target} is not in the originals map`)
      problems += 1
      continue
    }
    if (!original.includes(mutation.from)) {
      // Name the usual cause instead of leaving it as a mystery. On Windows
      // with `core.autocrlf=true` a plain `git checkout -- <file>` rewrites
      // the file CRLF, and every anchor here that spans a newline stops
      // matching -- so an invariant silently stops being checked while the
      // sweep says something that sounds like a stale mutation. Three files
      // were in that state on 2026-09-04. `.gitattributes` pins LF now; this
      // says so out loud if it ever happens again.
      const crlf = original.includes(String.fromCharCode(13, 10))
      console.error(
        `  [SKIP] ${mutation.name} -- anchor not found` +
          (crlf ? ` (${target} has CRLF line endings; anchors here are LF -- normalise the file)` : '')
      )
      problems += 1
      continue
    }
    writeFileSync(target, original.replace(mutation.from, mutation.to), 'utf8')
    const result = runSuite()
    writeFileSync(target, original, 'utf8')

    if (result.unparseable || result.total !== baseline.total) {
      console.error(`  [INVALID] ${mutation.name} -- the file stopped running, so this red means nothing`)
      problems += 1
      continue
    }
    const caught = result.failed.includes(mutation.expect)
    if (!caught) problems += 1
    console.error(
      `  [${caught ? 'CAUGHT' : 'SURVIVED'}] ${mutation.name}` +
        (caught ? '' : `\n            expected "${mutation.expect}" to fail; failures: ${result.failed.join(', ') || 'none'}`)
    )
  }
} finally {
  for (const [file, text] of originals) writeFileSync(file, text, 'utf8')
  rmSync(REPORT, { force: true })
}

console.error(`\n${problems === 0 ? 'ALL MUTATIONS CAUGHT' : `${problems} MUTATION(S) UNACCOUNTED FOR`}`)
process.exit(problems === 0 ? 0 : 1)
