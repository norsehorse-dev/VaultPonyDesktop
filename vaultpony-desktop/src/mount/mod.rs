//! The OS-mount layer (PLAN.md section 4). Empty until P5.
//!
//! The design: a single trait the three platform backends implement, all
//! translating the core's `vc_fs::Vfs` into the platform's userspace filesystem
//! callbacks (`fuser` on Linux, FSKit or macFUSE on macOS, WinFsp or Dokan on
//! Windows). Block-level mounting, which is what VeraCrypt itself does, is ruled
//! out here: it needs signed kernel drivers on macOS and Windows, and on Linux
//! it means NBD, whose surface collides with the zero-network invariant.
//!
//! Nothing is implemented yet. This module is a placeholder so the shape of the
//! tree matches the plan and the mount work has a home to land in.
