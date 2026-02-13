const DIGIT: &[u8] = b"0123456789ABCDEF";
const MAX_DIGITS: usize = 20; // 10よりちいさい基数はとらない
// RangeInclusiveより軽量
pub struct ClosedInterval<T: std::cmp::Ord>(pub T, pub T);

impl<T: std::cmp::Ord + Copy> ClosedInterval<T> {
    #[inline(always)]
    pub fn contains(&self, val: T) -> bool {
        self.0 <= val && self.1 >= val
    }
}

pub fn itoa_usize(buf: &mut [u8; MAX_DIGITS], mut val: usize, radix: usize) -> usize {
    let mut i = MAX_DIGITS - 1;
    loop {
        buf[i] = DIGIT[val % radix];
        val /= radix;
        if val == 0 {
            break;
        }
        i -= 1;
    }
    i
}

pub fn push_itoa_usize_to_string(s: &mut String, val: usize, radix: usize) {
    let mut buf = [0u8; MAX_DIGITS];
    let i = itoa_usize(&mut buf, val, radix);
    unsafe {
        s.push_str(std::str::from_utf8_unchecked(&buf[i..]));
    }
}

pub fn push_itoa_usize_to_vec_u8(v: &mut Vec<u8>, val: usize, radix: usize) {
    let mut buf = [0u8; MAX_DIGITS];
    let i = itoa_usize(&mut buf, val, radix);
    v.extend_from_slice(&buf[i..])
}

pub fn push_str_to_vec_u8(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
}

pub fn push_char_to_vec_u8(out: &mut Vec<u8>, c: char) {
    let mut b = [0; 4];
    out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
}
