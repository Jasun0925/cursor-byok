---
name: release
description: Prepare, authorize, publish, troubleshoot, and verify Cursor BYOK desktop GitHub Releases. Use for version bumps, release tags, GitHub Actions release runs, updater manifests, signing, or release-readiness checks.
---

# Desktop release

Release through `.github/workflows/release.yml`. Preserve the updater manifests used by the current workflow: Tauri uses `latest.json`, Windows portable updates use `portable-latest.json`, and legacy `v0.0.49` clients use `update.json`.

## Publication authority

- Release only to the user's explicitly selected repository. This fork uses `origin = haohaomin/cursor-byok`; `upstream = leookun/cursor-byok` is a source of code, not a publication destination.
- Before publication, require an explicit release instruction in the current task and verify the selected GitHub repository, push permission, and actor. The current workflow requires `gh api user --jq .login` to match that repository's owner (`haohaomin` for this fork). Honor authorization already supplied for the current task; do not require the upstream author's identity to publish the user's fork.
- Treat all of these as publication actions: pushing a `v*` tag, rerunning the release workflow, and publishing or editing a GitHub Release. Pushing a release commit to `main` only prepares the release and must never trigger publication by itself.
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
- Keep ordinary `main` pushes and manual workflow dispatch disabled as release triggers. The author pushes the matching tag only after the release commit is present on `origin/main`.
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

## Prepare and validate

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

1. Commit only the reviewed release set and push it to `main`. Confirm the release commit is present in `origin/main`; this push must not start the release workflow.
2. Create the matching tag on that commit, for example `v0.1.0-beta.1`, and push only that tag. This tag push is the publication trigger.
3. Follow the triggered `Release desktop app` run through completion. Report the run URL and stop on failure; diagnose locally before asking the author to authorize another live attempt.
4. Verify `v<version>` exists, is published rather than draft, has `prerelease: false`, and is the repository's Latest release.
5. Verify the Release contains signed Tauri updater artifacts plus `latest.json`, the signed Windows portable archive plus `portable-latest.json`, and the legacy platform archives plus `update.json`. Confirm manifest URLs target the selected repository and signatures match the app's existing updater public key.
6. For a beta, report clearly that it is a test version even though GitHub represents it as a normal Latest Release.
