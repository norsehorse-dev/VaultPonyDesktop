# Releasing VaultPony Desktop

Each release ships eight artifacts: a signed and notarized **`.dmg`** (macOS, **universal**), an
**`.msi`** (Windows, x64), and six for Linux: a **`.deb`**, a portable **`.tar.gz`** and an
**`.AppImage`**, each for both **x86_64** and **ARM64** - plus a detached `.asc` for every one of
them and a signed `SHA256SUMS` covering the lot. Nine signatures.

The work is split because the signing credentials must never reach a hosted runner:

| Where | What |
| --- | --- |
| **CI**, on a tag push | the six Linux artifacts + the `.msi`, into a **draft** release |
| **Your Mac** | the `.dmg`, notarization, every PGP signature, and publishing the draft |

**This repository holds no secrets.** The Developer ID certificate never reaches a hosted runner,
and neither does the PGP release key. A compromised Actions run cannot sign anything.

The release shape is AgePonyDesktop's, deliberately: same trigger, same draft flow, same naming, same
signing scheme, universal macOS build, `ubuntu-22.04` for glibc 2.35, tests before artifacts, install
to `/usr/bin`. Two things are VaultPony-specific: the self-test creates and reads a throwaway
container rather than checking a crypto vector, and there is no recipient store to open.

## 0. First release only - prerequisites the repository does not have yet

**VaultPonyCore must be reachable from a fresh clone, or CI cannot build.** Locally the core resolves
through a sibling checkout; on a hosted runner it does not exist unless the repository carries it. The
workflows check out with `submodules: recursive`, so the intended setup is a committed submodule:

```sh
git submodule add https://github.com/norsehorse-dev/VaultPonyCore VaultPonyCore
git commit -m "Add VaultPonyCore as a submodule"
```

If you would rather vendor the core (commit its files directly instead of a submodule), that also
works - `submodules: recursive` is then a no-op - but the `[patch.crates-io] fatfs` path and the
`../VaultPonyCore/...` dependency paths must resolve from the repository root exactly as they do now.
Either way, `git clone --recursive <repo>` on a clean machine must produce a tree that `cargo build`
completes, because that is what the runner does.

Create the repository and push `main` before the first tag:

```sh
git init
git add -A
git commit -m "VaultPony Desktop 1.0.0"
gh repo create norsehorse-dev/VaultPonyDesktop --public --source=. --push
```

**Check `.gitignore` first.** It covers `/target` and the local `.cargo/config.toml` dev override.
The Apple notarization password lives in `~/.pgpony-release-env` **outside** the repository; never
commit it.

Push `main` before the first tag. The release workflow triggers only on `v*`, so pushing `main`
gives you a free syntax check of `build.yml` in the Actions tab. **The release workflow cannot be
dry-run.**

## 1. Before tagging

**One number.** The workspace `version` in `Cargo.toml` is the single source: it feeds the deb, the
MSI, the Info.plist, and the version shown in the app's About screen. The shared VaultPonyCore is
versioned independently and is reported separately (the `(core ...)` half of the version line), so
you bump only this one number for a desktop release.

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
cargo run --release --bin vaultpony -- selftest
git status --short
git push
```

A tag on a tree with uncommitted work produces a release that does not contain it. `selftest` must
print `PASS - all 5 checks`.

## 2. Tag - CI builds the seven

```sh
git tag v1.0.0
git push origin v1.0.0
sleep 15
gh run watch $(gh run list --workflow=release.yml --limit 1 \
  --json databaseId --jq '.[0].databaseId')
