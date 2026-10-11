// Diagnostic only. Executes selected methods from the locally installed Cursor
// bundle with fake services and a manual clock. No Cursor state or files edited.
// Run: node docs/upstream-issue-reviews/2026-10-10-issue341-callback-probe.mjs
import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import path from 'node:path';

const bundlePath = '/Applications/Cursor.app/Contents/Resources/app/out/vs/workbench/workbench.desktop.main.js';
const source = fs.readFileSync(bundlePath, 'utf8');
const executorPath = '/Applications/Cursor.app/Contents/Resources/app/extensions/cursor-agent-exec/dist/main.js';
const executorSource = fs.readFileSync(executorPath, 'utf8');
function extract(start, end, text = source) {
  const a = text.indexOf(start);
  assert(a >= 0, `Missing start marker: ${start}`);
  const b = text.indexOf(end, a + start.length);
  assert(b > a, `Missing end marker: ${end}`);
  return text.slice(a, b);
}
const registrySource = extract('rbh=class extends we{', ',mn(sCt,rbh,1)').slice(4);
const handlerSource = extract('async handleFileChange(e,t){', 'async _handleNotebookFileChange(');
const lookupSource = extract('function JZi(', 'var CCh=');
const checkpointSource = extract('async updateLastCheckpointWithFileIfNotTracked(', '},ie([zs(');
const writeSource = extract('class $u{constructor(e,t,r,{workspacePaths:', 'const ju=', executorSource);
const trackerSource = extract('class Ni{constructor(e){this.onChange=e}', 'function Fi(', executorSource);
const results = [];
const flush = async () => { for (let i = 0; i < 4; i++) await new Promise(setImmediate); };

function fixture() {
  let nextId = 0;
  const timers = new Map();
  const bubbles = new Map();
  const originals = {};
  const checkpoints = [];
  const diffs = [];
  const emitted = [];
  const handle = { composerId: 'probe', data: { composerId: 'probe', originalFileStates: originals } };
  const sandbox = vm.createContext({
    we: class { _register(v) { return v; } dispose() {} },
    ve: class { event = () => {}; fire() {} },
    setTimeout(fn, ms) { const id = ++nextId; timers.set(id, { fn, ms }); return id; },
    clearTimeout(id) { timers.delete(id); },
    jwt: path => ({ path, toString: () => `file://${path}` }),
    IFn: () => false,
    sy: text => text.split(/\r?\n/),
    jj: () => false,
    rJe: () => false,
    zn: () => `diff-${++nextId}`,
    Gl: class { constructor(start, end) { this.startLineNumber = start; this.endLineNumberExclusive = end; } },
    Oee: 'probe',
    hs: { TOOL_FORMER: 'tool' },
    nt: { TASK_V2: 'task' },
  });
  const Registry = vm.runInContext(`(${registrySource})`, sandbox);
  vm.runInContext(lookupSource, sandbox);
  const Handler = vm.runInContext(`(class { ${handlerSource} })`, sandbox);
  const Checkpoint = vm.runInContext(`(class { ${checkpointSource} })`, sandbox);
  const registry = new Registry();
  const service = new Handler();
  const dataService = {
    async getComposerHandleById(id) { return id === 'probe' ? handle : undefined; },
    getComposerData: () => handle.data,
    getToolFormer: () => ({
      getBubbleIdByToolCallId: id => bubbles.get(id),
      getBubbleByShortToolCallReferenceId: () => undefined,
    }),
    getLoadedConversation: () => [],
    updateComposerDataSetStore(_handle, update) {
      update((key, uri, value) => { assert.equal(key, 'originalFileStates'); originals[uri] = value; });
    },
  };
  Object.assign(service, {
    composerDataService: dataService,
    pendingApprovalRegistry: registry,
    _pendingFileChangesByKey: new Map(),
    _pendingWaitStartedByKey: new Set(),
    workspaceContextService: {},
    composerCheckpointService: { async updateLastCheckpointWithFileIfNotTracked(_h, uri, options) { checkpoints.push({ uri: uri.toString(), ...options }); } },
    composerFileService: { async revertFile() {} },
    composerCodeBlockService: { getInlineDiff: () => undefined },
    async maybeHoldGlassLspTextModelReference() {},
    diffDecorationVisibilityService: { shouldHideInlineDiffs: () => false },
    textModelService: { async createModelReference() { return { dispose() {} }; } },
    textFileService: { files: { async resolve() {} } },
    inlineDiffService: {
      inlineDiffs: { nonReactive: () => diffs },
      async addDecorationsOnlyDiff(diff) { diffs.push(diff); },
      getHandlerByDiffId: id => diffs.find(d => d.id === id),
      updateComposerMetadata() {}, remove() {},
    },
    composerEventService: { fireDidAiEditFile(event) { emitted.push(event); } },
    _composerLogService: { info() {} },
  });
  return {
    service, registry, timers, bubbles, originals, checkpoints, diffs, emitted, Checkpoint,
    async change(id = 'call-1', before = 'old\n') {
      await service.handleFileChange('probe', { path: '/isolated/example.ts', before, after: 'new\n', metadata: { toolCallId: id } });
      await flush();
    },
    async createBubble(id = 'call-1') { bubbles.set(id, `bubble-${id}`); registry.signalBubbleCreated('probe', id); await flush(); },
    async expire() { const pending = [...timers.values()]; timers.clear(); for (const timer of pending) { assert.equal(timer.ms, 5000); timer.fn(); } await flush(); },
    summary() { return { originals: Object.keys(originals).length, checkpoints: checkpoints.length, diffs: diffs.length, queued: service._pendingFileChangesByKey.size, timers: timers.size }; },
  };
}

