//! PCI configuration space access and bus enumeration (M6a).
//!
//! Uses legacy Configuration Mechanism #1 (I/O ports 0xCF8 / 0xCFC) to scan the
//! PCI bus for devices. The immediate goal is to locate an xHCI USB host
//! controller (class 0x0C, subclass 0x03, prog-if 0x30) and read its MMIO base
//! address so the xHCI driver (M6b) can take over.
//!
//! # Design
//!
//! - `PciConfig` wraps the raw port I/O, exposing typed register reads/writes
//!   without the caller handling address encoding.
//! - `Device` is a plain data struct — the scan produces a `Vec<Device>`.
//! - `Bar` decodes the three BAR types (memory-32, memory-64, I/O).

use alloc::vec::Vec;
use x86_64::instructions::port::{PortWriteOnly, PortReadOnly};

// --- port I/O ---------------------------------------------------------------

const CONFIG_ADDRESS: u16 = 0xCF8;
const CONFIG_DATA: u16 = 0xCFC;

/// A typed handle on the PCI configuration space of a specific device/function.
#[derive(Clone, Copy)]
pub struct PciConfig {
    bus: u8,
    device: u8,
    function: u8,
}

impl PciConfig {
    /// Construct a config-space handle. No I/O is performed until a read/write.
    pub const fn new(bus: u8, device: u8, function: u8) -> Self {
        Self { bus, device, function }
    }

    /// Read a 32-bit word from `offset` (must be 4-byte aligned).
    pub fn read32(&self, offset: u8) -> u32 {
        assert!(offset & 0x3 == 0, "PCI config offset must be aligned to 4 bytes");
        let addr = config_address(self.bus, self.device, self.function, offset);
        unsafe {
            let mut port: PortWriteOnly<u32> = PortWriteOnly::new(CONFIG_ADDRESS);
            port.write(addr);
            let mut data: PortReadOnly<u32> = PortReadOnly::new(CONFIG_DATA);
            data.read()
        }
    }

    /// Write a 32-bit word to `offset` (must be 4-byte aligned).
    #[allow(dead_code)] // will be used by xHCI init (M6b)
    pub fn write32(&self, offset: u8, value: u32) {
        assert!(offset & 0x3 == 0, "PCI config offset must be aligned to 4 bytes");
        let addr = config_address(self.bus, self.device, self.function, offset);
        unsafe {
            let mut port: PortWriteOnly<u32> = PortWriteOnly::new(CONFIG_ADDRESS);
            port.write(addr);
            let mut data: PortWriteOnly<u32> = PortWriteOnly::new(CONFIG_DATA);
            data.write(value);
        }
    }

    /// Convenience: read vendor ID (offset 0x00, low 16 bits).
    pub fn vendor_id(&self) -> u16 {
        (self.read32(0x00) & 0xFFFF) as u16
    }

    /// Convenience: read device ID (offset 0x00, high 16 bits).
    pub fn device_id(&self) -> u16 {
        ((self.read32(0x00) >> 16) & 0xFFFF) as u16
    }

    /// Convenience: read header type (offset 0x0C, bits 16–23 of dword at 0x0C).
    pub fn header_type(&self) -> u8 {
        ((self.read32(0x0C) >> 16) & 0xFF) as u8
    }

    /// Read class code tuple: (class, subclass, prog-if), from offset 0x08.
    pub fn class_info(&self) -> (u8, u8, u8) {
        let dword = self.read32(0x08);
        let class = ((dword >> 24) & 0xFF) as u8;
        let subclass = ((dword >> 16) & 0xFF) as u8;
        let prog_if = ((dword >> 8) & 0xFF) as u8;
        (class, subclass, prog_if)
    }

    /// Read a single BAR at `bar_index` (0–5).  Offset = 0x10 + index * 4.
    pub fn read_bar(&self, bar_index: u8) -> Bar {
        let offset = 0x10 + bar_index as u8 * 4;
        let raw = self.read32(offset);

        // Bit 0 = 1 → I/O space; = 0 → memory space.
        if raw & 0x1 == 1 {
            Bar::Io { port: (raw & 0xFFFFFFFC) as u16 }
        } else {
            let ty = (raw >> 1) & 0x3;
            let prefetchable = (raw >> 3) & 0x1 == 1;
            let addr32 = raw & 0xFFFFFFF0;
            match ty {
                0x0 => Bar::Memory32 {
                    base: addr32,
                    prefetchable,
                },
                0x2 => {
                    // 64-bit memory BAR: read the upper 32 bits from the next register.
                    let high = self.read32(offset + 4);
                    let base64 = addr32 as u64 | ((high as u64) << 32);
                    Bar::Memory64 {
                        base: base64,
                        prefetchable,
                    }
                }
                _ => Bar::Unknown { raw },
            }
        }
    }
}

/// Decoded PCI Base Address Register.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Bar {
    /// I/O port BAR.
    Io { port: u16 },
    /// 32-bit memory-mapped BAR.
    Memory32 { base: u32, prefetchable: bool },
    /// 64-bit memory-mapped BAR (two consecutive 32-bit registers).
    Memory64 { base: u64, prefetchable: bool },
    /// Unrecognised BAR type.
    Unknown { raw: u32 },
}

