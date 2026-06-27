//! xHCI (eXtensible Host Controller Interface) driver (M6b).
//!
//! Initialises the xHCI USB 3.x host controller found by the PCI scanner.
//! Implements the controller bring-up sequence defined in the Intel xHCI spec
//! (rev 1.2): stop → reset → configure → start, then set up the command ring,
//! event ring, and Device Context Base Address Array (DCBAA).
//!
//! # Architecture
//!
//! - All MMIO addresses use raw `u64` (not `VirtAddr`) to avoid canonical-address
//!   truncation — the phys_offset identity map produces addresses like
//!   `0x1_0000_febf_0000` which are NOT x86_64 canonical (bit 47 ≠ bits 63:48).
//! - `Registers` — typed accessor for the MMIO register space.
//! - `Controller` — owns the ring buffers and exposes the init sequence.
//! - Ring buffers are 64-byte aligned and allocated from the kernel heap.
//!   Physical addresses for DMA are resolved via `memory::virt_to_phys`.
//!
//! # References
//!
//! - eXtensible Host Controller Interface for Universal Serial Bus (xHCI), rev 1.2
//! - QEMU xHCI emulation (hw/usb/hcd-xhci.c) — 1b36:000d

use alloc::alloc::{alloc, Layout};
use alloc::boxed::Box;
use core::ptr;

use x86_64::VirtAddr;

use crate::memory;

// --- MMIO register structure -------------------------------------------------

/// Operational + capability + runtime register view over the xHCI MMIO BAR.
///
/// All addresses are raw `u64` virtual addresses (not `VirtAddr`) because the
/// physical-memory offset identity map produces non-canonical addresses.
pub struct Registers {
    base: u64,
    op_offset: u64,
    rt_offset: u64,
}

impl Registers {
    /// # Safety
    /// `bar_phys` must be the physical MMIO base address from the xHCI PCI BAR,
    /// and `phys_offset` must be the virtual base where all physical memory is
    /// identity-mapped.
    pub unsafe fn new(bar_phys: u64, phys_offset: VirtAddr) -> Self {
        let base = phys_offset.as_u64() + bar_phys;
        let caplength = ptr::read_volatile(base as *const u8) as u64;
        let rtsoff = ptr::read_volatile((base + 0x18) as *const u32) as u64;
        Self { base, op_offset: caplength, rt_offset: rtsoff }
    }

    // -- capability registers (relative to BAR base) -------------------------

    pub fn hciversion(&self) -> u16 {
        (unsafe { ptr::read_volatile((self.base + 0x00) as *const u32) >> 16 }) as u16
    }

    pub fn max_slots(&self) -> u8 {
        self.read_cap(4) as u8
    }

    pub fn max_ports(&self) -> u8 {
        ((self.read_cap(4) >> 24) & 0xFF) as u8
    }

    #[allow(dead_code)]
    pub fn max_scratchpad_bufs(&self) -> u16 {
        let v = self.read_cap(8);
        let hi = (v >> 27) & 0x1F;
        let lo = (v >> 21) & 0x1F;
        ((hi << 5) | lo) as u16
    }

    #[allow(dead_code)]
    pub fn context_size_64(&self) -> bool {
        self.read_cap(0x10) & (1 << 2) != 0
    }

    fn read_cap(&self, offset: u64) -> u32 {
        unsafe { ptr::read_volatile((self.base + offset) as *const u32) }
    }

    // -- operational registers (relative to BAR + caplength) -----------------

    fn op_addr(&self, offset: u64) -> u64 {
        self.base + self.op_offset + offset
    }

    fn read_op(&self, offset: u64) -> u32 {
        unsafe { ptr::read_volatile(self.op_addr(offset) as *const u32) }
    }

    fn write_op(&self, offset: u64, value: u32) {
        unsafe { ptr::write_volatile(self.op_addr(offset) as *mut u32, value) };
    }

    fn write_op64(&self, offset: u64, value: u64) {
        unsafe {
            ptr::write_volatile(self.op_addr(offset) as *mut u32, value as u32);
            ptr::write_volatile(self.op_addr(offset + 4) as *mut u32, (value >> 32) as u32);
        }
    }

    pub fn usbcmd(&self) -> u32 { self.read_op(0x00) }
    pub fn write_usbcmd(&self, v: u32) { self.write_op(0x00, v) }

    pub fn usbsts(&self) -> u32 { self.read_op(0x04) }
    #[allow(dead_code)]
    pub fn write_usbsts(&self, v: u32) { self.write_op(0x04, v) }

    #[allow(dead_code)]
    pub fn pagesize(&self) -> u32 { self.read_op(0x08) }

