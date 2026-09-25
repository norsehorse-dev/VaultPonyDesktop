//! Header tools and volume info (P4), UI-free so they are tested headlessly.
//!
//! Each of these operates on the container file directly, opening its own block
//! device; none needs an unlocked session. They derive header keys, so the
//! caller runs them on a worker thread. The output is always a displayable
//! string with no secret and no path in it (volume info is deliberately
//! support-safe: parameters only).

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use vault_core::VolumeInfo;
use vc_io::FileDevice;

/// Probe the container and return a support-safe description (no names, no
/// paths, no key material): the same facts `vaultpony info` prints.
pub fn info(container: &Path, secret: &[u8], pim: u32) -> Result<String> {
    let vi = vault_core::probe(container, secret, pim, &mut |_, _, _| {}).map_err(vc)?;
    Ok(format_info(&vi))
}

fn format_info(vi: &VolumeInfo) -> String {
    format!(
        "cipher:               {}\n\
         hash:                 {}\n\
         header:               {:?}\n\
         header version:       {}\n\
         min program version:  {:#06x}\n\
         volume size:          {} bytes\n\
         sector size:          {}\n\
         data area:            offset {}, {} bytes\n\
         filesystem:           {:?}",
        vi.scheme,
        vi.prf,
        vi.source,
        vi.header_version,
        vi.min_program_version,
        vi.geometry.volume_size,
        vi.geometry.sector_size,
        vi.geometry.encrypted_area_start,
        vi.geometry.encrypted_area_size,
        vi.filesystem,
    )
}

/// Export the container's leading header group (128 KiB of ciphertext) to
/// `out`. Carries no secret the container does not already hold, but it keeps
/// accepting the *current* password even after a password change, so the caller
/// warns the user to store it as carefully as the container itself.
pub fn backup_header(container: &Path, out: &Path) -> Result<()> {
    let mut dev = FileDevice::open_read(container)
        .map_err(vc)
        .context("opening the container to read its header")?;
    let bytes = vault_core::export_header_backup(&mut dev).map_err(vc)?;
    std::fs::write(out, &bytes).with_context(|| format!("writing {}", out.display()))?;
    Ok(())
}

/// Restore the primary header from a previously exported backup file. The
/// backup must unlock with `secret`/`pim` before a byte is written.
pub fn restore_from_file(container: &Path, backup: &Path, secret: &[u8], pim: u32) -> Result<()> {
    let bytes = std::fs::read(backup).with_context(|| format!("reading {}", backup.display()))?;
    let mut dev = FileDevice::open_rw(container)
        .map_err(vc)
        .context("opening the container to write its header")?;
    vault_core::restore_header_from_file(&mut dev, &bytes, secret, pim).map_err(vc)?;
    Ok(())
}

/// Restore the primary header from the container's own embedded backup (its
/// trailing copy), verifying it unlocks with `secret`/`pim` first.
pub fn restore_from_embedded(container: &Path, secret: &[u8], pim: u32) -> Result<()> {
    let mut dev = FileDevice::open_rw(container)
        .map_err(vc)
        .context("opening the container to write its header")?;
    vault_core::restore_header_from_embedded(&mut dev, secret, pim).map_err(vc)?;
    Ok(())
}

/// Change a volume's password and PIM in place. The data is untouched; only the
/// header key protecting the master keys is re-derived. Verify-then-write: the
/// old secret must unlock before anything is written.
///
/// `new_prf` moves the header to another KDF/hash by registry name ("Argon2id",
/// "SHA-512", ...); `None` keeps the current one. `new_pim` is read under the
/// KDF the header ends up with, so 0 means that KDF's default. A PIM below the
/// default with a password under 20 characters is refused, as VeraCrypt does.
pub fn change_password(
    container: &Path,
    old: &[u8],
    old_pim: u32,
    new: &[u8],
    new_pim: u32,
    new_prf: Option<&str>,
) -> Result<()> {
    let prf = match new_prf {
        None => None,
        Some(name) => Some(
            crate::newvolume::prf_by_name(name).ok_or_else(|| anyhow!("unknown hash {name}"))?,
        ),
    };
    let mut dev = FileDevice::open_rw(container)
        .map_err(vc)
        .context("opening the container to re-key it")?;
    vault_core::change_password_to(
        &mut dev,
        &vault_core::UnlockSecret::new(old, old_pim),
        &vault_core::UnlockSecret::new(new, new_pim),
        prf,
        new,
    )
    .map_err(vc)?;
    Ok(())
}

/// Render a `VcError` into an `anyhow::Error` by its message.
fn vc(e: vc_types::VcError) -> anyhow::Error {
    anyhow!(e.to_string())
}
