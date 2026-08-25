# VaultPonyDesktop plan

Goal: a desktop app for opening and editing VeraCrypt file containers, on Linux,
macOS, and Windows, built on the shared Rust core (`VaultPonyCore`) that already
backs VaultPonyAndroid. Pure Rust, `egui`/`eframe`, no UniFFI. The desktop app
links `vault-core` directly and calls its Rust API.

This plan is grounded in a read of all four trees as they stand: `VaultPonyCore`
(the `vault-core` session API, the `Vfs` trait, and the `vaultpony` CLI),
`VaultPonyAndroid` (the shipped feature set the desktop mirrors), and
`AgePonyDesktop` (the proven desktop template: workspace, `panels/` pattern,
`theme.rs`, `mark.rs`, `tasks.rs`, dual binary, packaging).

Two decisions shape everything below, both made up front:

1. Real OS-level mounting is in scope, not just an in-app browser. The container
   opens as a real drive the user's other apps can see.
2. The desktop CLI ships as a second binary alongside the GUI, the AgePony way.

Last updated: 2026-08-25.

---

## 1. What the core already gives us

`vault-core` is done and does the hard part. The desktop app is a shell over it,
exactly as VaultPonyAndroid is a shell over the same crate through UniFFI. The
API the desktop consumes:

| Need | Core call |
|---|---|
| Inspect a container without opening it | `probe` -> `VolumeInfo` (support-safe: parameters only, no names or paths) |
| Unlock (read-only or read-write) | `Session::unlock`, `Session::unlock_with` |
| Unlock the outer volume with hidden protection | `Session::unlock_outer_protected` |
| Create a container | `create_container`, `create_container_with_hidden` |
| Browse and edit files inside | `Session::vfs` -> `Vfs` trait (list, read, write, mkdir) |
| Change password / PIM | `change_password` |
| Header backup and restore | `export_header_backup`, `restore_header_from_file`, `restore_header_from_embedded` |
| Lock and zeroize | `Session::lock` |
| Filesystem choice for new volumes | `ContainerFs::{Fat, Exfat}` |

Two levels are exposed, and both matter for the mount work in section 4:

- A **decrypted block device** (`DecryptedDevice`): the raw, decrypted filesystem
  bytes (FAT / exFAT / NTFS), the same surface VeraCrypt's own driver hands to the
  OS.
- A **file-level VFS** (`Vfs`): an in-process filesystem adapter (fatfs read-write,
  `norse-exfat`, `ntfs` read-only) that lists, reads, and writes files without any
  OS involvement.

The desktop app is the first shell that can use the block-device level, because it
is the first shell that can talk to an OS mount layer. Android and iOS cannot mount
a system drive, so they only ever use the VFS. That is the one genuinely new
capability the desktop adds over the phone.

---

## 2. Invariants we inherit (non-negotiable)

`VaultPonyCore/THREAT_MODEL.md` is a living document that CI enforces, and it wins
over any feature. Everything below obeys it:

- **Zero network.** No networking crate in the dependency tree, enforced by
  `cargo-deny`. This has a direct consequence for the mount design in section 4:
  it rules out NBD-style loopback block devices, whose whole surface reads as
  network code even on localhost.
- **Secret hygiene.** Keys and passphrases are zeroized on drop; locking a volume
  zeroizes through one code path (`Session::lock`). The desktop must route auto-lock,
  explicit lock, and unmount-on-idle through that same path, with nothing lingering
  in UI buffers.
- **No secret, filename, or container path in logs, errors, or any persisted
  surface.** The core's error variants are built to be user-explainable without
  embedding user data. The desktop keeps that: no container path in a crash log, no
  volume name in a recents list unless the user opts in.
- **`panic = "abort"`, no `unwrap` on attacker-controlled input.** A malicious
  container must degrade to a clean error, never a crash-as-exploit. New code that
  reads untrusted bytes gets the same property-test-and-fuzz treatment the core
  parsers already have.
