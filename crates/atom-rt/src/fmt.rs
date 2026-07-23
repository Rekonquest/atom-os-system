pub fn fmt_u64(mut n: u64, buf: &mut [u8; 20]) -> usize {
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 20];
    let mut i = 0usize;
    while n > 0 {
        tmp[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    for j in 0..i {
        buf[j] = tmp[i - 1 - j];
    }
    i
}

pub fn fmt_fixed6(x: f64, buf: &mut [u8; 32]) -> usize {
    if x.is_nan() {
        buf[..3].copy_from_slice(b"nan");
        return 3;
    }
    if x.is_infinite() {
        if x.is_sign_negative() {
            buf[..4].copy_from_slice(b"-inf");
            return 4;
        }
        buf[..3].copy_from_slice(b"inf");
        return 3;
    }
    let mut i = 0usize;
    let neg = x.is_sign_negative();
    let ax = if neg { -x } else { x };
    if ax >= 1.0e13 {
        buf[..3].copy_from_slice(b"ovf");
        return 3;
    }
    if neg {
        buf[i] = b'-';
        i += 1;
    }
    let scaled = (ax * 1_000_000.0 + 0.5) as u64;
    let whole = scaled / 1_000_000;
    let frac = scaled % 1_000_000;
    let mut wb = [0u8; 20];
    let wl = fmt_u64(whole, &mut wb);
    buf[i..i + wl].copy_from_slice(&wb[..wl]);
    i += wl;
    buf[i] = b'.';
    i += 1;
    let mut digits = [0u8; 6];
    let mut f = frac;
    for k in (0..6).rev() {
        digits[k] = b'0' + (f % 10) as u8;
        f /= 10;
    }
    buf[i..i + 6].copy_from_slice(&digits);
    i + 6
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;

    fn fmt_u64_s(n: u64) -> String {
        let mut b = [0u8; 20];
        let len = fmt_u64(n, &mut b);
        String::from_utf8(b[..len].to_vec()).unwrap()
    }

    fn fmt_fixed6_s(x: f64) -> String {
        let mut b = [0u8; 32];
        let len = fmt_fixed6(x, &mut b);
        String::from_utf8(b[..len].to_vec()).unwrap()
    }

    #[test]
    fn u64_zero() {
        assert_eq!(fmt_u64_s(0), "0");
    }

    #[test]
    fn u64_max() {
        assert_eq!(fmt_u64_s(u64::MAX), "18446744073709551615");
    }

    #[test]
    fn u64_small() {
        assert_eq!(fmt_u64_s(42), "42");
    }

    #[test]
    fn fixed6_positive() {
        assert_eq!(fmt_fixed6_s(39.953), "39.953000");
    }

    #[test]
    fn fixed6_zero() {
        assert_eq!(fmt_fixed6_s(0.0), "0.000000");
    }

    #[test]
    fn fixed6_negative() {
        assert_eq!(fmt_fixed6_s(-1.25), "-1.250000");
    }

    #[test]
    fn fixed6_rounds() {
        assert_eq!(fmt_fixed6_s(0.1234567), "0.123457");
    }

    #[test]
    fn fixed6_non_finite() {
        assert_eq!(fmt_fixed6_s(f64::NAN), "nan");
        assert_eq!(fmt_fixed6_s(f64::INFINITY), "inf");
        assert_eq!(fmt_fixed6_s(f64::NEG_INFINITY), "-inf");
    }

    #[test]
    fn fixed6_overflow_guard() {
        assert_eq!(fmt_fixed6_s(1.0e13), "ovf");
        assert_eq!(fmt_fixed6_s(-2.0e13), "ovf");
    }

    #[test]
    fn fixed6_negative_zero() {
        assert_eq!(fmt_fixed6_s(-0.0), "-0.000000");
    }
}
