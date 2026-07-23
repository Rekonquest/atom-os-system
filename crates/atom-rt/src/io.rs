use crate::sys;

pub fn print(s: &str) {
    print_bytes(s.as_bytes());
}

pub fn print_bytes(bytes: &[u8]) {
    for &b in bytes {
        sys::write_byte(b);
    }
}

pub fn print_u64(n: u64) {
    let mut buf = [0u8; 20];
    let len = crate::fmt::fmt_u64(n, &mut buf);
    print_bytes(&buf[..len]);
}

pub fn print_fixed6(x: f64) {
    let mut buf = [0u8; 32];
    let len = crate::fmt::fmt_fixed6(x, &mut buf);
    print_bytes(&buf[..len]);
}

pub fn read_line(buf: &mut [u8]) -> usize {
    let mut len = 0usize;
    loop {
        match sys::read_char() {
            Some(b'\n') => {
                sys::write_byte(b'\n');
                return len;
            }
            Some(0x08) => {
                if len > 0 {
                    len -= 1;
                    sys::write_byte(0x08);
                }
            }
            Some(c) => {
                if len < buf.len() {
                    buf[len] = c;
                    len += 1;
                    sys::write_byte(c);
                }
            }
            None => sys::yield_now(),
        }
    }
}