impl Bar {
    /// Physical base address for a memory BAR, or `None` for I/O / unknown.
    pub fn memory_base(&self) -> Option<u64> {
        match self {
            Bar::Memory32 { base, .. } => Some(*base as u64),
            Bar::Memory64 { base, .. } => Some(*base),
            _ => None,
        }
    }
}

/// A discovered PCI device.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Device {
    pub bus: u8,
    pub device: u8,
    pub function: u8,
    pub vendor_id: u16,
    pub device_id: u16,
    pub class: u8,
    pub subclass: u8,
    pub prog_if: u8,
    pub header_type: u8,
    pub bars: [Option<Bar>; 6],
}

// --- helpers ----------------------------------------------------------------

/// Build the 32-bit CONFIG_ADDRESS value for Configuration Mechanism #1.
fn config_address(bus: u8, device: u8, function: u8, offset: u8) -> u32 {
    0x8000_0000
        | ((bus as u32) << 16)
        | (((device & 0x1F) as u32) << 11)
        | (((function & 0x7) as u32) << 8)
        | (offset as u32 & 0xFC)
}

/// Scan all PCI buses and return every device found (non-0xFFFF vendor).
///
/// Recursively descends into multi-function devices (header type bit 7 set).
pub fn enumerate() -> Vec<Device> {
    let mut devices = Vec::new();
    for bus in 0..=255u8 {
        for device in 0..32u8 {
            let cfg = PciConfig::new(bus, device, 0);
            let vid = cfg.vendor_id();
            if vid == 0xFFFF {
                continue;
            }
            let header_type = cfg.header_type();
            let (class, subclass, prog_if) = cfg.class_info();
            let did = cfg.device_id();
            let mut device_info = Device {
                bus,
                device,
                function: 0,
                vendor_id: vid,
                device_id: did,
                class,
                subclass,
                prog_if,
                header_type,
                bars: [None; 6],
            };
            // Read BARs, skipping the upper-half register of 64-bit memory BARs.
            let mut bar_idx = 0u8;
            while bar_idx < 6 {
                let bar = cfg.read_bar(bar_idx);
                let is_64bit = matches!(bar, Bar::Memory64 { .. });
                device_info.bars[bar_idx as usize] = Some(bar);
                bar_idx += 1;
                if is_64bit {
                    bar_idx += 1; // upper 32 bits consumed by the 64-bit BAR
                }
            }
            devices.push(device_info);

            // Multi-function device?
            if header_type & 0x80 != 0 {
                for function in 1..8u8 {
                    let cfg = PciConfig::new(bus, device, function);
                    let vid = cfg.vendor_id();
                    if vid == 0xFFFF {
                        continue;
                    }
                    let (class, subclass, prog_if) = cfg.class_info();
                    let mut dev = Device {
                        bus,
                        device,
                        function,
                        vendor_id: vid,
                        device_id: cfg.device_id(),
                        class,
                        subclass,
                        prog_if,
                        header_type: cfg.header_type(),
                        bars: [None; 6],
                    };
                    let mut bar_idx = 0u8;
                    while bar_idx < 6 {
                        let bar = cfg.read_bar(bar_idx);
                        let is_64bit = matches!(bar, Bar::Memory64 { .. });
                        dev.bars[bar_idx as usize] = Some(bar);
                        bar_idx += 1;
                        if is_64bit {
                            bar_idx += 1;
                        }
                    }
                    devices.push(dev);
                }
            }
        }
    }
    devices
}

/// Filter `devices` for xHCI controllers: class 0x0C, subclass 0x03, prog-if 0x30.
pub fn find_xhci(devices: &[Device]) -> Vec<&Device> {
    devices
        .iter()
        .filter(|d| d.class == 0x0C && d.subclass == 0x03 && d.prog_if == 0x30)
        .collect()
}

// --- human-readable helpers (for serial logging) ---------------------------

/// Return a short string for the PCI class code.
pub fn class_name(class: u8, subclass: u8) -> &'static str {
    match (class, subclass) {
        (0x00, 0x00) => "Non-VGA",
        (0x00, 0x01) => "VGA Compatible",
        (0x01, 0x00) => "SCSI",
        (0x01, 0x01) => "IDE",
        (0x01, 0x06) => "SATA",
        (0x01, 0x07) => "SAS",
        (0x02, 0x00) => "Ethernet",
        (0x03, 0x00) => "VGA",
        (0x04, _) => "Multimedia",
        (0x06, 0x00) => "Host Bridge",
        (0x06, 0x01) => "ISA Bridge",
        (0x06, 0x04) => "PCI-to-PCI Bridge",
        (0x06, 0x07) => "CardBus Bridge",
        (0x0C, 0x03) => "USB Host Controller",
        (0x0C, 0x05) => "SMBus",
        (0x0D, 0x00) => "IRQ Controller",
        _ => "Unknown",
    }
}