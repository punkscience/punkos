//! Launch ns-os in QEMU.
//!
//! `cargo run` boots the BIOS image (no firmware blob needed — QEMU's bundled
//! SeaBIOS). Flags:
//!   --uefi        boot the UEFI image instead (requires OVMF via $OVMF_PATH)
//!   --headless    no QEMU window; serial only (useful for CI / quick checks)
//! Env:
//!   QEMU=<path>   override the qemu-system-x86_64 binary
//!
//! The disk images are produced by build.rs regardless of whether QEMU is
//! installed; their paths are printed so they can be booted by other means.

use std::process::Command;

const BIOS_IMAGE: &str = env!("BIOS_IMAGE");
const UEFI_IMAGE: &str = env!("UEFI_IMAGE");

/// Locate a `qemu-system-x86_64`. Honors $QEMU, then PATH, then known install
/// locations on this machine (MSYS2 CLANGARM64 ships a native ARM64 build).
fn find_qemu() -> String {
    if let Ok(q) = std::env::var("QEMU") {
        return q;
    }
    let candidates = [
        r"C:\msys64\clangarm64\bin\qemu-system-x86_64.exe",
        r"C:\Program Files\qemu\qemu-system-x86_64.exe",
    ];
    for c in candidates {
        if std::path::Path::new(c).exists() {
            return c.to_string();
        }
    }
    // Last resort: assume it's on PATH.
    "qemu-system-x86_64".to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let uefi = args.iter().any(|a| a == "--uefi");
    let headless = args.iter().any(|a| a == "--headless");

    let qemu = find_qemu();

    eprintln!("ns-os :: BIOS image = {BIOS_IMAGE}");
    eprintln!("ns-os :: UEFI image = {UEFI_IMAGE}");

    let mut cmd = Command::new(&qemu);
    if uefi {
        match std::env::var("OVMF_PATH") {
            Ok(ovmf) => {
                cmd.arg("-bios").arg(ovmf);
                cmd.arg("-drive").arg(format!("format=raw,file={UEFI_IMAGE}"));
            }
            Err(_) => {
                eprintln!("--uefi needs an OVMF firmware: set OVMF_PATH=<BOOTX64 OVMF .fd>");
                std::process::exit(2);
            }
        }
    } else {
        cmd.arg("-drive").arg(format!("format=raw,file={BIOS_IMAGE}"));
    }
    cmd.arg("-serial").arg("stdio");
    cmd.arg("-no-reboot");
    if headless {
        cmd.arg("-display").arg("none");
    }

    eprintln!("ns-os :: launching {qemu} ...");
    match cmd.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(0)),
        Err(e) => {
            eprintln!("ns-os :: could not launch QEMU ({qemu}): {e}");
            eprintln!("ns-os :: install QEMU (qemu-system-x86_64) and put it on PATH, or set QEMU=<path>.");
            eprintln!("ns-os :: the disk images above are already built and bootable.");
            std::process::exit(1);
        }
    }
}
