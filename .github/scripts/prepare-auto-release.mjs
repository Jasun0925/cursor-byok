import { execFileSync } from 'node:child_process';
import { appendFileSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export const versionFiles = [
  'apps/desktop/package.json',
  'apps/desktop/package-lock.json',
  'apps/desktop/src-tauri/tauri.conf.json',
  'apps/desktop/src-tauri/Cargo.toml',
  'Cargo.lock',
];
const stable = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const desktopLockVersion = /(\[\[package\]\]\r?\nname = "cursor-byok-desktop"\r?\nversion = ")([^"]+)(")/;
const cargoVersion = /(^version = ")([^"]+)(")/m;
const compare = (a, b) => {
  const left = a.split('.').map(Number), right = b.split('.').map(Number);
  for (let i = 0; i < 3; i++) if (left[i] !== right[i]) return left[i] - right[i];
  return 0;
};

export function isReleaseChange(path) {
  return !/^(docs\/|\.agents\/|\.github\/(ISSUE_TEMPLATE|PULL_REQUEST_TEMPLATE)\/)/.test(path)
    && !/^(AGENTS\.md|README(?:[.-][^/]*)?|LICENSE(?:\.[^/]*)?|CONTRIBUTING\.md|UPSTREAM\.md|\.gitignore)$/.test(path);
}

function updateVersions(cwd, next) {
  const sources = versionFiles.map(path => readFileSync(resolve(cwd, path), 'utf8'));
  const [pkg, lock, tauri] = sources.slice(0, 3).map(text => JSON.parse(text));
  const versions = [pkg.version, lock.version, lock.packages?.['']?.version, tauri.version,
    sources[3].match(cargoVersion)?.[2], sources[4].match(desktopLockVersion)?.[2]];
  if (!stable.test(versions[0]) || versions.some(version => version !== versions[0])) {
    throw new Error(`Desktop manifests and locks must have one stable version: ${versions.join(', ')}`);
  }
  if (!next) return versions[0];
  pkg.version = lock.version = lock.packages[''].version = tauri.version = next;
  const updated = [pkg, lock, tauri].map(value => JSON.stringify(value, null, 2) + '\n');
  updated.push(sources[3].replace(cargoVersion, `$1${next}$3`));
  updated.push(sources[4].replace(desktopLockVersion, `$1${next}$3`));
  versionFiles.forEach((path, i) => writeFileSync(resolve(cwd, path), updated[i]));
}

// Called only for a successful push CI run on this repository's main branch.
// Push the version commit and tag atomically: a concurrent main update wins.
export function prepareAutoRelease({ cwd, expectedSha }) {
  const git = (...args) => execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  const skip = reason => ({ should_publish: 'false', reason });
  if (!/^[0-9a-f]{40}$/.test(expectedSha ?? '')) throw new Error('Missing CI commit SHA');
  git('fetch', 'origin', 'main', '--tags');
  const remoteHead = () => git('ls-remote', 'origin', 'refs/heads/main').split(/\s/)[0];
  if (remoteHead() !== expectedSha) return skip('A newer main commit superseded this CI run');
  if (git('rev-parse', 'HEAD') !== expectedSha) throw new Error('Checkout must match the successful CI commit');
  if (git('status', '--porcelain')) throw new Error('Release checkout must be clean');
  if (git('tag', '--points-at', 'HEAD').split('\n').some(tag => /^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(tag))) {
    return skip('The tested commit already has a release tag');
  }
  const tags = git('tag', '--list', 'v*').split('\n').filter(tag => stable.test(tag.slice(1)))
    .sort((a, b) => compare(b.slice(1), a.slice(1)));
  const merged = new Set(git('tag', '--merged', 'HEAD').split('\n'));
  const base = tags.find(tag => merged.has(tag));
  if (!base) throw new Error('Automatic releases require an existing stable release tag on main');
  const changes = git('diff', '--name-only', '--no-renames', `${base}..HEAD`).split('\n').filter(Boolean);
  if (!changes.some(isReleaseChange)) return skip('No application or build changes since the last release tag');
  const current = updateVersions(cwd);
  const latest = tags[0].slice(1);
  let version = current;
  if (compare(current, latest) <= 0) {
    const parts = latest.split('.').map(Number);
    parts[2]++;
    version = parts.join('.');
  }
  updateVersions(cwd, version);
  git('config', 'user.name', 'github-actions[bot]');
  git('config', 'user.email', '41898282+github-actions[bot]@users.noreply.github.com');
  git('add', '--', ...versionFiles);
  if (git('diff', '--cached', '--name-only')) git('commit', '-m', `chore(release): v${version}`);
  const tag = `v${version}`;
  git('tag', '-a', tag, '-m', `Cursor BYOK ${tag}`);
  try {
    git('push', '--atomic', 'origin', 'HEAD:refs/heads/main', `refs/tags/${tag}`);
  } catch (error) {
    if (remoteHead() !== expectedSha) return skip('Main advanced before the atomic release push');
    throw error;
  }
  return { should_publish: 'true', version, release_sha: git('rev-parse', 'HEAD') };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const result = prepareAutoRelease({ cwd: process.cwd(), expectedSha: process.env.CI_HEAD_SHA });
  console.log(JSON.stringify(result));
  if (process.env.GITHUB_OUTPUT) {
    for (const [name, value] of Object.entries(result)) appendFileSync(process.env.GITHUB_OUTPUT, `${name}=${value}\n`);
  }
}
