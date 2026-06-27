# punkos — bare-metal Rust OS

Work in: `kernel/src/` (no_std kernel) and `os/` (host builder/runner).
Read `AGENTS.md` for the full agent manual.

## Quick start

```bash
cargo run               # build + boot in QEMU (BIOS)
cargo run -- --headless  # serial only, no QEMU window
cargo test -p os         # host-side tests
```

QEMU path: `C:\msys64\clangarm64\bin\qemu-system-x86_64.exe`

## Current state

M1–M6b are done (framebuffer, memory, timer, renderer, PS/2 kbd, PCI+xHCI).
M7 (USB enum) is next. All work is tracked in GitHub milestones M7–M13.

## Domain

Read `CONTEXT.md` for the glossary. Key terms: Fragment, Relation, Idea,
Capture, Pod, Quad store, Identity, Device. Avoid banned synonyms.

## Code style

- TDD + SOLID always
- Log with `serial_println!` — the only I/O channel
- MMIO addresses use raw `u64` (not `VirtAddr`) where identity-map addresses
  are non-canonical
- `pub(crate)` visibility, `#[allow(dead_code)]` on spec-complete unused items
- `# Safety` comments on every `unsafe` block