// Run the actual local writer with in-memory filesystem dependencies. Its
// conditions, before-content capture, callback await and success return are native.
async function nativeWrite(f, { bytes = false, oldSize = 4 } = {}) {
  let content = 'old\n';
  const captures = [];
  const wire = new Proxy({}, { get: () => class { constructor(value) { Object.assign(this, value); } } });
  const ctx = { withName() { return this; } };
  const sandbox = vm.createContext({
    Buffer, s: path, Xn: 52428800, fi: wire,
    Nu: (_r, value) => value, Fu() {}, C: { VI: value => ({ ctx: value }) },
    Lu: (_ctx, _name, _ms, fn) => fn(),
    O: { o1: value => value, fl: () => false }, wc: () => false,
    n: { stat: async () => ({ size: oldSize }), mkdir: async () => {}, open: async () => ({ sync() {}, close() {} }) },
    bc: async value => ({ resolvedPath: value }),
    _e: { yR: async () => content, Bh: async (_path, text) => ({ buffer: Buffer.from(text), lines: text.split('\n').length }) },
    qu: async (_path, buffer) => { content = buffer.toString(); },
    Bu: { O_RDONLY: 0 },
  });
  const Tracker = vm.runInContext(`(${trackerSource})`, sandbox);
  const Writer = vm.runInContext(`(${writeSource})`, sandbox);
  const tracker = new Tracker(async change => {
    captures.push(change);
    // Same toolCallId guard as the native createSessionExecManager callback.
    if (change.metadata?.toolCallId) await f.service.handleFileChange('probe', change);
  });
  const writer = new Writer(tracker, { shouldBlockWrite: async () => null }, {}, { workspacePaths: ['/isolated'], workspacePath: '/isolated' });
  const result = await writer.execute(ctx, { path: '/isolated/example.ts', fileText: 'new\n', toolCallId: 'call-1', fileBytes: bytes ? Buffer.from('new\n') : Buffer.alloc(0), returnFileContentAfterWrite: false });
  await flush();
  return { result: result.result.case, content, callbacks: captures.length };
}

let f = fixture();
await f.createBubble(); await f.change();
assert.equal(f.originals['file:///isolated/example.ts'].content, 'old\n');
assert.equal(f.diffs[0].originalTextLines.join('\n'), 'old\n');
assert.equal(f.diffs[0].toolCallId, 'call-1');
assert.equal(f.checkpoints[0].forceModelContent.join('\n'), 'old\n');
results.push({ scenario: 'bubble exists before callback', ...f.summary() });

// Omit before explicitly; the helper has a default for existing-file scenarios.
f = fixture(); await f.createBubble();
await f.service.handleFileChange('probe', { path: '/isolated/new.ts', after: 'new\n', metadata: { toolCallId: 'call-1' } });
assert.equal(f.originals['file:///isolated/new.ts'].isNewlyCreated, true);
assert.equal(f.diffs[0].originalTextLines.length, 0);
results.push({ scenario: 'new file', ...f.summary() });

