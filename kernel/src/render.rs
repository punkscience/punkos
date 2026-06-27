//! Software renderer for the Punk Science "build" bubbles.
//!
//! Three acid-green circles on Hardcore Black. The framebuffer has no alpha, so
//! brand opacity (100 / 60 / 30 %) is precomputed by blending the acid onto the
//! background. A loading animation walks the full-opacity "leader" across the
//! three dots — the mark is alive.
//!
//! We render into an off-screen back buffer and then copy to the real
//! framebuffer in one pass (double buffering), so the display never shows a
//! half-drawn frame — no tearing.

use alloc::vec;
use alloc::vec::Vec;

use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};

use crate::framebuffer::{color, write_pixel};
use crate::interrupts;

/// Opacity steps for the 100 / 60 / 30 % brand stagger, as 0..=255.
const STAGGER: [u16; 3] = [255, 153, 76];

/// Blend a foreground colour onto a background at `opacity` (0..=255).
fn blend(fg: (u8, u8, u8), bg: (u8, u8, u8), opacity: u16) -> (u8, u8, u8) {
    let mix = |a: u8, b: u8| ((a as u16 * opacity + b as u16 * (255 - opacity)) / 255) as u8;
    (mix(fg.0, bg.0), mix(fg.1, bg.1), mix(fg.2, bg.2))
}

/// An off-screen drawing surface matching the framebuffer's layout.
struct Canvas {
    buf: Vec<u8>,
    w: usize,
    h: usize,
    stride: usize,
    bpp: usize,
    fmt: PixelFormat,
}

impl Canvas {
    fn new(info: &FrameBufferInfo) -> Self {
        Canvas {
            buf: vec![0u8; info.byte_len],
            w: info.width,
            h: info.height,
            stride: info.stride,
            bpp: info.bytes_per_pixel,
            fmt: info.pixel_format,
        }
    }

    fn clear(&mut self, rgb: (u8, u8, u8)) {
        let (fmt, bpp) = (self.fmt, self.bpp);
        for px in self.buf.chunks_exact_mut(bpp) {
            write_pixel(px, fmt, rgb);
        }
    }

    fn fill_circle(&mut self, cx: isize, cy: isize, r: isize, rgb: (u8, u8, u8)) {
        let (w, h) = (self.w as isize, self.h as isize);
        let (stride, bpp, fmt) = (self.stride, self.bpp, self.fmt);
        let r2 = r * r;
        let y0 = (cy - r).max(0);
        let y1 = (cy + r).min(h - 1);
        for y in y0..=y1 {
            let dy = y - cy;
            let x0 = (cx - r).max(0);
            let x1 = (cx + r).min(w - 1);
            for x in x0..=x1 {
                let dx = x - cx;
                if dx * dx + dy * dy <= r2 {
                    let off = (y as usize * stride + x as usize) * bpp;
                    write_pixel(&mut self.buf[off..off + bpp], fmt, rgb);
                }
            }
        }
    }

    /// Copy rows `y0..=y1` of the back buffer to the framebuffer in one pass.
    fn present_rows(&self, fb: &mut FrameBuffer, y0: usize, y1: usize) {
        let row_bytes = self.stride * self.bpp;
        let start = y0 * row_bytes;
        let end = (y1 + 1) * row_bytes;
        fb.buffer_mut()[start..end].copy_from_slice(&self.buf[start..end]);
    }
}

/// Geometry of the three-bubble mark.
struct Layout {
    centers: [isize; 3],
    cy: isize,
    r: isize,
}

fn layout(info: &FrameBufferInfo) -> Layout {
    let (w, h) = (info.width as isize, info.height as isize);
    let r = h / 12; // bubble radius relative to screen height
    let gap = (r * 13) / 5; // ~2.6 r between centres (brand: lightly spaced)
    let cx = w / 2;
    let cy = h / 2;
    Layout {
        centers: [cx - gap, cx, cx + gap],
        cy,
        r,
    }
}

/// Draw the three bubbles into the canvas with the leader at index `lead`.
fn draw(canvas: &mut Canvas, l: &Layout, lead: usize) {
    for (i, &bx) in l.centers.iter().enumerate() {
        let step = (3 + i - lead) % 3;
        let rgb = blend(color::BUILD_ACID, color::HARDCORE_BLACK, STAGGER[step]);
        canvas.fill_circle(bx, l.cy, l.r, rgb);
    }
}

/// Run the loading animation forever: the leader walks left→right.
pub fn run(fb: &mut FrameBuffer) -> ! {
    let info = fb.info();
    let mut canvas = Canvas::new(&info);
    canvas.clear(color::HARDCORE_BLACK);
    let l = layout(&info);

    // Rows the bubbles occupy — only these get copied each frame.
    let band0 = (l.cy - l.r).max(0) as usize;
    let band1 = (l.cy + l.r).min(info.height as isize - 1) as usize;

    let mut last_frame = u64::MAX;
    loop {
        // Echo any keyboard input to serial.
        while let Some(c) = crate::keyboard::read_char() {
            crate::serial_print!("{}", c as char);
        }

        // Advance one step roughly every 120 ms (12 ticks at 100 Hz).
        let frame = interrupts::ticks() / 12;
        if frame != last_frame {
            last_frame = frame;
            let lead = (frame % 3) as usize;
            draw(&mut canvas, &l, lead);
            canvas.present_rows(fb, band0, band1);
        }
        x86_64::instructions::hlt();
    }
}
