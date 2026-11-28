//! Platform layer: terminal lifecycle, raw key events, and console output.
//!
//! Zero dependencies. On Windows this drives the console through the Win32 API;
//! on Unix it shells out to `stty` and parses ANSI key sequences.
//!
//! Why hand-rolled: the console gives us true key *up* records on Windows, which
//! is what makes "both players hold a key at the same time" feel exact.

use std::io::{self, Write};

use conduit::keys::{Key, KeyEv, Mods};

pub fn enter() -> io::Result<()> {
    imp::enter()
}

pub fn leave() {
    imp::leave()
}

/// Current terminal size in cells (columns, rows).
pub fn size() -> (i32, i32) {
    imp::size()
}

/// Wait up to `timeout_ms` for input, then drain everything pending.
pub fn poll_events(timeout_ms: u32, out: &mut Vec<KeyEv>) -> io::Result<()> {
    imp::poll_events(timeout_ms, out)
}

/// A `Write` sink that renders UTF-8 to the console.
pub struct Console {
    buf: Vec<u8>,
}

impl Console {
    pub fn new() -> Self {
        Self {
            buf: Vec::with_capacity(1 << 16),
        }
    }
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for Console {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        imp::write_bytes(&self.buf)?;
        self.buf.clear();
        Ok(())
    }
}

// ---------------------------------------------------------------- windows

#[cfg(windows)]
mod imp {
    use super::*;
    use std::collections::VecDeque;
    use std::ffi::c_void;
    use std::sync::{Mutex, OnceLock};

    type Handle = *mut c_void;
    type Bool = i32;
    type Dword = u32;

    const STD_INPUT: Dword = -10i32 as Dword;
    const STD_OUTPUT: Dword = -11i32 as Dword;

    const IN_PROCESSED: Dword = 0x0001;
    const IN_LINE: Dword = 0x0002;
    const IN_ECHO: Dword = 0x0004;
    const IN_WINDOW: Dword = 0x0008;
    const IN_QUICK_EDIT: Dword = 0x0040;
    const IN_EXTENDED: Dword = 0x0080;

    const OUT_PROCESSED: Dword = 0x0001;
    const OUT_VT: Dword = 0x0004;

    const KEY_EVENT: u16 = 0x0001;
    const WINDOW_EVENT: u16 = 0x0004;
    const FOCUS_EVENT: u16 = 0x0010;

    const CTRL_RIGHT_ALT: Dword = 0x0001;
    const CTRL_LEFT_ALT: Dword = 0x0002;
    const CTRL_RIGHT: Dword = 0x0004;
    const CTRL_LEFT: Dword = 0x0008;
    const CTRL_SHIFT: Dword = 0x0010;

    const VK_BACK: u16 = 0x08;
    const VK_TAB: u16 = 0x09;
    const VK_RETURN: u16 = 0x0D;
    const VK_ESCAPE: u16 = 0x1B;
    const VK_SPACE: u16 = 0x20;
    const VK_LEFT: u16 = 0x25;
    const VK_UP: u16 = 0x26;
    const VK_RIGHT: u16 = 0x27;
    const VK_DOWN: u16 = 0x28;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Csbi {
        size: Coord,
        cursor: Coord,
        attributes: u16,
        window: SmallRect,
        max_size: Coord,
    }

