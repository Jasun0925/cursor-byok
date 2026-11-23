import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { isReleaseChange, prepareAutoRelease, versionFiles } from './prepare-auto-release.mjs';

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'auto-release-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const cwd = join(root, 'repo'), remote = join(root, 'remote.git');
  mkdirSync(cwd);
  const git = (...args) => execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  const write = (path, value) => {
    mkdirSync(join(cwd, path, '..'), { recursive: true });
    writeFileSync(join(cwd, path), value);
  };
  git('init', '-b', 'main');
  git('config', 'user.name', 'Test');
  git('config', 'user.email', 'test@example.com');
  git('config', 'commit.gpgsign', 'false');
  git('config', 'tag.gpgsign', 'false');
  const setVersion = version => {
    write(versionFiles[0], JSON.stringify({ name: 'cursor-byok-desktop', version }, null, 2) + '\n');
    write(versionFiles[1], JSON.stringify({ version, packages: { '': { version }, dependency: { version: '9.0.0' } } }, null, 2) + '\n');
    write(versionFiles[2], JSON.stringify({ version, plugins: { updater: { pubkey: 'preserved' } } }, null, 2) + '\n');
    write(versionFiles[3], `[package]\nname = "cursor-byok-desktop"\nversion = "${version}"\n`);
    write(versionFiles[4], `[[package]]\nname = "cursor-server"\nversion = "0.1.0"\n\n[[package]]\nname = "cursor-byok-desktop"\nversion = "${version}"\n`);
  };
  const commit = message => { git('add', '.'); git('commit', '-m', message); return git('rev-parse', 'HEAD'); };
  const push = () => git('push', 'origin', 'main', '--tags');
  const remoteSha = ref => git('ls-remote', 'origin', ref).split(/\s/)[0];
  setVersion('1.1.0');
  write('server/src/lib.rs', '// initial\n');
  commit('initial release');
  git('tag', 'v1.1.0');
  git('init', '--bare', remote);
  git('remote', 'add', 'origin', remote);
  push();
  return { root, cwd, remote, git, write, setVersion, commit, push, remoteSha };
}

function codeChange(f) {
  f.write('server/src/lib.rs', '// next feature\n');
  const sha = f.commit('feature');
  f.push();
  return sha;
}

test('publishes one patch version, updates all manifests, and preserves dependencies and key', t => {
  const f = fixture(t), expectedSha = codeChange(f);
  const result = prepareAutoRelease({ cwd: f.cwd, expectedSha });
  assert.equal(result.should_publish, 'true');
  assert.equal(result.version, '1.1.1');
  assert.equal(f.remoteSha('refs/heads/main'), result.release_sha);
  assert.equal(f.remoteSha('refs/tags/v1.1.1^{}'), result.release_sha);
  assert.equal(f.git('rev-parse', 'HEAD^'), expectedSha);
  assert.deepEqual(f.git('diff', '--name-only', 'HEAD^', 'HEAD').split('\n').sort(), [...versionFiles].sort());
  const pkg = JSON.parse(readFileSync(join(f.cwd, versionFiles[0])));
  const lock = JSON.parse(readFileSync(join(f.cwd, versionFiles[1])));
  const tauri = JSON.parse(readFileSync(join(f.cwd, versionFiles[2])));
  assert.equal(pkg.version, '1.1.1');
  assert.equal(lock.version, '1.1.1');
  assert.equal(lock.packages[''].version, '1.1.1');
  assert.equal(lock.packages.dependency.version, '9.0.0');
  assert.equal(tauri.version, '1.1.1');
  assert.equal(tauri.plugins.updater.pubkey, 'preserved');
  assert.match(readFileSync(join(f.cwd, versionFiles[3]), 'utf8'), /version = "1.1.1"/);
  assert.match(readFileSync(join(f.cwd, versionFiles[4]), 'utf8'), /name = "cursor-server"\nversion = "0.1.0"/);
  assert.match(readFileSync(join(f.cwd, versionFiles[4]), 'utf8'), /name = "cursor-byok-desktop"\nversion = "1.1.1"/);
  assert.equal(prepareAutoRelease({ cwd: f.cwd, expectedSha }).should_publish, 'false');
  assert.equal(prepareAutoRelease({ cwd: f.cwd, expectedSha: result.release_sha }).should_publish, 'false');
});

test('skips documentation-only changes without pushing a tag or changing versions', t => {
  const f = fixture(t);
  f.write('docs/guide.md', 'instructions');
  f.write('README.zh-CN.md', 'readme');
  f.write('README-EN.md', 'readme');
  f.write('.agents/skills/release/SKILL.md', 'policy');
  const expectedSha = f.commit('docs'); f.push();
  assert.equal(prepareAutoRelease({ cwd: f.cwd, expectedSha }).should_publish, 'false');
  assert.equal(f.remoteSha('refs/heads/main'), expectedSha);
  assert.equal(f.git('tag', '--list'), 'v1.1.0');
});