    pub fn write_dcbaap(&self, addr: u64) {
        // Split into two 32-bit writes — QEMU drops high bits on 64-bit access.
        self.write_op(0x30, addr as u32);
        self.write_op(0x34, (addr >> 32) as u32);
    }
    pub fn write_crcr(&self, addr: u64) {
        self.write_op(0x18, addr as u32);
        self.write_op(0x1c, (addr >> 32) as u32);
    }
    pub fn write_config(&self, v: u32) { self.write_op(0x38, v) }

    /// Return a mutable pointer to a doorbell register.  Doorbell array starts
    /// at BAR + DBOFF; doorbell `target` (slot or 0 for commands) rings when
    /// any value is written to it.
    pub fn doorbell(&self, target: u32) -> *mut u32 {
        let dboff = (self.read_cap(0x14) & !0x3) as u64;
        (self.base + dboff + target as u64 * 4) as *mut u32
    }

    // -- runtime registers (relative to BAR + rtsoff) ------------------------

    fn rt_addr(&self, offset: u64) -> u64 {
        self.base + self.rt_offset + offset
    }

    /// Write the Interrupter 0 Event Ring Segment Table pointer (two 32-bit
    /// writes — QEMU's handler drops high 32 bits on 64-bit access).
    pub fn write_erstba(&self, addr: u64) {
        unsafe {
            ptr::write_volatile(self.rt_addr(0x30) as *mut u32, addr as u32);
            // QEMU calls xhci_er_reset() on the HIGH half write — order matters!
            ptr::write_volatile(self.rt_addr(0x34) as *mut u32, (addr >> 32) as u32);
        }
    }

    pub fn write_erstsz(&self, count: u16) {
        unsafe { ptr::write_volatile(self.rt_addr(0x28) as *mut u16, count) };
    }

    pub fn write_erdp(&self, addr: u64) {
        unsafe {
            ptr::write_volatile(self.rt_addr(0x38) as *mut u32, addr as u32);
            ptr::write_volatile(self.rt_addr(0x3c) as *mut u32, (addr >> 32) as u32);
        }
    }

    pub fn enable_interrupter(&self) {
        unsafe {
            // Write 1 to IP (bit 1) to clear pending, then set IE (bit 0).
            ptr::write_volatile(self.rt_addr(0x20) as *mut u32, 1 << 1);
            ptr::write_volatile(self.rt_addr(0x20) as *mut u32, 1 << 0);
        }
    }

    // -- port registers ------------------------------------------------------

    pub fn portsc_addr(&self, port: u8) -> u64 {
        self.base + self.op_offset + 0x400 + ((port as u64 - 1) * 0x10)
    }

    pub fn read_portsc(&self, port: u8) -> u32 {
        unsafe { ptr::read_volatile(self.portsc_addr(port) as *const u32) }
    }

    pub fn write_portsc(&self, port: u8, value: u32) {
        unsafe { ptr::write_volatile(self.portsc_addr(port) as *mut u32, value) };
    }
}

// --- USBSTS / USBCMD bitfields -----------------------------------------------

pub mod usbcmd {
    pub const RS: u32 = 1 << 0;
    pub const HCRST: u32 = 1 << 1;
    pub const INTE: u32 = 1 << 2;
    pub const HSEE: u32 = 1 << 3;
}

pub mod usbsts {
    pub const HCH: u32 = 1 << 0;
    pub const CNR: u32 = 1 << 11;
}

pub mod portsc {
    pub const CCS: u32 = 1 << 0;
    pub const PED: u32 = 1 << 1;
    pub const PR: u32 = 1 << 4;
    pub const PP: u32 = 1 << 9;
    pub const PRC: u32 = 1 << 21;
    pub const SPEED_SHIFT: u32 = 10;
    pub const SPEED_MASK: u32 = 0xF << 10;
}

// --- transfer request block (TRB) types --------------------------------------

#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct Trb {
    pub parameter: u64,
    pub status: u32,
    pub control: u32,
}

impl Trb {
    pub const fn zero() -> Self {
        Self { parameter: 0, status: 0, control: 0 }
    }

    pub fn cycle_bit(&self) -> bool {
        self.control & 1 != 0
    }

    pub fn trb_type(&self) -> u8 {
        ((self.control >> 10) & 0x3F) as u8
    }
}

#[allow(dead_code)]
pub mod trb_type {
    pub const LINK: u8 = 6;
    pub const NO_OP_CMD: u8 = 23;        // No Op Command (command ring)
    pub const ENABLE_SLOT: u8 = 9;
    pub const DISABLE_SLOT: u8 = 10;
    pub const ADDRESS_DEVICE: u8 = 11;
    pub const CONFIGURE_ENDPOINT: u8 = 12;
    pub const EVALUATE_CONTEXT: u8 = 13;
    pub const RESET_ENDPOINT: u8 = 14;
    pub const STOP_ENDPOINT: u8 = 15;
    pub const SET_TR_DEQUEUE: u8 = 16;
    pub const RESET_DEVICE: u8 = 17;
    pub const TRANSFER_EVENT: u8 = 32;
    pub const COMMAND_COMPLETION: u8 = 33;
    pub const PORT_STATUS_CHANGE: u8 = 34;
}