```

The `sleep` matters: `gh run list` fires before the new run registers and otherwise hands you the
*previous* run's id.

Three build jobs - `linux (x86_64)`, `linux (aarch64)` and `windows`. The two Linux legs are one
matrix job deliberately: an ARM lane maintained as a copy of the x86_64 lane drifts, and a lane that
drifts is a lane whose assertions quietly stop covering it.

CI runs checks a green build would otherwise hide. Read the log rather than the checkmark the first
time:

- **the deb's identity** - `Package` must be `vaultpony`, lowercase, `Architecture` must match the
  lane, and `Depends` must be non-empty and mention `libc6`. An empty `Depends` means `$auto` did not
  scan the ELF, and the deb installs onto a machine with no GL and fails at launch.
- **the desktop entry** - passes `desktop-file-validate`, has a real `Categories`, and carries **no**
  `MimeType`. The last one is a tripwire: VaultPony registers no handler for an opened file, so if
  associations are added later this fails and forces the `.desktop` and the routing code to land
  together.
- **`selftest` passes in the shipped binary**, on all three platforms and both architectures. It
  creates a throwaway container, unlocks it read-write, round-trips a file through the FAT bridge, and
  deletes it - the crypto and filesystem path in that exact artifact, which a test suite running only
  on the build machine cannot see.
- **on Windows, both binaries exist, `vaultpony-cli.exe` prints, and `vaultpony.exe` is
  GUI-subsystem.** The last one is read straight out of the PE header: if `windows_subsystem =
  "windows"` were dropped, the GUI would flash a console window on every launch and nothing else would
  notice.

## 3. On the Mac - the dmg

The four values live in a file outside the repo, `chmod 600`, shared with the other pony desktops
because the Apple credentials are account-level.

```sh
source ~/.pgpony-release-env      # MACOS_SIGN_IDENTITY, NOTARIZATION_{APPLE_ID,PASSWORD,TEAM_ID}
bash tools/make-dmg.sh
```

The script builds both architectures, `lipo`s them together, writes and **reads back** the
`Info.plist`, signs with the hardened runtime, notarizes, staples, and then verifies the app inside
the mounted image. Expect several minutes of apparent inactivity at the notarization step - that is
Apple's service, not a hang.

Gotchas, all inherited from AgePony and all still current:

- `MACOS_SIGN_IDENTITY` is the certificate **name**, not its SHA-1 hash.
- Only **one** `Developer ID Application` certificate may be in the keychain, or codesign reports
  "multiple matching certificates". List with `security find-identity -v -p codesigning`, delete
  keyless extras with `security delete-certificate -Z <hash>`.
- **The dmg container is deliberately unsigned.** The `.app` is signed and notarized and the ticket
  is stapled to the image. `spctl -a -t open --context context:primary-signature` therefore
  **rejects a perfectly good release**. `make-dmg.sh` runs the checks that mean something -
  `spctl -a -t exec` on the app inside the mounted image, and then actually running it.

Confirm `lipo -info` reported **both** `x86_64` and `arm64`, and that `selftest` printed
`PASS - all 5 checks`.

## 4. Assemble, sign, publish

Pull CI's seven down beside your dmg, then sign all eight together so `SHA256SUMS` covers the exact
bytes that ship.

```sh
mkdir -p ~/vaultpony-release && cd ~/vaultpony-release
gh release download v1.0.0 --repo norsehorse-dev/VaultPonyDesktop --dir .
cp /Users/kevinstewart/Apps/VaultPonyDesktop/dist/VaultPony-macOS.dmg .

FILES=(
  VaultPony-macOS.dmg
  VaultPony-linux-x86_64.deb
  VaultPony-linux-x86_64.tar.gz
  VaultPony-x86_64.AppImage
  VaultPony-linux-arm64.deb
  VaultPony-linux-aarch64.tar.gz
  VaultPony-aarch64.AppImage
  VaultPony-windows.msi
)

shasum -a 256 $FILES > SHA256SUMS
cat SHA256SUMS
for f in $FILES SHA256SUMS; do
  gpg -u A0CBC8F65AACE56F1C5B767753F9798E4919DE62 --armor --detach-sign "$f"
done
for f in $FILES SHA256SUMS; do
  gpg --verify "$f.asc" "$f"
