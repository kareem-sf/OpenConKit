# Releasing OpenConKit

Releases are built from validated SemVer tags by
`.github/workflows/release.yml`. Native packages are produced only on their
matching operating-system runners. The workflow keeps the GitHub Release as a
draft until the Windows, Linux, macOS, portable, updater-signature, and merged
feed checks all pass.

## Version and channels

`VERSION` is canonical. Run `pnpm version:sync` after changing it and
`pnpm version:check` before committing.

- Stable tag: `v0.0.1`
- Beta tag: `v0.0.2-beta.1`
- Hotfix: increment the patch version normally

Stable releases update `latest-stable.json` and `latest-beta.json`. A
prerelease updates only `latest-beta.json`; it can never replace the stable
feed. The two files live on the `updates` branch and are served from the
compiled-in raw GitHub URLs.

## Signing model

Tauri updater signatures and operating-system publisher signatures solve
different problems.

- The updater key is mandatory. Its public key is embedded in
  `tauri.conf.json`; its private key and password are stored as
  `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` GitHub Actions secrets.
- The restricted local recovery copy is outside the repository and outside
  app home. Never print it, copy it into `.env`, or attach it to a release.
- v0.0.1 does not have paid Windows publisher signing or Apple Developer ID
  notarization. Windows SmartScreen and macOS Gatekeeper can therefore show
  an unknown-publisher warning. macOS artifacts are ad-hoc signed. Do not
  describe them as notarized.

If the updater private key is lost, existing installations cannot trust a
replacement key. Preserve the restricted recovery copy. If it is suspected
compromised, stop publishing feeds and document a manual reinstall/key
rotation; never silently replace the public key.

## Release preparation

1. Update `CHANGELOG.md` with user-visible changes and known limitations.
2. Edit `VERSION`, run `pnpm version:sync`, and review every propagated file.
3. Fetch the current host sidecar once with `pnpm codex:fetch`; do not commit
   the staged executable.
4. Run:

   ```sh
   pnpm install --frozen-lockfile
   pnpm format:check
   pnpm lint
   pnpm typecheck
   pnpm test
   pnpm test:e2e
   pnpm build
   pnpm contracts:check
   pnpm tool:completeness
   pnpm security:secrets
   pnpm licenses:check
   pnpm notices:check
   pnpm audit --audit-level moderate
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
   cargo test --workspace --all-features --locked
   cargo deny check --all-features
   ```

5. Commit with `chore(release): vX.Y.Z`, push `main`, and wait for branch CI.
6. Create and push the exact annotated tag only after CI is green:

   ```sh
   git tag -a vX.Y.Z -m "OpenConKit vX.Y.Z"
   git push origin vX.Y.Z
   ```

## Native artifacts

The release matrix produces these user-facing files (plus updater archives
and signatures):

- `OpenConKit_<version>_windows_x64_setup.exe`
- `OpenConKit_<version>_windows_x64_portable.zip`
- `OpenConKit_<version>_linux_x64.AppImage`
- `OpenConKit_<version>_linux_x64.deb`
- `OpenConKit_<version>_macos_universal.dmg`
- `OpenConKit_<version>_macos_universal.zip`
- Tauri updater archives and `.sig` files.

The Codex fetcher verifies pinned official archives for every target. On
macOS it uses `lipo` to assemble a universal app-server, confirms both
`x86_64` and `arm64` slices, and verifies the resulting binary version on the
native runner. After bundling, CI verifies every executable in the `.app`
contains both architectures and passes strict ad-hoc `codesign` verification
before creating the user-facing ZIP.

The portable ZIP includes the desktop executable, renamed Codex sidecar,
protocol schema, all license/notice files, a bilingual README, and the
`OPENCONKIT_PORTABLE` marker. Its data still lives under
`%USERPROFILE%\.openconkit`; it is not portable-data mode. The portable
package requires Microsoft Edge WebView2 Runtime and cannot update itself in
place. Windows smoke tests must launch both the extracted portable executable
and an installed copy, verify WebView2 creates data only under
`%USERPROFILE%\.openconkit\cache\webview`, and fail if either executable
directory gains a sibling `*.WebView2` profile. The release workflow enforces
this with `pnpm package:smoke:windows` before uploading the portable archive.

## Feed publication and failure behavior

Each native build uploads its artifacts to one draft release. These jobs are
serialized because Tauri Action merges platform entries by replacing
`latest.json`; serialization prevents concurrent updates from dropping a
platform. The final job:

1. downloads every draft release asset;
2. requires the six exact, non-empty user-facing distribution files;
3. requires Windows NSIS, Linux AppImage, and both macOS architecture keys;
4. accepts only project-owned release-asset URLs;
5. adds authoritative asset sizes and bounds notes/signatures;
6. publishes the draft release; and
7. advances the appropriate channel file through the GitHub Contents API.

If any required platform or signature is missing, the workflow fails and the
release stays draft. If feed publication fails after release publication,
the existing feed remains unchanged; users can still download the release
manually. Repair the workflow and rerun it—do not hand-edit signature data.

## Rollback

1. Stop the affected channel by restoring its previous known-good feed file
   on the `updates` branch.
2. Mark the faulty GitHub release appropriately and publish a normal SemVer
   hotfix.
3. Do not delete or rewrite database migrations and do not make an older app
   auto-downgrade app-home data.
4. Users may reinstall a previous package manually only if that version
   supports their current database schema.
