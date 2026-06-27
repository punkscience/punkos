//! punkos kernel entry point.
//!
//! Boot brings up, in order: serial logging, the brand-coloured framebuffer
//! (M1), then the core kernel plumbing (M2) — GDT/TSS, interrupt handlers,
//! paging, and a heap allocator. Later milestones add a timer, the renderer,
//! input, and the fragment/quad store.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

#[macro_use]
mod serial;
mod allocator;
mod framebuffer;
mod gdt;
mod interrupts;
mod keyboard;
mod memory;
mod render;

use alloc::boxed::Box;
use alloc::vec::Vec;
use bootloader_api::config::{BootloaderConfig, Mapping};
use bootloader_api::{entry_point, BootInfo};
use core::panic::PanicInfo;
use x86_64::VirtAddr;

/// Ask the bootloader to map all physical memory at a dynamic virtual offset so
/// we can edit page tables and reach frames.
static CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    serial::init();
    serial_println!();
    serial_println!("punk.science // ns-os");
    serial_println!("build dots loading  [* * *]");
    serial_println!("[ ok ] kernel reached long mode and the entry point");

    // M1: clear the screen to the brand's Hardcore Black.
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        let info = fb.info();
        framebuffer::clear(fb, framebuffer::color::HARDCORE_BLACK);
        serial_println!(
            "[ ok ] framebuffer cleared to Hardcore Black: {}x{} {:?}",
            info.width,
            info.height,
            info.pixel_format
        );
    } else {
        serial_println!("[!!] no framebuffer provided by bootloader");
    }

    // M2: core memory plumbing.
    gdt::init();
    interrupts::init_idt();
    serial_println!("[ ok ] GDT + IDT installed");

    // Prove the IDT works: a breakpoint should be caught and execution resume.
    x86_64::instructions::interrupts::int3();
    serial_println!("[ ok ] returned from breakpoint exception");

    let phys_offset = VirtAddr::new(
        boot_info
            .physical_memory_offset
            .into_option()
            .expect("bootloader did not map physical memory"),
    );
    let memory_regions: &'static MemoryRegions = &boot_info.memory_regions;
    let mut mapper = unsafe { memory::init(phys_offset) };
    let mut frame_allocator = unsafe { memory::BootInfoFrameAllocator::init(memory_regions) };
    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialisation failed");
    serial_println!(
        "[ ok ] paging + {} KiB heap online",
        allocator::HEAP_SIZE / 1024
    );

    // Exercise the heap.
    let boxed = Box::new(0xDEADBEEFu64);
    let mut v: Vec<u64> = Vec::new();
    for i in 0..500 {
        v.push(i);
    }
    serial_println!(
        "[ ok ] heap works: box={:#x}, sum(0..500)={}",
        *boxed,
        v.iter().sum::<u64>()
    );

    serial_println!("[ ok ] M2 complete — memory management online");

    // M3: hardware timer.
    interrupts::init();
    serial_println!("[ ok ] PIC + PIT up, interrupts enabled (~100 Hz)");
    while interrupts::ticks() < 10 {
        x86_64::instructions::hlt(); // sleep until the next interrupt
    }
    serial_println!("[ ok ] M3 complete — timer ticking: {} ticks", interrupts::ticks());

    // M5: PS/2 keyboard input.
    interrupts::init_keyboard();
    serial_println!("[ ok ] keyboard IRQ1 handler registered");

    // Quick smoke test: type a line, echo it back.
    serial_print!("\n> ");
    let line = keyboard::read_line();
    serial_println!();
    serial_println!("[ ok ] you typed: {}", line);
    serial_println!("[ ok ] M5 complete — keyboard working");

    // M4: render the animated acid build-bubbles, with live keyboard echo.
    serial_println!("[ ok ] M4 rendering acid build-bubbles  [* * *]");
    match boot_info.framebuffer.as_mut() {
        Some(fb) => render::run(fb),
        None => {
            serial_println!("[!!] no framebuffer to render into");
            hcf();
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("[!!] panic: {}", info);
    hcf();
}

/// Halt and catch fire: park the CPU forever.
pub(crate) fn hcf() -> ! {
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

use bootloader_api::info::MemoryRegions;