done
shasum -a 256 -c SHA256SUMS
```

The same NorseHorse release key signs the rest of the family. One key to publish, one fingerprint for
users to learn.

An **array**, not a space-separated string. This shell is zsh, and zsh does not word-split an
unquoted `$var` the way bash does - `FILES="a b c"` followed by `shasum $FILES` passes one filename
made of all three joined, and every command in the block then fails as if the files are missing.

The explicit `-u` is not decoration either: a bare `gpg --detach-sign` fails with "no default secret
key" on a keyring with more than one. Count **nine** `Good signature` lines before going any further.

Verify before publishing, not after. A signature that does not check out is worse than none.

```sh
gh release upload v1.0.0 VaultPony-macOS.dmg *.asc SHA256SUMS \
  --repo norsehorse-dev/VaultPonyDesktop
gh release edit v1.0.0 --draft=false --latest --repo norsehorse-dev/VaultPonyDesktop
```

`--latest` is load-bearing. `releases/latest/download/...` - the stable, versionless URLs used in a
README or an install script - resolve to whichever release carries the "latest" flag, not to the
newest tag. Published without it, those links point at the previous version silently, or on a first
release at nothing.

## 5. Verification checklist

Artifacts:

- [ ] `gpg --verify` each `.asc`, and `SHA256SUMS.asc` against `SHA256SUMS` - nine in total
- [ ] `shasum -a 256 -c SHA256SUMS` passes (macOS has no `sha256sum`; the format is identical, so
      Linux users can verify later with `sha256sum -c`)
- [ ] the tarball extracts and `VaultPony/vaultpony version` prints, once by hand on a real ARM machine
- [ ] the AppImage is executable and runs, and double-clicking it opens the GUI on a desktop with FUSE
- [ ] `selftest` reports `PASS - all 5 checks` from the **installed** artifact on every OS
- [ ] `selftest`'s version line reports the same rustc on all three platforms. The dmg is the artifact
      at risk, because it is the only one CI does not build

macOS:

- [ ] `lipo -info` on the installed binary reports both architectures
- [ ] `xcrun stapler validate` on the dmg
- [ ] `spctl -a -t exec -vv` on the app inside the mounted dmg says `Notarized Developer ID`
- [ ] a quarantined dmg opens with no dialog, and the app launches from `/Applications` without a prompt
- [ ] the Dock icon looks right at Retina size - this is the first universal build and the only
      artifact CI never exercises
- [ ] test on an **Intel** Mac if one is reachable

Windows:

- [ ] the MSI installs on a clean machine and the Start Menu entry appears
- [ ] `vaultpony.exe` opens the GUI with **no console window flashing behind it**
- [ ] `vaultpony-cli.exe selftest` prints and exits 0
- [ ] SmartScreen behaviour recorded for the release notes - the MSI is unsigned

Every OS:

- [ ] create a container through the GUI, add a file, lock, reopen, and read it back
- [ ] a container created here opens in desktop VeraCrypt with the same password, and one made by
      VeraCrypt opens here
- [ ] the GUI opens in both light and dark mode, in more than one language, and nothing is clipped

## 6. Not set up yet - known gaps

Listed so they are decisions rather than oversights:

- **The site.** `vaultpony.app` has no `/desktop` download page. Until it does, the GitHub release is
  the only download channel.
- **winget / AUR.** No manifest and no `vaultpony-bin` PKGBUILD. An AUR PKGBUILD must be pushed
  **after** the release is published, or it points users at a draft and 404s. For winget, note that
  `ProductCode` is **not** the `UpgradeCode` in `wix/main.wxs` - different GUIDs.
- **The MSI is the least-verified artifact.** `wix/main.wxs` was generated by `cargo wix init` and
  hand-edited for the product name, description, icon and the second binary. CI installs the MSI and
  runs the binary out of it, so a broken source fails the tag rather than a user - but expect the
  first tag to be where any WiX issue surfaces.
- **Mounting is not in the shipped build.** The `fuse` feature is off by default and never bundled, so
  the released artifacts browse containers in-app rather than mounting them as drives. A build with
  `--features fuse` (and the FUSE runtime installed) is a local, opt-in thing for now.
