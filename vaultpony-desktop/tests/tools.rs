//! Headless coverage of the header tools (P4).
//!
//! Creates a container, then exercises volume info, header backup, change
//! password, and restore, reopening with the core to confirm each took effect.
//! PIM 1 keeps derivations fast without changing any code path.

use std::fs;
use std::path::{Path, PathBuf};

use vault_core::{ContainerFs, Session};
use vaultpony_desktop::newvolume::{self, Spec};
use vaultpony_desktop::tools;
use zeroize::Zeroizing;

// PIM 1 is below the default, so both need 20+ characters (VeraCrypt's rule).
const A: &[u8] = b"first password for the tools test";
const B: &[u8] = b"second password for the tools test";
const PIM: u32 = 1;

fn scratch(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name)
}

fn create_with(path: &Path, pass: &[u8]) -> anyhow::Result<()> {
    newvolume::create(&Spec {
        path: path.to_path_buf(),
        size: 8 * 1024 * 1024,
        scheme: newvolume::scheme_by_name("AES").expect("AES"),
        prf: newvolume::prf_by_name("SHA-512").expect("SHA-512"),
        pim: PIM,
        fs: ContainerFs::Fat,
        passphrase: Zeroizing::new(pass.to_vec()),
        keyfiles: Vec::new(),
        hidden: None,
    })
}

fn opens(path: &Path, pass: &[u8]) -> bool {
    Session::unlock_with(path, pass, PIM, false, &mut |_, _, _| {}).is_ok()
}

#[test]
fn info_is_support_safe_and_reports_parameters() -> anyhow::Result<()> {
    let path = scratch("tools_info.hc");
    create_with(&path, A)?;

    let text = tools::info(&path, A, PIM)?;
    assert!(text.contains("cipher:"));
    assert!(text.contains("AES"));
    assert!(text.contains("SHA-512"));
    // Support-safe: no path leaks into the output.
    assert!(!text.contains("tools_info"));
    Ok(())
}

#[test]
fn backup_change_and_restore_round_trip() -> anyhow::Result<()> {
    let path = scratch("tools_ckr.hc");
    create_with(&path, A)?;

    // Back up the header while A is the password.
    let backup = scratch("header.hcbak");
    tools::backup_header(&path, &backup)?;
    assert!(fs::metadata(&backup)?.len() > 0, "backup file has content");

    // Change A -> B.
    tools::change_password(&path, A, PIM, B, PIM, None)?;
    assert!(opens(&path, B), "new password opens the volume");
    assert!(
        !opens(&path, A),
        "old password no longer opens the primary header"
    );

    // The backup still accepts A, so restoring it brings A back.
    tools::restore_from_file(&path, &backup, A, PIM)?;
    assert!(
        opens(&path, A),
        "restoring the backup revived the old password"
    );
    Ok(())
}

#[test]
fn change_password_can_move_the_header_to_argon2id_and_back() -> anyhow::Result<()> {
    let path = scratch("tools_kdf.hc");
    create_with(&path, A)?;

    tools::change_password(&path, A, PIM, B, PIM, Some("Argon2id"))?;
    let s = Session::unlock_with(&path, B, PIM, false, &mut |_, _, _| {})?;
    assert_eq!(s.prf(), "Argon2id");
    drop(s);

    tools::change_password(&path, B, PIM, A, PIM, Some("SHA-512"))?;
    let s = Session::unlock_with(&path, A, PIM, false, &mut |_, _, _| {})?;
    assert_eq!(s.prf(), "SHA-512");
    Ok(())
}

#[test]
fn change_password_refuses_a_short_password_at_a_low_pim() -> anyhow::Result<()> {
    let path = scratch("tools_short.hc");
    create_with(&path, A)?;
    let err = tools::change_password(&path, A, PIM, b"short", PIM, Some("Argon2id"))
        .expect_err("a short password at PIM 1 must be refused");
    assert!(err.to_string().contains("20"), "{err}");
    assert!(opens(&path, A), "nothing was written");
    Ok(())
}

#[test]
fn embedded_restore_succeeds_on_a_healthy_container() -> anyhow::Result<()> {
    let path = scratch("tools_emb.hc");
    create_with(&path, A)?;

    tools::restore_from_embedded(&path, A, PIM)?;
    assert!(
        opens(&path, A),
        "container still opens after an embedded restore"
    );
    Ok(())
}
