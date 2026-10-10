// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
use core::slice::Split;

use alloc::vec::Vec;

use crate::{c_str::CStr, header::limits::PATH_MAX};

pub struct PathSearchIter<'a> {
    file_bytes: &'a [u8],
    path_splits: Split<'a, u8, fn(&u8) -> bool>,
}

const PATH_SEPARATOR: u8 = b':';

impl<'a> PathSearchIter<'a> {
    /// Construct a new PATH parser.
    /// Safety: file must have no slashes
    pub fn new(file_bytes: &'a [u8], path_env: &'a CStr) -> Self {
        Self {
            file_bytes,
            path_splits: path_env.to_bytes().split(|&b| b == PATH_SEPARATOR),
        }
    }
}

impl<'a> Iterator for PathSearchIter<'a> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        for path in &mut self.path_splits {
            let len = path.len() + self.file_bytes.len() + 2;
            if len > PATH_MAX {
                continue;
            }
            // nife: a `Vec` in place of relibc's `arrayvec`; the length was checked just above.
            let mut program: Vec<u8> = Vec::with_capacity(len);
            program.extend_from_slice(path);
            program.push(b'/');
            program.extend_from_slice(self.file_bytes);
            program.push(b'\0');
            return Some(program);
        }

        None
    }
}
