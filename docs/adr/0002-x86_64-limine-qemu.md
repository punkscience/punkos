# Target: x86_64 + QEMU (boot via the pure-Rust `bootloader` crate)

The kernel targets `x86_64` and is tested in **QEMU** (`qemu-system-x86_64`). It
boots via the pure-Rust **`bootloader` crate (0.11)**, which builds BIOS and UEFI
disk images entirely in Rust and hands the kernel a linear framebuffer + memory
map + a mapped higher-half kernel.

## Considered Options

- **Limine** (originally chosen) — **superseded** (see amendment): its bootable
  image creation needs `xorriso`/`mtools`, which are absent on the Windows-on-ARM
  dev host and may lack native ARM64 builds.
- **aarch64 + QEMU virt** — rejected: fewer docs, more hand-rolled early boot.

## Amendment (execution)

Originally this ADR chose **Limine**. During setup we found the dev host is
**Windows-on-ARM** with no `xorriso`/`mtools`/`mkfs.fat`, so the standard Limine
image flow was unworkable without a tooling rabbit hole. We reversed to the
`bootloader` crate (the documented runner-up): it produces images in pure Rust
(no external tools), and BIOS boot needs no OVMF firmware (QEMU's bundled
SeaBIOS). The kernel builds for the precompiled `x86_64-unknown-none` target (no
`build-std` required); the kernel ELF is consumed as a Cargo **artifact
dependency** by an `os` builder/runner crate.

## Consequences

QEMU runs the x86_64 guest under software emulation (TCG) on the ARM host —
correct but slower; fine for a small kernel. Framebuffer config via the
`bootloader` crate is slightly more basic than Limine's GOP; acceptable.
