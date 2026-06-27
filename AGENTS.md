# AGENTS.md — punkos agent manual

Everything an agent needs to clone, build, understand, and contribute to punkos.

---

## Project overview

punkos is a from-scratch, bare-metal Rust operating system. It targets `x86_64`
and boots via the pure-Rust `bootloader` crate in QEMU. No host OS underneath —
the kernel talks directly to hardware.

The defining product concept: the machine is the user's SOLID **pod** and second
brain. Everything the user (and the machine) knows is stored as **Fragments** in
an RDF-native quad store. The OS treats itself as a self-describing graph —
hardware devices are also Fragments.

---

## Architecture

```
punkos/                      # Cargo workspace
├── kernel/                  # #![no_std] kernel, built for x86_64-unknown-none
│   └── src/
│       ├── main.rs          # Entry point + milestone boot sequence
│       ├── allocator.rs     # 8 MiB kernel heap (linked_list_allocator)
│       ├── framebuffer.rs   # Linear framebuffer + brand colors
│       ├── gdt.rs           # GDT + TSS with double-fault IST
│       ├── interrupts.rs    # IDT, PIC, PIT timer, keyboard IRQ
│       ├── keyboard.rs      # PS/2 scancode handler + read_line
│       ├── memory.rs        # Paging + frame allocator + virt_to_phys
│       ├── pci.rs           # PCI bus enumeration (config mech #1)
│       ├── render.rs        # Acid build-bubble TUI renderer
│       ├── serial.rs        # Serial port macros (serial_println!)
│       └── xhci.rs          # xHCI USB host controller driver
├── os/                      # Host-side builder+runner (builds for host triple)
│   ├── build.rs             # Turns kernel ELF → BIOS/UEFI disk images
│   └── src/main.rs          # Launches QEMU with the disk image
├── docs/
│   ├── adr/                 # Architecture Decision Records (0001–0008)
│   └── agents/              # Agent skill docs (domain, issue tracker, triage)
├── CONTEXT.md               # Domain glossary (read this first)
├── AGENTS.md                # This file
├── CLAUDE.md                # Bootstrap (points here)
└── Cargo.toml               # Workspace root (default-members = ["os"])
```

**Boot flow**: `bootloader` crate → kernel ELF → BIOS/UEFI disk image → QEMU.
The bootloader hands the kernel a linear framebuffer, memory map, and
physical-memory offset (identity map). The kernel initializes its own GDT, IDT,
paging, and heap.

---

## Current state (milestones completed)

| M# | Milestone | Status |
|----|-----------|--------|
| M1 | Framebuffer (Hardcore Black clear) | ✅ done |
| M2 | Core plumbing: GDT/IDT/paging/8 MiB heap | ✅ done |
| M3 | PIC + PIT timer (~100 Hz tick counter) | ✅ done |
| M4 | Acid build-bubble animated TUI | ✅ done |
| M5 | PS/2 keyboard input (scancode → read_line) | ✅ done |
| M6a | PCI bus enumeration | ✅ done |
| M6b | xHCI controller init + port detection | ✅ done |

**Next up**: See GitHub milestones. The roadmap:

```
M7 (USB enum) ──→ M8 (USB HID kbd) ──┐
M9 (quad store) ─────────────────────┤  parallel
                                     ├──→ M10 (identity) ──→ M12 (NVMe+persistence) ──→ M11 (capture) ──→ M13 (self-describing)
```

---

## Build and run

### Prerequisites

- Rust Nightly (see `rust-toolchain.toml` — currently `nightly-aarch64-pc-windows-gnullvm`)
- QEMU (`qemu-system-x86_64`) on PATH, or set `QEMU=<path>`
- On this dev machine: MSYS2 CLANGARM64 provides both the LLVM toolchain and QEMU at `C:\msys64\clangarm64\bin\`

### Commands

```bash
# Build everything + boot in QEMU (BIOS, SeaBIOS — no firmware needed)
cargo run

# Build everything + boot UEFI (needs OVMF_PATH set)
cargo run -- --uefi

# Headless: no QEMU window, serial output only
cargo run -- --headless

# Build the kernel only (check for compilation errors without booting)
cargo build -p kernel --target x86_64-unknown-none

# Clean build
cargo clean && cargo run
```

`cargo run` from the repo root always works — it builds the `os` crate (the
default workspace member), which pulls the kernel as an artifact dependency and
produces the bootable image.

### QEMU devices

The runner attaches these QEMU devices:
- `-device qemu-xhci` — xHCI USB 3.x host controller (PCI 1b36:000d)
- `-device usb-kbd` — emulated USB keyboard for HID testing
- `-serial stdio` — serial console output

---

## Agent skills

### Issue tracker

GitHub Issues via the `gh` CLI. PRs are not a triage surface.
See `docs/agents/issue-tracker.md`.

**Milestones**: The project is organized into structured milestones (M7–M13).
Each milestone has 5–7 issues. Issues reference their ADR or spec section.

**Labels**: Domain labels (`kernel`, `usb`, `store`, `tui`, `identity`,
`storage`, `hardware`) categorize work. Triage labels use the standard
vocabulary. See `docs/agents/triage-labels.md`.

**Workflow for picking up an issue**:
1. Read the issue body — it references spec sections and ADRs
2. Assign yourself: `gh issue edit <N> --add-assignee @me`
3. Mark in-progress: apply `ready-for-agent` label
4. Implement following TDD + SOLID
5. When done, close with a comment summarizing what shipped
6. Move to the next issue in the same milestone (within a milestone, issues are
dependency-ordered top to bottom)

### Code conventions

**Always use TDD**: write a failing test first, implement the smallest change to
pass it, then refactor.

**Apply SOLID principles** when writing or refactoring.

**Module patterns** used in the existing codebase:
- `serial_println!` / `serial_print!` macros for all logging — the only I/O
- Hardware drivers are plain structs with `::init()` constructors
- MMIO uses raw `u64` addresses (not `VirtAddr`) for non-canonical addresses in
the identity map
- `pub(crate)` visibility for kernel-internal items
- `#[allow(dead_code)]` on items that exist for spec completeness but aren't
called yet
- `core::hint::spin_loop()` for busy-wait polling
- `unsafe` blocks are minimal and documented with `# Safety` comments

**Testing approach**:
- Kernel unit tests: run on the host via `cargo test` in the `os` crate (for
pure data structures like the quad store)
- Integration tests: boot in QEMU, observe serial output — the kernel's own
`[ ok ]` and `[!!]` messages serve as assertions
- Fuzz tests: host-side randomized correctness checks (see M9, issue #15)

### Domain docs

Single-context layout — one `CONTEXT.md` + `docs/adr/` at the repo root.
See `docs/agents/domain.md`.

**Before writing any code, read**:
1. `CONTEXT.md` — the domain glossary
2. ADRs that touch your area (especially 0003–0008 for anything beyond M6)

**Use the glossary's vocabulary** in code identifiers, issue comments, and
commit messages. Avoid banned synonyms listed in CONTEXT.md.

### Triage labels

Default vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`,
`ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

---

## Key specs referenced by issues

- **xHCI spec rev 1.2** — USB host controller (M7, M8)
- **USB 2.0 spec** — chapter 9 (device framework), HID spec (M8)
- **NVMe spec rev 1.4+** — NVM Express controller (M12)
- **PCI Express Base spec** — PCI configuration space (M6, M12)
- **W3C did:key method** — self-sovereign identity (M10)
- **RDF 1.1 Concepts** — quad store semantics (M9)
- **FOAF, DC, PIM vocabularies** — WebID profile (M10, M13)

