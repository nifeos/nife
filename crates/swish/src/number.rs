//! **A number as the shell prints it**: [`write_num`], which nearly every renderer here calls.
//!
//! Name: provisional (milestone 884, 2026-10-12 UTC), for calef's naming pass.

/// Write a small unsigned number in base 10.
pub fn write_num(mut v: u64, out: &mut dyn FnMut(&[u8])) {
    let mut digits = [0u8; 20];
    let mut i = digits.len();
    loop {
        i -= 1;
        digits[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    out(&digits[i..]);
}

#[cfg(test)]
mod tests;
