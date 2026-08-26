//! Creating a new container (P2), with optional hidden volume (P3), UI-free so
//! it is tested headlessly.
//!
//! A thin, ordered wrapper over the core's create calls: size the backing file,
//! build the parameters, hand over a block device, and let the core write the
//! VeraCrypt volume and lay a fresh filesystem inside it. The one expensive step
//! (the header-key derivation at the chosen PRF and PIM) runs here, so the caller
//! drives this from a worker thread, never the UI thread.

use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use vault_core::{create_container, create_container_with_hidden, ContainerFs, CreateParams};
use vc_io::{BlockDevice, FileDevice};
use vc_types::{EncryptionScheme, Prf};
use zeroize::Zeroizing;

/// Sector size we format with. VeraCrypt's default, and the only value the
/// desktop offers for now.
const SECTOR_SIZE: u32 = 512;

/// The hidden volume carved into the tail of a container. It reuses the outer
/// volume's cipher, hash, and filesystem; only its secret, PIM, and size differ.
pub struct HiddenPart {
    pub passphrase: Zeroizing<Vec<u8>>,
    pub pim: u32,
    pub keyfiles: Vec<PathBuf>,
    /// The hidden volume's usable data area, carved from the container tail.
    pub data_size: u64,
}

/// Everything needed to create one container. Owned and `Send`, so it moves
/// straight into the worker thread.
pub struct Spec {
    pub path: PathBuf,
    pub size: u64,
    pub scheme: &'static EncryptionScheme,
    pub prf: &'static Prf,
    pub pim: u32,
    pub fs: ContainerFs,
    pub passphrase: Zeroizing<Vec<u8>>,
    pub keyfiles: Vec<PathBuf>,
    /// When set, a hidden volume is created inside the outer one.
    pub hidden: Option<HiddenPart>,
}

/// Create the container described by `spec`. Overwrites `spec.path` if it
/// exists, so the caller confirms the destination first (the save dialog does).
pub fn create(spec: &Spec) -> Result<()> {
    if spec.passphrase.is_empty() && spec.keyfiles.is_empty() {
        return Err(anyhow!("set a passphrase or add at least one keyfile"));
    }
    match &spec.hidden {
        Some(hidden) => create_with_hidden(spec, hidden),
        None => create_plain(spec),
    }
}

fn create_plain(spec: &Spec) -> Result<()> {
    let keyfiles = read_keyfiles(&spec.keyfiles)?;
    size_file(&spec.path, spec.size)?;

    let params = CreateParams {
        scheme: spec.scheme,
        prf: spec.prf,
        pim: spec.pim,
        passphrase: &spec.passphrase,
        keyfiles: &keyfiles,
        size: spec.size,
        sector_size: SECTOR_SIZE,
    };

    let dev = FileDevice::open_rw(&spec.path).context("opening the new container for writing")?;
    create_container(Box::new(dev), &params, spec.fs).map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}

fn create_with_hidden(spec: &Spec, hidden: &HiddenPart) -> Result<()> {
    if hidden.passphrase.is_empty() && hidden.keyfiles.is_empty() {
        return Err(anyhow!(
            "the hidden volume needs its own passphrase or keyfile"
        ));
    }
    if hidden.passphrase == spec.passphrase && hidden.keyfiles == spec.keyfiles {
        return Err(anyhow!(
            "the hidden volume must use a different secret from the outer volume"
        ));
    }
    if hidden.data_size >= spec.size {
        return Err(anyhow!(
            "the hidden volume must be smaller than the container"
        ));
    }

    let outer_keyfiles = read_keyfiles(&spec.keyfiles)?;
    let hidden_keyfiles = read_keyfiles(&hidden.keyfiles)?;
    size_file(&spec.path, spec.size)?;

    // Outer and hidden share the container size; the hidden data area is carved
    // from the tail (`hidden.data_size`).
    let outer = CreateParams {
        scheme: spec.scheme,
        prf: spec.prf,
        pim: spec.pim,
        passphrase: &spec.passphrase,
        keyfiles: &outer_keyfiles,
        size: spec.size,
        sector_size: SECTOR_SIZE,
    };
    let hidden_params = CreateParams {
        scheme: spec.scheme,
        prf: spec.prf,
        pim: hidden.pim,
        passphrase: &hidden.passphrase,
        keyfiles: &hidden_keyfiles,
        size: spec.size,
        sector_size: SECTOR_SIZE,
    };

    // The core re-opens the backing store three times (both headers, then each
    // filesystem), so it takes a factory that hands back a fresh device.
    let path: &Path = &spec.path;
    let make_dev = move || FileDevice::open_rw(path).map(|d| Box::new(d) as Box<dyn BlockDevice>);

    create_container_with_hidden(make_dev, &outer, &hidden_params, hidden.data_size, spec.fs)
        .map_err(|e| anyhow!(e.to_string()))?;
    Ok(())
}

fn read_keyfiles(paths: &[PathBuf]) -> Result<Vec<Vec<u8>>> {
    paths
        .iter()
        .map(|p| std::fs::read(p).with_context(|| format!("reading keyfile {}", p.display())))
        .collect()
}

/// Create and size the backing file. The core's create calls require it to be
/// exactly `size` bytes.
fn size_file(path: &Path, size: u64) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    file.set_len(size)
        .with_context(|| format!("sizing {} to {} bytes", path.display(), size))?;
    Ok(())
}

/// The cipher names the core knows, in registry order, for the picker.
pub fn scheme_names() -> impl Iterator<Item = &'static str> {
    vc_types::registry::ENCRYPTION_SCHEMES
        .iter()
        .map(|s| s.name)
}

/// The PRF (hash) names the core knows, in registry order, for the picker.
pub fn prf_names() -> impl Iterator<Item = &'static str> {
    vc_types::registry::PRFS.iter().map(|p| p.name)
}

/// Resolve a cipher name to its static registry entry.
pub fn scheme_by_name(name: &str) -> Option<&'static EncryptionScheme> {
    vc_types::registry::ENCRYPTION_SCHEMES
        .iter()
        .find(|s| s.name == name)
}

/// Resolve a PRF name to its static registry entry.
pub fn prf_by_name(name: &str) -> Option<&'static Prf> {
    vc_types::registry::PRFS.iter().find(|p| p.name == name)
}
