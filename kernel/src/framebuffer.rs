//! Direct linear-framebuffer access.
//!
//! The bootloader hands us a mapped framebuffer (an array of pixels in RAM). We
//! write RGB values straight into it — the foundation of the software renderer.
//! The framebuffer has no alpha channel, so brand "opacity" is precomputed by
//! blending onto the Hardcore Black background elsewhere.

use bootloader_api::info::{FrameBuffer, PixelFormat};

/// Punk Science palette (sRGB).
pub mod color {
    pub const HARDCORE_BLACK: (u8, u8, u8) = (0x0E, 0x0E, 0x10);
    pub const NOVA_WHITE: (u8, u8, u8) = (0xF5, 0xF1, 0xEA);
    pub const BUILD_ACID: (u8, u8, u8) = (0xD6, 0xFF, 0x3F);
    pub const HALIFAX_COBALT: (u8, u8, u8) = (0x3A, 0x6B, 0xFF);
}

/// Fill the entire framebuffer with a single colour.
pub fn clear(fb: &mut FrameBuffer, rgb: (u8, u8, u8)) {
    let info = fb.info();
    let bpp = info.bytes_per_pixel;
    let fmt = info.pixel_format;
    let buf = fb.buffer_mut();
    for px in buf.chunks_exact_mut(bpp) {
        write_pixel(px, fmt, rgb);
    }
}

/// Write one pixel honouring the framebuffer's byte order.
#[inline]
pub fn write_pixel(px: &mut [u8], fmt: PixelFormat, (r, g, b): (u8, u8, u8)) {
    match fmt {
        PixelFormat::Rgb => {
            px[0] = r;
            px[1] = g;
            px[2] = b;
        }
        PixelFormat::Bgr => {
            px[0] = b;
            px[1] = g;
            px[2] = r;
        }
        PixelFormat::U8 => {
            // Grayscale luminance approximation.
            px[0] = ((r as u16 * 54 + g as u16 * 183 + b as u16 * 19) >> 8) as u8;
        }
        // PixelFormat is #[non_exhaustive]; default to BGR (most common in QEMU).
        _ => {
            px[0] = b;
            px[1] = g;
            px[2] = r;
        }
    }
}