// --- ring buffers -----------------------------------------------------------

pub struct Ring {
    trbs: Box<[Trb]>,
    phys: u64,
    enqueue: usize,
    cycle: bool,
}

impl Ring {
    pub fn new(count: usize, phys_offset: VirtAddr) -> Self {
        assert!(count > 0 && count.is_power_of_two());

        let size = count * core::mem::size_of::<Trb>();
        let layout = Layout::from_size_align(size + 64, 64).expect("invalid ring layout");
        let ptr = unsafe { alloc(layout) };
        assert!(!ptr.is_null(), "ring allocation failed");

        let aligned = ((ptr as usize + 63) & !63) as *mut Trb;
        unsafe { for i in 0..count { ptr::write(aligned.add(i), Trb::zero()); } }

        let virt = VirtAddr::new(aligned as u64);
        let phys = unsafe { memory::virt_to_phys(virt, phys_offset) }
            .expect("ring: cannot resolve physical address");

        Self {
            trbs: unsafe { Box::from_raw(core::slice::from_raw_parts_mut(aligned, count)) },
            phys,
            enqueue: 0,
            cycle: true,
        }
    }

    pub fn physical_address(&self) -> u64 { self.phys }

    pub fn push(&mut self, trb: Trb) {
        let idx = self.enqueue;
        self.trbs[idx] = Trb {
            control: (trb.control & !1) | (self.cycle as u32),
            ..trb
        };
        self.enqueue = (self.enqueue + 1) & (self.trbs.len() - 1);
    }

    #[allow(dead_code)]
    pub fn toggle_cycle(&mut self) { self.cycle = !self.cycle; }
}

// --- event ring segment table entry ------------------------------------------

#[repr(C, align(16))]
struct ErstEntry {
    ring_base: u64,
    ring_size: u16,
    _reserved: u16,
    _rsvd2: u32,
}

// --- device context base address array --------------------------------------

struct Dcbaa {
    #[allow(dead_code)]
    ptrs: Box<[u64]>,
    phys: u64,
}

impl Dcbaa {
    fn new(max_slots: u8, phys_offset: VirtAddr) -> Self {
        let count = max_slots as usize + 1;
        let size = count * 8;
        let layout = Layout::from_size_align(size + 64, 64).expect("invalid DCBAA layout");
        let ptr = unsafe { alloc(layout) };
        assert!(!ptr.is_null(), "DCBAA allocation failed");
        let aligned = ((ptr as usize + 63) & !63) as *mut u64;
        unsafe { for i in 0..count { ptr::write(aligned.add(i), 0); } }
        let virt = VirtAddr::new(aligned as u64);
        let phys = unsafe { memory::virt_to_phys(virt, phys_offset) }
            .expect("DCBAA: cannot resolve physical address");
        Self {
            ptrs: unsafe { Box::from_raw(core::slice::from_raw_parts_mut(aligned, count)) },
            phys,
        }
    }

    fn physical_address(&self) -> u64 { self.phys }
}

// --- controller --------------------------------------------------------------

pub struct Controller {
    regs: Registers,
    cmd_ring: Ring,
    event_ring: Ring,
    dcbaa: Dcbaa,
    max_slots: u8,
    max_ports: u8,
    #[allow(dead_code)]
    phys_offset: VirtAddr,
}

