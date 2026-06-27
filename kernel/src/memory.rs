//! Paging + physical frame allocation.
//!
//! The bootloader maps all physical memory at a fixed virtual offset (we request
//! this in the BootloaderConfig). That lets us read/write the active page tables
//! to map new pages, and hand out usable physical frames from the memory map.

use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, OffsetPageTable, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

/// Initialise an `OffsetPageTable` over the active level-4 table.
///
/// # Safety
/// `physical_memory_offset` must be the base at which the bootloader mapped all
/// physical memory, and this must be called only once.
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    &mut *page_table_ptr
}

/// Hands out usable 4 KiB frames straight from the bootloader memory map.
pub struct BootInfoFrameAllocator {
    memory_regions: &'static MemoryRegions,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// # Safety
    /// All frames marked `Usable` in `memory_regions` must really be unused.
    pub unsafe fn init(memory_regions: &'static MemoryRegions) -> Self {
        Self {
            memory_regions,
            next: 0,
        }
    }

    /// Skip frames until we pass `min_addr` — avoids giving out frames
    /// in low conventional memory or the bootloader's footprint.
    pub fn skip_below(&mut self, min_addr: u64) {
        let skip_count = self
            .usable_frames()
            .take_while(|f| f.start_address().as_u64() < min_addr)
            .count();
        self.next = skip_count;
    }

    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> + '_ {
        self.memory_regions
            .iter()
            .filter(|r| r.kind == MemoryRegionKind::Usable)
            .map(|r| r.start..r.end)
            .flat_map(|range| range.step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}

/// Walk the active page tables to translate a virtual address to a physical
/// address.  Uses `phys_offset` to reach physical page-table frames.
///
/// # Safety
/// `phys_offset` must be the identity-map base for all physical memory.
pub unsafe fn virt_to_phys(virt: VirtAddr, phys_offset: VirtAddr) -> Option<u64> {
    let (p4_frame, _) = Cr3::read();

    // Helper: dereference a physical page-table frame viewed through phys_offset.
    let pt_at = |frame: PhysFrame| -> *const PageTable {
        (phys_offset + frame.start_address().as_u64()).as_ptr::<PageTable>()
    };

    let p4: &PageTable = unsafe { &*pt_at(p4_frame) };
    let p4e = &p4[virt.p4_index()];
    if !p4e.flags().contains(PageTableFlags::PRESENT) {
        return None;
    }

    let p3: &PageTable = unsafe { &*pt_at(p4e.frame().ok()?) };
    let p3e = &p3[virt.p3_index()];
    if !p3e.flags().contains(PageTableFlags::PRESENT) {
        return None;
    }
    if p3e.flags().contains(PageTableFlags::HUGE_PAGE) {
        return Some(p3e.addr().as_u64() + (virt.as_u64() & 0x3FFF_FFFF));
    }

    let p2: &PageTable = unsafe { &*pt_at(p3e.frame().ok()?) };
    let p2e = &p2[virt.p2_index()];
    if !p2e.flags().contains(PageTableFlags::PRESENT) {
        return None;
    }
    if p2e.flags().contains(PageTableFlags::HUGE_PAGE) {
        return Some(p2e.addr().as_u64() + (virt.as_u64() & 0x1F_FFFF));
    }

    let p1: &PageTable = unsafe { &*pt_at(p2e.frame().ok()?) };
    let p1e = &p1[virt.p1_index()];
    if !p1e.flags().contains(PageTableFlags::PRESENT) {
        return None;
    }

    Some(p1e.addr().as_u64() + (virt.as_u64() & 0xFFF))
}
