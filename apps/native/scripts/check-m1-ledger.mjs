// No installs: use the reference repo's already-built reader.
// node scripts/check-m1-ledger.mjs <absolute reader dist/index.js> <ledger directory> <mission id>
import assert from 'node:assert/strict';
import { pathToFileURL } from 'node:url';
const [reader, rootDirectory, missionId, phase = 'completed'] = process.argv.slice(2);
const { createFileMissionLedger } = await import(pathToFileURL(reader));
const ledger = createFileMissionLedger({ rootDirectory });
const result = await ledger.readMission(missionId);
assert.ok(result.mission, JSON.stringify(result));
assert.equal(result.issues.length, 0, JSON.stringify(result.issues));
assert.equal(result.mission.phase, phase);
// A note Locust adds once a turn has ended (a turn's receipt, a carry-on) follows its ending, as Electron writes it:
// the run's own last event is the last one that is not such a note.
assert.equal(result.mission.events.filter((event) => event.type !== 'adapter.diagnostic').at(-1).type, `run.${phase}`);
assert.equal(result.mission.issues.length, 0);
console.log(`TypeScript reader accepted ${result.mission.events.length} Rust events without issues.`);
