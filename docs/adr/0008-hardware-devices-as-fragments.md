# Hardware devices are Fragments (self-describing machine)

The OS enumerates its own hardware — for the MVP, storage devices (via PCI scan +
virtio-blk) — and mints a **Fragment** per device into the quad store, linked to
the Identity via `pim:storage`, rendered as acid-bubble nodes in the same graph.

## Context

This extends the uniform-fragment-graph (ADR-0004) and quad-native core
(ADR-0005): *everything*, including the machine's own body, is a fragment. The
second brain is aware of itself. The MVP scopes this to storage; generalising to
all hardware (input devices, framebuffer, CPU/memory) is the roadmap direction.

## Consequences

We persist a private key unencrypted to a block device for the QEMU demo
(acceptable short-term); encryption-at-rest (passphrase / TPM / secure element)
is a tracked future task.
