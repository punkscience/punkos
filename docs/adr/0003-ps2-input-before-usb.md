# PS/2 input for the MVP, USB stack as the next milestone

MVP keyboard input uses the emulated **PS/2** controller (a few I/O ports +
interrupts, readable almost immediately). The real **USB** path (xHCI host
controller → USB core → HID class) is the dedicated milestone immediately after
the MVP.

## Context

Despite "USB ports" being an explicit product goal, a USB-HID stack from scratch
is weeks of work before the first keypress arrives, whereas PS/2 makes the
capture demo interactive at once. Both are "real hardware" from the kernel's view;
PS/2 is simply far simpler. Mouse/touchpad are deferred — the capture→graph demo
needs only the keyboard.
