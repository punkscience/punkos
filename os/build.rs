//! Build the bootable disk images from the compiled kernel ELF.
//!
//! Cargo builds the `kernel` artifact dependency for `x86_64-unknown-none` and
//! exposes its path via `CARGO_BIN_FILE_KERNEL_kernel`. We hand that to the
//! `bootloader` crate to produce BIOS and UEFI images, then export their paths
//! as compile-time env vars for `main.rs` to launch.

use std::path::PathBuf;

fn main() {
    let kernel = PathBuf::from(
        std::env::var_os("CARGO_BIN_FILE_KERNEL_kernel")
            .expect("kernel artifact dependency not found (need nightly + bindeps)"),
    );
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    let bios = out_dir.join("punkos-bios.img");
    bootloader::BiosBoot::new(&kernel)
        .create_disk_image(&bios)
        .expect("failed to create BIOS disk image");

    let uefi = out_dir.join("punkos-uefi.img");
    bootloader::UefiBoot::new(&kernel)
        .create_disk_image(&uefi)
        .expect("failed to create UEFI disk image");

    println!("cargo:rustc-env=BIOS_IMAGE={}", bios.display());
    println!("cargo:rustc-env=UEFI_IMAGE={}", uefi.display());
    println!("cargo:rerun-if-changed={}", kernel.display());
}
