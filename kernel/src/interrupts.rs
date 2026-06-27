//! Interrupt Descriptor Table, CPU-exception handlers, and the hardware timer.
//!
//! M2 installed the essential exception handlers (breakpoint, double fault, page
//! fault). M3 adds the legacy 8259 PIC, a ~100 Hz PIT timer on IRQ0, and a global
//! tick counter that drives animation. Keyboard (IRQ1) arrives in M5.

use core::sync::atomic::{AtomicU64, Ordering};

use pic8259::ChainedPics;
use spin::{Lazy, Mutex};
use x86_64::instructions::port::Port;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

use crate::gdt;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

/// Monotonic tick count, incremented by the timer IRQ (~100 Hz).
pub static TICKS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }
}

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.page_fault.set_handler_fn(page_fault_handler);
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    }
    idt[InterruptIndex::Timer.as_u8()].set_handler_fn(timer_handler);
    idt[InterruptIndex::Keyboard.as_u8()].set_handler_fn(keyboard_handler);
    idt
});

pub fn init_idt() {
    IDT.load();
}

/// Bring up the PIC + PIT and enable maskable interrupts.
pub fn init() {
    unsafe {
        PICS.lock().initialize();
    }
    init_pit(100); // 100 Hz
    x86_64::instructions::interrupts::enable();
}

/// Program PIT channel 0 to fire at `hz` Hz (mode 3, square wave).
fn init_pit(hz: u32) {
    let divisor = (1_193_182u32 / hz) as u16;
    unsafe {
        let mut command: Port<u8> = Port::new(0x43);
        let mut channel0: Port<u8> = Port::new(0x40);
        command.write(0x36u8);
        channel0.write((divisor & 0xFF) as u8);
        channel0.write((divisor >> 8) as u8);
    }
}

/// Current tick count.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

pub fn init_keyboard() {
    // IRQ1 is already unmasked by the PIC initialisation; the IDT entry was
    // registered at boot. Nothing extra needed on the PIC side for PS/2.
}

// --- handlers ---------------------------------------------------------------

extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    TICKS.fetch_add(1, Ordering::Relaxed);
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    crate::keyboard::handle_scancode();
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    crate::serial_println!("[int] breakpoint at {:#x}", stack_frame.instruction_pointer);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("double fault\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    crate::serial_println!(
        "[int] PAGE FAULT  addr={:?}  code={:?}",
        Cr2::read(),
        error_code
    );
    crate::serial_println!("{:#?}", stack_frame);
    crate::hcf();
}
