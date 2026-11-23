---
name: release
description: Prepare, authorize, publish, troubleshoot, and verify Cursor BYOK desktop GitHub Releases. Use for version bumps, release tags, GitHub Actions release runs, updater manifests, signing, or release-readiness checks.
---

# Desktop release

Release through `.github/workflows/release.yml`. Preserve the updater manifests used by the current workflow: Tauri uses `latest.json`, Windows portable updates use `portable-latest.json`, and legacy `v0.0.49` clients use `update.json`.

## Publication authority

- Release only to the user's explicitly selected repository. This fork uses `origin = haohaomin/cursor-byok`; `upstream = leookun/cursor-byok` is a source of code, not a publication destination.
- Automatic stable releases are authorized by the repository owner: approved code pushes or merges to `main` publish after successful CI. Verify the selected repository and push permission before merging or pushing. Manual tag publication still requires explicit authorization and the repository owner (`haohaomin`) as actor. Honor existing task authorization.
- Pushing application/build changes to `main` starts CI and then automatic publication. Pushing a `v*` tag, rerunning the release workflow, and publishing or editing a GitHub Release are also publication actions. Use a feature branch for work that should not yet publish.
- Without that authorization, restrict work to inspection, local edits, validation, and a release-ready commit or branch. Do not infer publication permission from requests such as “prepare”, “check”, or “ready to release”.
- Never print, commit, or upload `.tauri/cursor-byok.key` anywhere except the repository's `TAURI_SIGNING_PRIVATE_KEY` Actions Secret when the author explicitly requests that secret configuration.
- Never delete, replace, or move an existing tag or published Release without separate explicit authorization.

## Version and GitHub Release policy

- Do not use GitHub prereleases. Keep `prerelease: false` for every release and publish the completed release as Latest.
- Use `vMAJOR.MINOR.PATCH` for a stable tag, for example `v0.1.0`.
- Use standard SemVer `vMAJOR.MINOR.PATCH-beta.N` for a test tag, for example `v0.1.0-beta.1`. A beta is still a normal GitHub Release, not a GitHub prerelease. Make its title or body visibly say Beta.
- This normal-Release rule is required because both installed update clients resolve assets through GitHub's `/releases/latest/download/` path, which excludes GitHub prereleases.
- Windows beta builds must use the NSIS bundle. WiX/MSI rejects nonnumeric prerelease identifiers such as `beta.1`; do not weaken the SemVer tag to accommodate MSI.
- Release only from a `v*` tag whose commit is contained in `origin/main`. The tag must equal `v<version>` from the desktop manifests.
- Keep automatic publication gated on a successful same-repository `main` push CI run. Skip stale CI results and documentation-only changes. The workflow atomically pushes a version commit and matching tag using `GITHUB_TOKEN`, then builds that exact commit in the same run. Do not add a second push-triggered release chain or an extra personal access token. Owner-pushed tags remain available for explicitly prepared releases.
- Never republish an already published version. Select a new version instead.

## Release sources

Keep the desktop version identical in the manifests and their locks:

```text
cursor-byok/
├── Cargo.lock
├── apps/desktop/
│   ├── package.json
│   ├── package-lock.json
│   └── src-tauri/
│       ├── Cargo.toml
│       └── tauri.conf.json
├── protocols/cursor/
│   ├── agent_v1.proto
│   └── aiserver_v1.proto
└── .github/workflows/release.yml
```

The two listed Proto files are required build inputs and must be committed. Check `server/build.rs` and protobuf imports for additional inputs; preserve the repository's tracked protocol sources.

## Automatic stable releases

```text
main push → CI succeeds → release.yml receives workflow_run
  → confirm the tested commit is still main
  → compare application/build changes against the last stable tag
  → increment patch (or retain an explicitly prepared higher stable version)
  → update five version files; atomically push commit + tag
  → build/sign four platforms from the prepared commit
  → generate update manifests and release notes → publish as Latest
```

`prepare-auto-release.mjs` reserves existing stable tags and never overwrites them. The bot's version push uses `GITHUB_TOKEN`, so it does not start another CI/release loop. A racing main push causes the old preparation to skip; the newer CI run owns publication. Pure documentation/instruction edits do not create versions. Failed CI does not publish; a failed build leaves the previous Latest intact. Investigate failed runs before any authorized rerun; never move the reserved tag.

For automation changes run `node --test .github/scripts/*.test.mjs` and actionlint, then verify the actual main CI → release run. Normal automatic releases run the full CI checks before preparing the version; no manual version bump or tag push is needed.

## Prepare and validate manual releases

1. Inspect `git status`, fetch `origin/main`, and preserve unrelated user changes. Confirm the release commit is based on the current remote head.
2. Choose stable or beta numbering explicitly. Update the desktop version in both manifests and lockfiles; do not change the independent `cursor-server` version merely to release the desktop app.
3. Confirm the updater public key in `tauri.conf.json` matches `.tauri/cursor-byok.key.pub` without exposing the private key.
4. Confirm `TAURI_SIGNING_PRIVATE_KEY` exists in GitHub Actions. `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` must be absent when the local key has no password.
5. Confirm neither the intended tag nor Release already exists.
6. From `apps/desktop`, run:

   ```bash
   npm run check
   npm run tauri:build -- --debug --no-bundle
   ```

7. Validate the workflow YAML and inspect the staged diff. Ensure `.tauri/`, unrelated local files, and unrelated user changes are not staged.
8. Use the `tauri-action@v1` input `uploadUpdaterJson: true`; `includeUpdaterJson` is not a valid v1 input.

## Publish and verify

After the author explicitly authorizes publication:

1. For normal stable releases, commit only reviewed changes and push or merge to `main`; follow its CI and the automatic release run. Do not race the automation with a manual version/tag.
2. For an explicitly requested manual release, atomically push the prepared version commit to `main` and its matching tag (for example `v0.1.0-beta.1`). The tag push triggers publication; subsequent CI skips the already tagged state.
3. Follow the triggered `Release desktop app` run through completion. Report the run URL and stop on failure; diagnose locally before asking the author to authorize another live attempt.
4. Verify `v<version>` exists, is published rather than draft, has `prerelease: false`, and is the repository's Latest release.
5. Verify the Release contains signed Tauri updater artifacts plus `latest.json`, the signed Windows portable archive plus `portable-latest.json`, and the legacy platform archives plus `update.json`. Confirm manifest URLs target the selected repository and signatures match the app's existing updater public key.
6. For a beta, report clearly that it is a test version even though GitHub represents it as a normal Latest Release.