    /// `INPUT_RECORD` is 20 bytes: a WORD tag, two bytes of padding, then a
    /// 16-byte union. Fields are read via explicit offsets (see `parse`).
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct InputRecord {
        event_type: u16,
        _pad: u16,
        data: [u32; 4],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: Dword) -> Handle;
        fn GetConsoleMode(h: Handle, mode: *mut Dword) -> Bool;
        fn SetConsoleMode(h: Handle, mode: Dword) -> Bool;
        fn GetConsoleScreenBufferInfo(h: Handle, info: *mut Csbi) -> Bool;
        fn ReadConsoleInputW(h: Handle, buf: *mut InputRecord, len: Dword, read: *mut Dword) -> Bool;
        fn GetNumberOfConsoleInputEvents(h: Handle, count: *mut Dword) -> Bool;
        fn WriteConsoleW(h: Handle, buf: *const u16, len: Dword, written: *mut Dword, r: *mut c_void) -> Bool;
        fn WaitForSingleObject(h: Handle, ms: Dword) -> Dword;
        fn SetConsoleOutputCP(cp: Dword) -> Bool;
        fn MessageBeep(kind: Dword) -> Bool;
    }

    static SAVED: Mutex<Option<(Dword, Dword)>> = Mutex::new(None);
    static PENDING: OnceLock<Mutex<VecDeque<KeyEv>>> = OnceLock::new();

    fn pending() -> &'static Mutex<VecDeque<KeyEv>> {
        PENDING.get_or_init(|| Mutex::new(VecDeque::new()))
    }

    fn stdin() -> Handle {
        unsafe { GetStdHandle(STD_INPUT) }
    }

    fn stdout_handle() -> Handle {
        unsafe { GetStdHandle(STD_OUTPUT) }
    }

    pub fn enter() -> io::Result<()> {
        unsafe {
            let hin = stdin();
            let hout = stdout_handle();
            let mut im: Dword = 0;
            let mut om: Dword = 0;
            if GetConsoleMode(hin, &mut im) == 0 || GetConsoleMode(hout, &mut om) == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "CONDUIT needs a real console (stdin/stdout are redirected)",
                ));
            }
            if let Ok(mut g) = SAVED.lock() {
                *g = Some((im, om));
            }
            // Raw keys: no line buffering, no echo, no ctrl-c translation,
            // no quick-edit selection stealing the console.
            let raw_in = (im & !(IN_LINE | IN_ECHO | IN_PROCESSED | IN_QUICK_EDIT))
                | IN_EXTENDED
                | IN_WINDOW;
            SetConsoleMode(hin, raw_in);
            SetConsoleMode(hout, om | OUT_PROCESSED | OUT_VT);
            SetConsoleOutputCP(65001);

            let boot = "\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H\x1b]0;CONDUIT\x07";
            write_bytes(boot.as_bytes())?;
        }
        Ok(())
    }

    pub fn leave() {
        let _ = write_bytes(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        if let Ok(g) = SAVED.lock() {
            if let Some((im, om)) = *g {
                unsafe {
                    SetConsoleMode(stdin(), im);
                    SetConsoleMode(stdout_handle(), om);
                }
            }
        }
    }

    pub fn size() -> (i32, i32) {
        unsafe {
            let mut info: Csbi = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(stdout_handle(), &mut info) != 0 {
                let w = (info.window.right - info.window.left + 1) as i32;
                let h = (info.window.bottom - info.window.top + 1) as i32;
                if w > 0 && h > 0 {
                    return (w, h);
                }
            }
        }
        (100, 32)
    }

    pub fn poll_events(timeout_ms: u32, out: &mut Vec<KeyEv>) -> io::Result<()> {
        // Anything left from a previous partial read.
        if let Ok(mut q) = pending().lock() {
            while let Some(ev) = q.pop_front() {
                out.push(ev);
            }
        }

        let h = stdin();
        unsafe {
            // The wait *is* the frame pacing: it returns the instant a key lands.
            WaitForSingleObject(h, timeout_ms.max(1));

            loop {
                let mut count: Dword = 0;
                if GetNumberOfConsoleInputEvents(h, &mut count) == 0 || count == 0 {
                    break;
                }
                let take = count.min(64) as Dword;
                let mut buf: [InputRecord; 64] = std::mem::zeroed();
                let mut read: Dword = 0;
                if ReadConsoleInputW(h, buf.as_mut_ptr(), take, &mut read) == 0 || read == 0 {
                    break;
                }
                for rec in buf.iter().take(read as usize) {
                    parse(rec, out);
                }
                if (read as Dword) < take {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Decode one `INPUT_RECORD`.
    ///
    /// KEY_EVENT_RECORD starts at byte 4 of the record:
    ///   +0  bKeyDown (BOOL)
    ///   +4  wRepeatCount (WORD)
    ///   +6  wVirtualKeyCode (WORD)
    ///   +8  wVirtualScanCode (WORD)
    ///   +10 uChar (WORD)
    ///   +12 dwControlKeyState (DWORD)
    fn parse(rec: &InputRecord, out: &mut Vec<KeyEv>) {
        match rec.event_type {
            KEY_EVENT => {
                let down = rec.data[0] != 0;
                let repeat = (rec.data[1] & 0xFFFF) as u32;
                let vk = (rec.data[1] >> 16) as u16;
                let uchar = (rec.data[2] >> 16) as u16;
                let ctrl = rec.data[3];
                let mods = Mods {
                    ctrl: ctrl & (CTRL_LEFT | CTRL_RIGHT) != 0,
                    alt: ctrl & (CTRL_LEFT_ALT | CTRL_RIGHT_ALT) != 0,
                    shift: ctrl & CTRL_SHIFT != 0,
                };
                let key = map_key(vk, uchar);
                if key == Key::Other {
                    return;
                }
                if down {
                    out.push(KeyEv::Press(key, mods));
                    // OS auto-repeat: extra strokes are refreshes, not edges.
                    for _ in 1..repeat.min(8) {
                        out.push(KeyEv::Repeat(key, mods));
                    }
                } else {
                    out.push(KeyEv::Release(key));
                }
            }
            WINDOW_EVENT => {
                let (w, h) = size();
                out.push(KeyEv::Resize(w, h));
            }
            FOCUS_EVENT => {
                if rec.data[0] != 0 {
                    out.push(KeyEv::FocusGained);
                } else {
                    out.push(KeyEv::FocusLost);
                }
            }
            _ => {}
        }
    }

    fn map_key(vk: u16, uchar: u16) -> Key {
        // Letters come from the typed character so the game follows the layout.
        let ch = char::from_u32(uchar as u32)
            .map(|c| c.to_ascii_lowercase())
            .unwrap_or('\0');
        match ch {
            'w' => return Key::W,
            'a' => return Key::A,
            's' => return Key::S,
            'd' => return Key::D,
            'p' => return Key::P,
            'r' => return Key::R,
            'q' => return Key::Q,
            'c' => return Key::C,
            '1' => return Key::Num1,
            '2' => return Key::Num2,
            '3' => return Key::Num3,
            '4' => return Key::Num4,
            ' ' => return Key::Space,
            _ => {}
        }
        match vk {
            VK_UP => Key::Up,
            VK_DOWN => Key::Down,
            VK_LEFT => Key::Left,
            VK_RIGHT => Key::Right,
            VK_RETURN => Key::Enter,
            VK_ESCAPE => Key::Esc,
            VK_TAB => Key::Tab,
            VK_SPACE => Key::Space,
            VK_BACK => Key::Other,
            _ => Key::Other,
        }
    }

    pub fn write_bytes(data: &[u8]) -> io::Result<()> {
        let text = String::from_utf8_lossy(data);
        let wide: Vec<u16> = text.encode_utf16().collect();
        let h = stdout_handle();
        let mut off = 0usize;
        while off < wide.len() {
            let chunk = (wide.len() - off).min(8192) as Dword;
            let mut written: Dword = 0;
            let ok = unsafe {
                WriteConsoleW(h, wide[off..].as_ptr(), chunk, &mut written, std::ptr::null_mut())
            };
            if ok == 0 || written == 0 {
                return Err(io::Error::last_os_error());
            }
            off += written as usize;
        }
        Ok(())
    }

    /// Short bell used for pickups; kept for future use.
    #[allow(dead_code)]
    pub fn beep() {
        unsafe {
            MessageBeep(0xFFFF_FFFF);
        }
    }
}

// ------------------------------------------------------------------- unix

#[cfg(unix)]
mod imp {
    use super::*;
    use std::process::Command;
    use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, TryRecvError};
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    static RX: OnceLock<Mutex<Receiver<u8>>> = OnceLock::new();
    static SIZE: OnceLock<Mutex<(i32, i32)>> = OnceLock::new();

    fn stty(args: &[&str]) {
        let _ = Command::new("stty").args(args).status();
    }

    pub fn enter() -> io::Result<()> {
        stty(&["-icanon", "-echo", "-isig", "-ixon", "min", "1", "time", "0"]);
        if RX.get().is_none() {
            let (tx, rx) = channel::<u8>();
            std::thread::spawn(move || {
                use std::io::Read;
                let mut stdin = std::io::stdin();
                let mut byte = [0u8; 1];
                while let Ok(1) = stdin.read(&mut byte) {
                    if tx.send(byte[0]).is_err() {
                        break;
                    }
                }
            });
            let _ = RX.set(Mutex::new(rx));
        }
        let (w, h) = size();
        let _ = SIZE.set(Mutex::new((w, h)));
        write_bytes(b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[H\x1b]0;CONDUIT\x07")
    }

    pub fn leave() {
        let _ = write_bytes(b"\x1b[0m\x1b[?25h\x1b[?1049l");
        stty(&["sane"]);
    }

    pub fn size() -> (i32, i32) {
        if let Ok(s) = std::env::var("CONDUIT_SIZE") {
            let mut it = s.split('x');
            if let (Some(a), Some(b)) = (it.next(), it.next()) {
                if let (Ok(w), Ok(h)) = (a.parse(), b.parse()) {
                    return (w, h);
                }
            }
        }
        if let Some(m) = SIZE.get() {
            if let Ok(g) = m.lock() {
                return *g;
            }
        }
        (100, 32)
    }

    pub fn poll_events(timeout_ms: u32, out: &mut Vec<KeyEv>) -> io::Result<()> {
        let Some(rx) = RX.get() else {
            return Ok(());
        };
        let Ok(guard) = rx.lock() else {
            return Ok(());
        };
        let mut bytes = Vec::with_capacity(16);
        match guard.recv_timeout(Duration::from_millis(timeout_ms.max(1) as u64)) {
            Ok(b) => bytes.push(b),
            Err(RecvTimeoutError::Timeout) => return Ok(()),
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
        loop {
            match guard.try_recv() {
                Ok(b) => bytes.push(b),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        parse(&bytes, out);
        Ok(())
    }

    /// ANSI input parsing. Unix terminals have no key-up, so every event is a
    /// press; `input::HOLD_MS` provides the release for us.
    fn parse(bytes: &[u8], out: &mut Vec<KeyEv>) {
        let mut i = 0;
        while i < bytes.len() {
            let b = bytes[i];
            if b == 0x1b && i + 2 < bytes.len() && (bytes[i + 1] == b'[' || bytes[i + 1] == b'O') {
                let k = match bytes[i + 2] {
                    b'A' => Key::Up,
                    b'B' => Key::Down,
                    b'C' => Key::Right,
                    b'D' => Key::Left,
                    b'Z' => Key::Tab,
                    _ => Key::Other,
                };
                i += 3;
                if k != Key::Other {
                    out.push(KeyEv::press(k));
                }
                continue;
            }
            i += 1;
            let key = match b {
                b'\r' | b'\n' => Some(Key::Enter),
                b' ' => Some(Key::Space),
                0x1b => Some(Key::Esc),
                0x03 => Some(Key::C),
                0x09 => Some(Key::Tab),
                b'w' | b'W' => Some(Key::W),
                b'a' | b'A' => Some(Key::A),
                b's' | b'S' => Some(Key::S),
                b'd' | b'D' => Some(Key::D),
                b'p' | b'P' => Some(Key::P),
                b'r' | b'R' => Some(Key::R),
                b'q' | b'Q' => Some(Key::Q),
                b'c' | b'C' => Some(Key::C),
                b'1' => Some(Key::Num1),
                b'2' => Some(Key::Num2),
                b'3' => Some(Key::Num3),
                b'4' => Some(Key::Num4),
                _ => None,
            };
            if let Some(k) = key {
                let mods = Mods {
                    ctrl: b == 0x03,
                    ..Mods::default()
                };
                out.push(KeyEv::Press(k, mods));
            }
        }
    }

    pub fn write_bytes(data: &[u8]) -> io::Result<()> {
        let mut out = std::io::stdout();
        out.write_all(data)?;
        out.flush()
    }
}