impl Controller {
    pub fn init(bar_phys: u64, phys_offset: VirtAddr) -> Self {
        let regs = unsafe { Registers::new(bar_phys, phys_offset) };

        let ver = regs.hciversion();
        crate::serial_println!(
            "[xhci] xHCI version: {}.{}.{}",
            (ver >> 8) & 0xFF, (ver >> 4) & 0x0F, ver & 0x0F,
        );

        let max_slots = regs.max_slots();
        let max_ports = regs.max_ports();
        crate::serial_println!("[xhci] max slots: {}, max ports: {}", max_slots, max_ports);

        // step 1: stop
        let cmd = regs.usbcmd();
        regs.write_usbcmd(cmd & !usbcmd::RS);
        crate::serial_println!("[xhci] stopped controller, waiting for HCHalted...");
        while regs.usbsts() & usbsts::HCH == 0 { core::hint::spin_loop(); }
        crate::serial_println!("[xhci] controller halted");

        // step 2: reset
        regs.write_usbcmd(cmd | usbcmd::HCRST);
        crate::serial_println!("[xhci] resetting controller...");
        while regs.usbcmd() & usbcmd::HCRST != 0 { core::hint::spin_loop(); }
        while regs.usbsts() & usbsts::CNR != 0 { core::hint::spin_loop(); }
        crate::serial_println!("[xhci] reset complete, controller ready");

        // step 3: allocate
        let cmd_ring = Ring::new(256, phys_offset);
        let event_ring = Ring::new(256, phys_offset);
        let dcbaa = Dcbaa::new(max_slots, phys_offset);

        crate::serial_println!(
            "[xhci] cmd ring phys={:#x}, event ring phys={:#x}, dcbaa phys={:#x}",
            cmd_ring.physical_address(),
            event_ring.physical_address(),
            dcbaa.physical_address(),
        );

        // step 4: configure
        regs.write_config(max_slots as u32);
        crate::serial_println!("[xhci] configured for {} device slots", max_slots);

        regs.write_dcbaap(dcbaa.physical_address());
        regs.write_crcr(cmd_ring.physical_address() | 1); // RCS = 1

        let (seg_phys, _) = allocate_erst(&event_ring, phys_offset);
        regs.write_erstba(seg_phys);
        regs.write_erstsz(1);
        regs.write_erdp(event_ring.physical_address());
        regs.enable_interrupter();

        // step 5: start
        regs.write_usbcmd(usbcmd::RS | usbcmd::INTE | usbcmd::HSEE);
        crate::serial_println!("[xhci] starting controller...");
        while regs.usbsts() & usbsts::HCH != 0 { core::hint::spin_loop(); }
        crate::serial_println!("[xhci] init complete");

        Controller { regs, cmd_ring, event_ring, dcbaa, max_slots, max_ports, phys_offset }
    }

    pub fn noop(&mut self) -> bool {
        self.cmd_ring.push(Trb {
            parameter: 0,
            status: 0,
            control: ((trb_type::NO_OP_CMD as u32) << 10),
        });

        unsafe { ptr::write_volatile(self.regs.doorbell(0), 0); }

        // Poll for command completion event.
        for _ in 0..1_000_000 {
            let trb = &self.event_ring.trbs[0];
            if trb.cycle_bit() == self.cmd_ring.cycle
                && trb.trb_type() == trb_type::COMMAND_COMPLETION
            {
                let cc = (trb.status >> 24) as u8;
                self.event_ring.trbs[0] = Trb::zero();
                return cc == 1;
            }
            core::hint::spin_loop();
        }
        // No-Op is a health check; controller works fine without it.
        false
    }

    pub fn initialise_ports(&mut self) -> u8 {
        let mut connected = 0;
        for port in 1..=self.max_ports {
            let sc = self.regs.read_portsc(port);
            crate::serial_println!("[xhci] port {} PORTSC = {:#010x}", port, sc);

            if sc & portsc::CCS == 0 { continue; }
            connected += 1;

            let speed = (sc & portsc::SPEED_MASK) >> portsc::SPEED_SHIFT;
            crate::serial_println!("[xhci] port {} connected, speed = {}", port, speed);

            if sc & portsc::PED == 0 {
                crate::serial_println!("[xhci] port {} not enabled, resetting...", port);
                self.regs.write_portsc(port, sc | portsc::PP | portsc::PR);
                loop {
                    let sc = self.regs.read_portsc(port);
                    if sc & portsc::PRC != 0 {
                        self.regs.write_portsc(port, sc | portsc::PRC);
                        break;
                    }
                    core::hint::spin_loop();
                }
                let sc = self.regs.read_portsc(port);
                crate::serial_println!(
                    "[xhci] port {} reset complete, PED={}, speed={}",
                    port,
                    sc & portsc::PED != 0,
                    (sc & portsc::SPEED_MASK) >> portsc::SPEED_SHIFT,
                );
            }
        }
        connected
    }
}

fn allocate_erst(event_ring: &Ring, phys_offset: VirtAddr) -> (u64, *mut ErstEntry) {
    let layout = Layout::from_size_align(
        core::mem::size_of::<ErstEntry>() + 64, 64,
    ).expect("invalid ERST layout");
    let ptr = unsafe { alloc(layout) };
    assert!(!ptr.is_null(), "ERST allocation failed");
    let aligned = ((ptr as usize + 63) & !63) as *mut ErstEntry;

    unsafe {
        ptr::write(aligned, ErstEntry {
            ring_base: event_ring.physical_address(),
            ring_size: event_ring.trbs.len() as u16,
            _reserved: 0,
            _rsvd2: 0,
        });
    }

    let virt = VirtAddr::new(aligned as u64);
    let phys = unsafe { memory::virt_to_phys(virt, phys_offset) }
        .expect("ERST: cannot resolve physical address");
    (phys, aligned)
}