- **The core never imports the UI.** `vault-core` stays a clean library. Nothing
  desktop-specific leaks back into it. If the mount work needs something from the
  core (for example, a stable block-device handle it can serve sectors from), that
  is a `vault-core` change argued in the core repo, not a desktop hack around the
  API.
- **Hidden-volume deniability.** A hidden volume, once open, must not be labeled
  "hidden" in any persistent or system-visible surface. For the desktop this is a
  live constraint on the mount layer: the mount point name, the drive label, the
  OS recents, and any window title must never reveal that what is mounted is a
  hidden volume, or even that a hidden volume exists.

---

## 3. Architecture and repo layout

A new repo at `~/Apps/VaultPonyDesktop`, its own Cargo workspace, mirroring
AgePonyDesktop's shape but with the core pulled in from outside rather than living
in-tree.

```
VaultPonyDesktop/
  Cargo.toml            workspace: members = ["vaultpony-desktop"]
  Cargo.lock            committed; --locked everywhere
  rust-toolchain.toml   pinned, matching the core's toolchain
  deny.toml             mirrors the core policy, plus vets the mount crates
  VaultPonyCore/        git submodule -> norsehorse-dev/VaultPonyCore
  vaultpony-desktop/
    Cargo.toml          depends on vault-core by path into the submodule
    src/
      main.rs           GUI binary (windows_subsystem = "windows" on release)
      bin/
        vaultpony-cli.rs   console binary for the Windows-no-console case + CI
      lib.rs            shared library the two binaries call
      app.rs            App state, the Tab rail, shortcuts
      tasks.rs          off-thread work (unlock, create, mount) + progress
      theme.rs          port of the AgePony theme
      mark.rs           icon rasterised from vertex data -> window/dock icon
      panels.rs
      panels/
        volumes.rs      unlock, the mount/session table, lock, unmount
        files.rs        in-app browser: navigate, extract, add, mkdir
        create.rs       new-container wizard
        tools.rs        change password, header backup/restore, volume info
        settings.rs     auto-lock, theme, language, defaults
      mount/            the OS-mount layer (section 4), behind a trait
  packaging/            deb, AppImage, wix, macOS .app (ported from AgePony)
  vectors/, fixtures/   interop round-trips against real VeraCrypt
```

**Core dependency.** `VaultPonyCore` comes in as a git submodule, the same way
VaultPonyAndroid resolves it, and `vaultpony-desktop` depends on the member crate
by path:

```
vault-core = { path = "../VaultPonyCore/core/vault-core" }
```

Clones use `--recursive`; CI runs `git submodule update --init --recursive`. For
local two-repo development (editing core and desktop together against a sibling
`~/Apps/VaultPonyCore` checkout), a committed-out `[patch]` stanza or a
`.cargo/config.toml` path override can point at the sibling without touching the
submodule pin. That override stays out of the default build so a fresh clone is
reproducible.

**Dual binary.** `vaultpony` is the GUI. `vaultpony-cli` is a second,
console-subsystem binary over the same library, so verbs print somewhere when the
GUI binary is launched from Explorer on Windows and has no console. It is built on
every platform so it cannot rot, and it is what CI uses to smoke-test a packaged
build without a display. The core already ships its own headless `vaultpony` CLI
(GPL-3.0); the desktop CLI is a thinner thing whose job is the Windows shim, the
CI check, and driving mount / unmount from a script. Verb overlap between the two
is fine: they are different binaries in different repos.

**Threading.** Unlock derives header keys through 500k PBKDF2 iterations; create
formats a filesystem; mount serves sectors on a hot path. None of that runs on the
UI thread. `tasks.rs` owns a worker, the panels post jobs and read back progress
(the core already threads an `UnlockProgress` callback for exactly this), and the
window stays responsive with a real progress bar during a slow unlock.

**License.** Apache-2.0, matching VaultPonyAndroid and the GUI's only real
dependency, `vault-core`. The mount runtimes complicate this and are called out in
section 4 and section 9.

---

## 4. The mount architecture (the centerpiece)

This is the one large piece of new engineering, and the one with real security
weight, so it gets argued in full before any code.

