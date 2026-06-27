//! PS/2 keyboard driver.
//!
//! Reads scancodes from I/O port 0x60 in the IRQ1 handler, converts them to
//! ASCII via a US QWERTY map with shift awareness, and buffers characters in
//! a lock-free ring buffer. M5 — first interactive input milestone.

use core::sync::atomic::{AtomicBool, Ordering};

use spin::Mutex;
use x86_64::instructions::port::Port;

/// Ring buffer for keystrokes.
const BUF_SIZE: usize = 256;

struct RingBuffer<const N: usize> {
    data: [char; N],
    head: usize,
    tail: usize,
}

impl<const N: usize> RingBuffer<N> {
    const fn new() -> Self {
        Self {
            data: ['\0'; N],
            head: 0,
            tail: 0,
        }
    }

    fn push(&mut self, c: char) {
        let next = (self.head + 1) % N;
        // Drop the oldest if full — better than panicking in an ISR.
        if next == self.tail {
            return;
        }
        self.data[self.head] = c;
        self.head = next;
    }

    fn pop(&mut self) -> Option<char> {
        if self.head == self.tail {
            None
        } else {
            let c = self.data[self.tail];
            self.tail = (self.tail + 1) % N;
            Some(c)
        }
    }
}

static BUF: Mutex<RingBuffer<BUF_SIZE>> = Mutex::new(RingBuffer::new());
static SHIFT: AtomicBool = AtomicBool::new(false);

/// Called from the keyboard ISR. Reads the scancode from port 0x60 and pushes
/// the corresponding character into the buffer (if any).
pub fn handle_scancode() {
    let scancode: u8 = unsafe {
        let mut port: Port<u8> = Port::new(0x60);
        port.read()
    };

    // Track shift state.
    match scancode {
        0x2A | 0x36 => {
            SHIFT.store(true, Ordering::Relaxed);
            return;
        }
        0xAA | 0xB6 => {
            SHIFT.store(false, Ordering::Relaxed);
            return;
        }
        _ => {}
    }

    // Ignore break codes (scancode with bit 7 set).
    if scancode & 0x80 != 0 {
        return;
    }

    let shifted = SHIFT.load(Ordering::Relaxed);
    if let Some(c) = scancode_to_char(scancode, shifted) {
        BUF.lock().push(c);
    }
}

/// Non-blocking: returns the next buffered keystroke, or `None`.
pub fn read_char() -> Option<char> {
    BUF.lock().pop()
}

/// Blocking: waits (hlt) until a character is available.
pub fn read_char_blocking() -> char {
    loop {
        if let Some(c) = read_char() {
            return c;
        }
        x86_64::instructions::hlt();
    }
}

/// Blocking: reads characters until a newline is returned, returns the line
/// (without the newline). Backspace erases the last character.
pub fn read_line() -> alloc::string::String {
    let mut line = alloc::string::String::new();
    loop {
        match read_char_blocking() {
            '\n' => return line,
            '\x08' => {
                line.pop();
            }
            c => {
                line.push(c);
            }
        }
    }
}

// --- scancode set 1 → ASCII (US QWERTY) -------------------------------------

fn scancode_to_char(scancode: u8, shifted: bool) -> Option<char> {
    match scancode {
        0x01 => Some('\x1b'), // escape
        0x0E => Some('\x08'), // backspace
        0x0F => Some('\t'),
        0x1C => Some('\n'),   // enter
        0x39 => Some(' '),

        // Number row
        0x02 => Some(if shifted { '!' } else { '1' }),
        0x03 => Some(if shifted { '@' } else { '2' }),
        0x04 => Some(if shifted { '#' } else { '3' }),
        0x05 => Some(if shifted { '$' } else { '4' }),
        0x06 => Some(if shifted { '%' } else { '5' }),
        0x07 => Some(if shifted { '^' } else { '6' }),
        0x08 => Some(if shifted { '&' } else { '7' }),
        0x09 => Some(if shifted { '*' } else { '8' }),
        0x0A => Some(if shifted { '(' } else { '9' }),
        0x0B => Some(if shifted { ')' } else { '0' }),
        0x0C => Some(if shifted { '_' } else { '-' }),
        0x0D => Some(if shifted { '+' } else { '=' }),

        // Letter block
        0x10 => Some(if shifted { 'Q' } else { 'q' }),
        0x11 => Some(if shifted { 'W' } else { 'w' }),
        0x12 => Some(if shifted { 'E' } else { 'e' }),
        0x13 => Some(if shifted { 'R' } else { 'r' }),
        0x14 => Some(if shifted { 'T' } else { 't' }),
        0x15 => Some(if shifted { 'Y' } else { 'y' }),
        0x16 => Some(if shifted { 'U' } else { 'u' }),
        0x17 => Some(if shifted { 'I' } else { 'i' }),
        0x18 => Some(if shifted { 'O' } else { 'o' }),
        0x19 => Some(if shifted { 'P' } else { 'p' }),
        0x1A => Some(if shifted { '{' } else { '[' }),
        0x1B => Some(if shifted { '}' } else { ']' }),
        0x1E => Some(if shifted { 'A' } else { 'a' }),
        0x1F => Some(if shifted { 'S' } else { 's' }),
        0x20 => Some(if shifted { 'D' } else { 'd' }),
        0x21 => Some(if shifted { 'F' } else { 'f' }),
        0x22 => Some(if shifted { 'G' } else { 'g' }),
        0x23 => Some(if shifted { 'H' } else { 'h' }),
        0x24 => Some(if shifted { 'J' } else { 'j' }),
        0x25 => Some(if shifted { 'K' } else { 'k' }),
        0x26 => Some(if shifted { 'L' } else { 'l' }),
        0x27 => Some(if shifted { ':' } else { ';' }),
        0x28 => Some(if shifted { '"' } else { '\'' }),
        0x2B => Some(if shifted { '|' } else { '\\' }),
        0x2C => Some(if shifted { 'Z' } else { 'z' }),
        0x2D => Some(if shifted { 'X' } else { 'x' }),
        0x2E => Some(if shifted { 'C' } else { 'c' }),
        0x2F => Some(if shifted { 'V' } else { 'v' }),
        0x30 => Some(if shifted { 'B' } else { 'b' }),
        0x31 => Some(if shifted { 'N' } else { 'n' }),
        0x32 => Some(if shifted { 'M' } else { 'm' }),
        0x33 => Some(if shifted { '<' } else { ',' }),
        0x34 => Some(if shifted { '>' } else { '.' }),
        0x35 => Some(if shifted { '?' } else { '/' }),

        _ => None,
    }
}
