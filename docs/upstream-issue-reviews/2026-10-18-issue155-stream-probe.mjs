// Run the installed Cursor stream-end guard in isolation. No UI, network, or edits.
// Usage: node <this file> /Applications/Cursor.app/Contents/Resources/app/out/vs/workbench/workbench.glass.main.js
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

const path = process.argv[2];
assert(path, 'Pass the installed Cursor workbench bundle path');
const source = readFileSync(path, 'utf8');
const anchor = source.indexOf('Stream ended without turnEnded');
const start = source.lastIndexOf('async function f()', anchor);
const end = source.indexOf('return{interactionStream:', anchor);
assert(anchor >= 0 && start >= 0 && end > anchor, 'Cursor stream guard layout changed');
const guard = source.slice(start, end);
assert(guard.length < 10000, 'Unexpected extraction range');

// Execute the original guard; replace only its surrounding I/O and error type.
const check = new Function('messages', `
  const e = (async function* () { yield* messages; })();
  const n = { onServerSentHeartbeat() {}, reset() {}, onStreamEnded() {} };
  const t = {}, r = { aborted: false }, s = { aborted: false };
  const i = () => {}, o = () => {}, a = () => {};
  let h = false, p = false;
  const sink = () => ({ async write() {}, close() {} });
  const l = sink(), c = sink(), u = sink(), d = sink();
  // Surrounding binding names in Cursor 3.25.9.
  const yPe = { warn() {}, debug() {} };
  const ztl = () => 'test', lDk = () => false;
  class uL extends Error {
    constructor(message, options) { super(message); Object.assign(this, options); }
  }
  ${guard}
  return f();
`);

const cases = [
  ['empty_success_stream', [], false],
  ['checkpoint_only', [{ message: { case: 'conversationCheckpointUpdate', value: {} } }], false],
  ['turn_ended_then_success', [{ message: {
    case: 'interactionUpdate', value: { message: { case: 'turnEnded', value: {} } },
  } }], true],
];
const results = [];
for (const [name, messages, expected] of cases) {
  let accepted = false, error;
  try { await check(messages); accepted = true; }
  catch (failure) { error = { message: failure.message, isTransport: failure.isTransport }; }
  assert.equal(accepted, expected, name);
  if (!expected) assert.equal(error.isTransport, true, name);
  results.push({ name, accepted, ...(error ? { error } : {}) });
}
console.log(JSON.stringify({
  bundle_sha256: createHash('sha256').update(source).digest('hex'),
  guard_sha256: createHash('sha256').update(guard).digest('hex'),
  scope: 'Installed client guard with mocked I/O; not a live Cursor UI run',
  results,
}, null, 2));