### 4.1 Two ways to mount, and why we pick one

**Block-level (what VeraCrypt itself does).** Expose the `DecryptedDevice` to the
kernel as a virtual disk and let the OS's own FAT / exFAT / NTFS driver mount it.
Byte-perfect filesystem semantics, full NTFS, best performance. The cost is a
kernel-level virtual block device on every OS:

- Linux: NBD or a loop device over a served image. NBD is a network protocol, which
  collides head-on with the zero-network invariant even on loopback, and needs root
  to `mount`.
- macOS: no supported userspace block device since Apple locked down kexts. A
  signed kext is effectively off the table for a small app.
- Windows: a signed virtual-disk driver (EV certificate, WHQL). A distribution and
  signing burden out of proportion to this project.

Block-level is tractable only on Linux, and even there it drags in the thing the
threat model forbids. It is not the path.

**File-level (FUSE family over the core's `Vfs`).** Present the `Vfs` as a
userspace filesystem. This reuses the core directly: a FUSE read is a `Vfs` read, a
FUSE write is a `Vfs` write. It is portable across all three platforms, needs no
driver signing from us, and touches no network:

- Linux: FUSE via the `fuser` crate (libfuse). Userspace, no root when the user is
  set up for FUSE.
- macOS: macFUSE via `fuser`, or Apple's FSKit on macOS 15+ (userspace, no kext).
- Windows: WinFsp (the modern, maintained choice) or Dokan, via their Rust
  bindings.

We build the file-level path. The mount layer sits behind one internal trait so the
three backends (`fuser`, macFUSE/FSKit, WinFsp) implement the same contract, and the
GUI never sees the difference. The trait is a thin translator: `Vfs` operations in,
platform filesystem callbacks out.

### 4.2 The in-app browser is the same bridge, so it comes first

The in-app file browser (panel `files.rs`) and the FUSE mount are two surfaces over
the identical `Vfs` consumer. Building the browser first is not throwaway work: it
proves the `Vfs`-to-file-operation translation (list, read a file out, write a file
in, make a directory, honor the write limits) with zero external dependencies and
no kernel involvement, entirely inside `cargo test`. The mount layer then wraps that
same translation in the platform's filesystem callbacks. So the sequencing is
browser first, mount second, and the mount inherits a tested core translation rather
than debugging it through a kernel boundary.

It also means the browser stays useful on its own: it needs no macFUSE or WinFsp
install, and it covers the common "just pull one file out" case without mounting
anything system-wide.

### 4.3 What the VFS can and cannot do, and what that means for mounting

The mount is only ever as capable as the underlying `Vfs` adapter:

- FAT: read-write.
- exFAT (`norse-exfat`): write support lands later in the core (the CLI marks exFAT
  write as a later phase). Until it does, an exFAT volume mounts read-only.
- NTFS: read-only in the core today.

The mount layer reports the real capability up to the OS: a read-only adapter mounts
read-only, and a write into it returns `EROFS` rather than failing halfway. The GUI
shows the mount's mode plainly so the user is never surprised that a mounted NTFS
container will not take a new file.

### 4.4 Security consequences of mounting (threat-model delta)

Mounting is a real widening of exposure and the threat model must be extended to say
so, in the same PR that adds it:

- **Plaintext leaves the process.** In the in-app model, decrypted content never
  leaves VaultPony's address space. A mounted volume is visible to every process
  running as that user, and to anything that indexes or backs up the mount point.
  This is the point of mounting, and it is a genuine trade the user is opting into.
  The GUI states it at mount time, and read-only is the default mount mode so the
  safer choice is the default.
- **Auto-lock must unmount.** Idle timeout, screen lock, and explicit lock all
  unmount first, then drop the `Session` through `Session::lock` to zeroize. If the
  unmount is blocked because another app holds a file open, the policy is: attempt a
  clean unmount, warn, and offer a forced unmount that still zeroizes keys (the OS
  keeps a stale handle, but no key material survives). That policy is written into
  the threat model, not left implicit.
- **Hidden-volume protection through the mount.** Mounting the outer volume
  read-write uses `unlock_outer_protected`, so a write landing in the hidden
  region is refused and latches the volume read-only, exactly as on the phone. The
  FUSE write path surfaces `HiddenVolumeProtected` as `EROFS` to the OS, and the GUI
  explains the stop (that message only ever reaches someone who supplied the hidden
  password, so it leaks nothing).
- **The mount point is metadata.** The mount name, the drive label, and the OS
  recents must carry nothing about the container: not its filename, and above all
  not the fact that a hidden volume is what is mounted. A neutral, fixed mount name
  (or a user-set label that defaults to neutral), never the container's own name.
  Linux mounts under a private directory, macOS under `/Volumes` with a neutral
  label, Windows to a drive letter with no revealing volume label.

### 4.5 The mount runtime is a distribution dependency

macFUSE and WinFsp are not ours to bundle silently, and they carry their own
licenses (WinFsp is GPLv3-or-commercial; recent macFUSE is no longer free software).
This is an open decision, tracked in section 10:

- On macOS, prefer FSKit on macOS 15+ (no third-party runtime, no kext) and treat
  macFUSE as the fallback for older systems.
- On Windows, weigh WinFsp against Dokan (more permissive licensing) before
  committing.
- Whatever the choice, the app detects whether the runtime is present, guides the
  user to install it if not, and falls back to the in-app browser so the app is
  never dead in the water without it.

---

## 5. Feature phases

Sequenced so each phase ships something usable and each builds on the last. Android
parity is the target for everything except the mount, which is desktop-only.

**P0. Scaffold and walking skeleton.** Workspace, submodule, pinned toolchain,
`deny.toml`, the no-network test, git identity set to the dev handle, CI. `theme.rs`
and `mark.rs` ported. A window opens with the Tab rail, and one real path works end
to end: pick a container, enter a password, `Session::unlock`, list the root
directory. This proves core-to-`egui` through the submodule.

**P1. In-app file browser.** The `Vfs` bridge as a panel: navigate directories,
extract a file or a tree to disk, add a file into a FAT volume, make a directory.
Tested entirely in-process. This is the translation the mount reuses (section 4.2).

**P2. Create-container wizard.** Size, cipher (including cascades), PRF/hash,
filesystem (FAT / exFAT), PIM, keyfiles. `create_container`. Round-trips against
real VeraCrypt from day one.

**P3. Hidden volumes.** Create with a hidden volume (`create_container_with_hidden`),
open a hidden volume by password alone (the password selects the slot; there is no
"open hidden" toggle to leak), and outer-volume writes under hidden protection
(`unlock_outer_protected`).

**P4. Header tools and volume info.** Header backup, restore from an external file,
restore from the embedded backup, and change password / PIM. A support-safe volume
info panel over `probe` (parameters only, never names or paths).

**P5. OS mount.** The FUSE-family layer of section 4, behind the mount trait.
Read-only first, then read-write where the adapter allows, auto-unmount wired into
auto-lock, hidden-volume protection through the mount, mount-point metadata hygiene,
and runtime detection with a browser fallback. Linux (`fuser`) leads, then Windows,
then macOS.

**P6. Lock and privacy.** Idle and screen-lock auto-lock (unmount then zeroize),
OS-native unlock convenience (Touch ID on macOS, Windows Hello) as an option, a
hidden-screen / obscure-on-unfocus behavior, and a no-trace mode that writes no
recents and no remembered parameters. Settings persistence.

**P7. Localization.** English, German, Spanish, French, Russian, and Brazilian
Portuguese, live in-app switching, matching the Android set. A strings module the
panels read through.

**P8. Packaging and release.** deb, AppImage, macOS `.app` / dmg, Windows msi,
ported from AgePony's packaging with the mount runtime handled per platform. Signing
and notarization. The dual-binary CLI surfaces the scriptable verbs (including
mount / unmount).

Rough dependency order: P0 -> P1 -> {P2, P4} -> P3 -> P5 -> P6 -> P7 -> P8. P2 and
P4 are independent of each other and can interleave.

---

## 6. Testing

- **Interop is inherited, and re-proven at the app layer.** The core already
  guarantees byte-faithful VeraCrypt compatibility from a pinned VeraCrypt source
  checkout, fixtures as the spec. The desktop adds round-trips at its own layer:
  create a container in VaultPonyDesktop and mount it in real VeraCrypt, and open a
  real VeraCrypt container (including a hidden volume) in VaultPonyDesktop, both
  directions in `vectors/`.
- **The `Vfs` bridge is tested in-process.** Every browser operation (P1) has a test
  that runs without a display and without a kernel mount, so the translation the
  mount depends on is green before the mount exists.
- **The mount layer gets an integration test per platform** in CI: mount a fixture
  read-only, stat and read a known file, unmount, and assert a write to a read-only
  mount returns `EROFS`. Hidden protection through the mount is tested the way the
  core tests it (`protect_gate`): the overrunning write is blocked and the hidden
  tree survives byte-identical.
- **Untrusted-input code keeps the core's discipline.** Anything new that parses
  attacker-controlled bytes gets a property test and a fuzz target, and `panic =
  "abort"` stands.
- **`cargo-deny` and the no-network test run in CI** and fail the build on a network
  crate or a policy violation, mount crates included.

---

## 7. Desktop equivalents of the Android privacy features

The Android app ships biometric unlock, auto-lock on background, a hidden screen,
and a no-trace mode. The desktop maps them rather than porting them literally:

- Biometric unlock -> OS-native unlock (Touch ID / Windows Hello) as an optional
  convenience over the passphrase, never a replacement for it.
- Auto-lock on background -> idle timeout and screen-lock, which on the desktop must
  also unmount (section 4.4).
- Hidden screen -> obscure the window contents on unfocus, and keep the mount name
  and any title neutral so a screenshot of the desktop or the drive list reveals
  nothing.
- No-trace mode -> no recents, no remembered PIM or parameters, no container path
  written anywhere.

---

## 8. Packaging notes carried from AgePony

AgePony's packaging is a working reference for the parts that are the same:
lowercase package name in the deb (a PascalCase `Package` field is uninstallable),
`depends = "$auto"` so the real X11 / Wayland / GL shared-library deps are derived
from the built ELF, the wix GUIDs committed once and never regenerated for the
upgrade code, and the icon rendered from the same vertex data the UI draws so it
cannot drift. The new work over AgePony is entirely the mount runtime per platform,
covered in section 4.5 and section 10.

---

## 9. Licensing to confirm

The GUI is Apache-2.0 and its only substantive dependency, `vault-core`, is
Apache-2.0. The mount runtimes are the open question: WinFsp is GPLv3-or-commercial
and recent macFUSE is not free software, and linking or depending on them at runtime
has to be squared with the app's license before P5 ships. FSKit on macOS and Dokan
on Windows are the more permissive routes and are the reason section 4.5 leans that
way. This gets settled, in writing, before the mount code lands.

---

## 10. Open questions

1. **macOS mount backend:** FSKit (macOS 15+, no runtime) with a macFUSE fallback,
   or macFUSE across the board for a wider macOS range? The FSKit route is cleaner
   and license-free but drops older macOS.
2. **Windows mount backend:** WinFsp or Dokan? Licensing (section 9) versus maturity
   and API fit.
3. **Read-write default:** should a mount default to read-only always, with
   read-write an explicit per-mount choice? Leaning yes, on safety grounds.
4. **Forced-unmount policy** when the OS reports the mount busy: how aggressive, and
   what exactly the user sees. The key-zeroize half is not negotiable; the handle
   half is.
5. **Local two-repo dev ergonomics:** `[patch]` versus `.cargo/config.toml` path
   override for building against a sibling `~/Apps/VaultPonyCore` without disturbing
   the submodule pin.
6. **Publish target:** presumably `github.com/norsehorse-dev/VaultPonyDesktop`,
   public, dev-handle identity. Confirm before the first push.