f = fixture(); await f.change();
assert.equal(f.checkpoints.length, 0);
assert.equal(f.service._pendingFileChangesByKey.size, 1);
await f.createBubble();
assert.equal(f.checkpoints.length, 1); assert.equal(f.diffs.length, 1);
results.push({ scenario: 'bubble arrives before 5000ms timeout', ...f.summary() });

f = fixture(); await f.change(); await f.expire(); await f.createBubble();
assert.equal(f.checkpoints.length, 0); assert.equal(f.diffs.length, 0);
assert.equal(f.service._pendingFileChangesByKey.size, 0);
results.push({ scenario: 'bubble arrives after timeout: registration permanently lost', ...f.summary() });

f = fixture(); f.registry.signalBubbleCreated('unrelated-composer', 'call-1'); await f.change();
assert.equal(f.timers.size, 0); assert.equal(f.service._pendingFileChangesByKey.size, 0);
await f.createBubble();
assert.equal(f.checkpoints.length, 0); assert.equal(f.diffs.length, 0);
results.push({ scenario: 'same toolCallId in unrelated composer: immediate drain drops change', ...f.summary() });

f = fixture(); await f.createBubble();
await assert.rejects(() => f.service.handleFileChange('probe', { path: '/isolated/example.ts', before: 'old', after: 'new' }), /No toolCallId/);
results.push({ scenario: 'missing toolCallId: explicit error', ...f.summary() });

f = fixture(); await f.createBubble();
f.service.composerCheckpointService.updateLastCheckpointWithFileIfNotTracked = async () => { throw new Error('synthetic checkpoint failure'); };
await assert.rejects(() => f.change(), /synthetic checkpoint failure/);
assert.equal(Object.keys(f.originals).length, 1); assert.equal(f.diffs.length, 0);
results.push({ scenario: 'checkpoint storage error stops inline diff registration', ...f.summary() });

f = fixture();
let checkpointWrites = 0;
const cp = new f.Checkpoint();
Object.assign(cp, {
  _composerDataService: { getLastBubbleWhere: () => undefined },
  _composerCheckpointStorageService: { async storeCheckpoint() { checkpointWrites++; }, async updateCheckpoint() { checkpointWrites++; } },
});
await cp.updateLastCheckpointWithFileIfNotTracked({ data: { composerId: 'probe' } }, { path: '/isolated/example.ts' }, { forceModelContent: ['old'] });
assert.equal(checkpointWrites, 0);
results.push({ scenario: 'no earlier bubble checkpoint: native checkpoint update silently skips', checkpointWrites });

f = fixture(); await f.createBubble();
let write = await nativeWrite(f);
assert.equal(write.result, 'success'); assert.equal(write.content, 'new\n');
assert.equal(f.originals['file:///isolated/example.ts'].content, 'old\n');
results.push({ scenario: 'native writer and callback with existing bubble', write, ...f.summary() });

f = fixture(); write = await nativeWrite(f);
assert.equal(write.result, 'success'); assert.equal(write.content, 'new\n');
assert.equal(f.service._pendingFileChangesByKey.size, 1);
await f.expire(); await f.createBubble();
assert.equal(f.checkpoints.length, 0); assert.equal(f.diffs.length, 0);
results.push({ scenario: 'native writer returns success but callback later expires', write, ...f.summary() });

f = fixture(); await f.createBubble(); write = await nativeWrite(f, { bytes: true });
assert.equal(write.callbacks, 0); assert.equal(write.result, 'success');
results.push({ scenario: 'native binary write skips callback', write, ...f.summary() });

f = fixture(); await f.createBubble(); write = await nativeWrite(f, { oldSize: 52428801 });
assert.equal(write.callbacks, 0); assert.equal(write.result, 'success');
results.push({ scenario: 'native existing file above 50MiB skips callback', write, ...f.summary() });

console.log(JSON.stringify({
  bundlePath,
  bundleSha256: createHash('sha256').update(source).digest('hex'),
  executorPath,
  executorSha256: createHash('sha256').update(executorSource).digest('hex'),
  scope: 'Unmodified extracted Cursor methods, mocked service/filesystem dependencies and manual 5000ms timers; no live Cursor UI/RPC or disk edits.',
  result: `${results.length} assertions scenarios passed`,
  results,
}, null, 2));