test('runtime Markdown, deleted code, workflows and build inputs count as release changes', t => {
  for (const path of ['server/prompts/system.md', '.github/workflows/ci.yml', 'Cargo.lock', 'apps/desktop/src/App.tsx']) {
    assert.equal(isReleaseChange(path), true, path);
  }
  const f = fixture(t);
  f.git('rm', 'server/src/lib.rs');
  const expectedSha = f.commit('remove obsolete module'); f.push();
  assert.equal(prepareAutoRelease({ cwd: f.cwd, expectedSha }).version, '1.1.1');
});

test('uses an explicitly prepared higher stable version', t => {
  const f = fixture(t); f.setVersion('1.2.0');
  const expectedSha = f.commit('prepare minor release'); f.push();
  const result = prepareAutoRelease({ cwd: f.cwd, expectedSha });
  assert.equal(result.version, '1.2.0');
  assert.equal(result.release_sha, expectedSha);
});

test('skips superseded CI and rejects an untested checkout or dirty files', t => {
  const f = fixture(t), oldSha = f.git('rev-parse', 'HEAD'), expectedSha = codeChange(f);
  assert.equal(prepareAutoRelease({ cwd: f.cwd, expectedSha: oldSha }).should_publish, 'false');
  f.git('checkout', oldSha);
  assert.throws(() => prepareAutoRelease({ cwd: f.cwd, expectedSha }), /Checkout must match/);
  f.git('checkout', 'main'); f.write('untracked.txt', 'dirty');
  assert.throws(() => prepareAutoRelease({ cwd: f.cwd, expectedSha }), /must be clean/);
  assert.throws(() => prepareAutoRelease({ cwd: f.cwd, expectedSha: 'main' }), /Missing CI commit SHA/);
});

test('rejects mismatched manifests before publishing anything', t => {
  const f = fixture(t);
  f.write(versionFiles[0], '{"version":"1.2.0"}\n');
  const expectedSha = f.commit('inconsistent versions'); f.push();
  assert.throws(() => prepareAutoRelease({ cwd: f.cwd, expectedSha }), /one stable version/);
  assert.equal(f.remoteSha('refs/heads/main'), expectedSha);
  assert.equal(f.remoteSha('refs/tags/v1.1.1'), '');
});

test('a main update racing the release push leaves main and tags untouched by release', t => {
  const f = fixture(t), expectedSha = codeChange(f);
  f.write('README.md', 'concurrent main change');
  const newerSha = f.commit('concurrent update');
  // Transfer the competing commit without updating main, then race during pre-push.
  f.git('push', 'origin', `${newerSha}:refs/heads/race`);
  f.git('reset', '--hard', expectedSha);
  const quoted = text => "'" + text.replaceAll("'", "'\\''") + "'";
  writeFileSync(join(f.cwd, '.git/hooks/pre-push'),
    `#!/bin/sh\ngit --git-dir=${quoted(f.remote)} update-ref refs/heads/main ${newerSha}\n`, { mode: 0o755 });
  const result = prepareAutoRelease({ cwd: f.cwd, expectedSha });
  assert.equal(result.should_publish, 'false');
  assert.equal(f.remoteSha('refs/heads/main'), newerSha);
  assert.equal(f.remoteSha('refs/tags/v1.1.1'), '');
});

test('tag rejection rolls back the atomic main update', t => {
  const f = fixture(t), expectedSha = codeChange(f);
  writeFileSync(join(f.remote, 'hooks/update'), '#!/bin/sh\ncase "$1" in refs/tags/*) exit 1;; esac\n', { mode: 0o755 });
  assert.throws(() => prepareAutoRelease({ cwd: f.cwd, expectedSha }), /Command failed/);
  assert.equal(f.remoteSha('refs/heads/main'), expectedSha);
  assert.equal(f.remoteSha('refs/tags/v1.1.1'), '');
});

test('reserved stable tags are never overwritten', t => {
  const f = fixture(t), expectedSha = codeChange(f);
  f.git('tag', 'v1.1.3', expectedSha + '^'); f.push();
  assert.equal(prepareAutoRelease({ cwd: f.cwd, expectedSha }).version, '1.1.4');
  assert.equal(f.remoteSha('refs/tags/v1.1.3'), f.git('rev-parse', expectedSha + '^'));
});

test('manual beta release tags do not start a second automatic release', t => {
  const f = fixture(t); f.setVersion('1.2.0-beta.1');
  const expectedSha = f.commit('manual beta');
  f.git('tag', 'v1.2.0-beta.1'); f.push();
  assert.equal(prepareAutoRelease({ cwd: f.cwd, expectedSha }).should_publish, 'false');
  assert.equal(f.remoteSha('refs/heads/main'), expectedSha);
});
