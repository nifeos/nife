//! A hand-written ustar reader, with pax `path` records unless `tar_own_ustar` is set, for a
//! package whose first member is MANIFEST. Borrowing, allocation-free, every member walked.
use crate::manifest::Manifest;
const B: usize = 512;
pub const MAX_MEMBERS: usize = 64;
#[derive(Clone, Copy, PartialEq)]
pub struct Name<'a> { pub prefix: &'a str, pub name: &'a str }
impl Name<'_> {
    pub fn is(&self, want: &str) -> bool {
        if self.prefix.is_empty() { return self.name == want; }
        let p = self.prefix.len();
        want.len() == p + 1 + self.name.len() && &want[..p] == self.prefix && want.as_bytes()[p] == b'/' && &want[p + 1..] == self.name
    }
}
pub struct Entry<'a> { pub name: Name<'a>, pub data: &'a [u8], pub digest: [u8; 32] }
pub struct Package<'a> { b: &'a [u8], m: Manifest<'a>, first: usize, count: usize }
fn cstr(f: &[u8]) -> Option<&str> { let e = f.iter().position(|&c| c == 0).unwrap_or(f.len()); core::str::from_utf8(&f[..e]).ok() }
fn octal(f: &[u8]) -> Option<usize> {
    let mut n: usize = 0; let mut seen = false;
    for &c in f { match c { b'0'..=b'7' => { n = n.checked_mul(8)?.checked_add((c - b'0') as usize)?; seen = true; } b' ' | 0 if seen => break, b' ' => {} _ => return None } }
    seen.then_some(n)
}
fn checksum_ok(h: &[u8]) -> bool {
    let sum: usize = h.iter().enumerate().map(|(i, &c)| if (148..156).contains(&i) { 32 } else { c as usize }).sum();
    octal(&h[148..156]) == Some(sum)
}
/// One raw header and what follows it: (typeflag, header, data, next offset).
fn raw(b: &[u8], at: usize) -> Option<(u8, &[u8], &[u8], usize)> {
    let h = b.get(at..at + B)?;
    if h.iter().all(|&c| c == 0) { return Some((0xff, h, &[], at)); }
    if &h[257..263] != b"ustar\0" || !checksum_ok(h) { return None; }
    let size = octal(&h[124..136])?; let start = at + B;
    let data = b.get(start..start.checked_add(size)?)?;
    Some((h[156], h, data, start + size.next_multiple_of(B)))
}
#[cfg(not(feature = "tar_own_ustar"))]
fn pax_path(rec: &[u8]) -> Option<Option<&str>> {
    let mut rest = rec; let mut path = None;
    while !rest.is_empty() {
        let sp = rest.iter().position(|&c| c == b' ')?;
        let n: usize = core::str::from_utf8(&rest[..sp]).ok()?.parse().ok()?;
        if n <= sp + 1 || n > rest.len() || rest[n - 1] != b'\n' { return None; }
        let kv = core::str::from_utf8(&rest[sp + 1..n - 1]).ok()?;
        let (k, v) = kv.split_once('=')?;
        match k { "path" => path = Some(v), "size" | "linkpath" | "uid" | "gid" | "uname" | "gname" => return None, _ => {} }
        rest = &rest[n..];
    }
    Some(path)
}
/// The next regular member at or after `at`: (name, data, next).
fn next(b: &[u8], at: usize) -> Option<Option<(Name<'_>, &[u8], usize)>> {
    #[allow(unused_mut)] let mut long: Option<&str> = None;
    #[allow(unused_mut)] let mut at = at;
    loop {
        let (t, h, data, after) = raw(b, at)?;
        match t {
            0xff => return Some(None),
            #[cfg(not(feature = "tar_own_ustar"))]
            b'x' => { long = pax_path(data)?; at = after; continue; }
            b'0' | 0 => {
                let name = match long { Some(p) => Name { prefix: "", name: p }, None => Name { prefix: cstr(&h[345..500])?, name: cstr(&h[0..100])? } };
                return Some(Some((name, data, after)));
            }
            _ => return None,
        }
    }
}
impl<'a> Package<'a> {
    pub fn parse(b: &'a [u8]) -> Option<Self> {
        let (n, data, first) = next(b, 0)??;
        if !n.is("MANIFEST") { return None; }
        let m = Manifest::parse(data)?;
        let mut at = first; let mut count = 0;
        while let Some((name, _, after)) = next(b, at)? {
            count += 1; if count > MAX_MEMBERS { return None; }
            let mut found = false;
            for l in m.text.split_terminator('\n').filter_map(|l| l.strip_prefix("member ")) {
                if let Some((_, p)) = l.split_once(' ') { if name.is(p) { found = true; break; } }
            }
            if !found { return None; }
            at = after;
        }
        if count != m.members { return None; }
        Some(Package { b, m, first, count })
    }
    pub fn name(&self) -> &'a str { self.m.name }
    pub fn version(&self) -> &'a str { self.m.version }
    pub fn architecture(&self) -> &'a str { self.m.architecture }
    pub fn len(&self) -> usize { self.count }
    pub fn member(&self, index: usize) -> Option<Entry<'a>> {
        let mut at = self.first;
        for i in 0..=index {
            let (name, data, after) = next(self.b, at)??;
            if i == index {
                let digest = self.m.text.split_terminator('\n').filter_map(|l| l.strip_prefix("member ")).find_map(|v| {
                    let (d, p) = v.split_once(' ')?; if name.is(p) { crate::manifest::decode_pub(d) } else { None }
                })?;
                return Some(Entry { name, data, digest });
            }
            at = after;
        }
        None
    }
    pub fn read(&self, want: &str) -> Option<&'a [u8]> {
        let mut at = self.first;
        while let Some((name, data, after)) = next(self.b, at)? { if name.is(want) { return Some(data); } at = after; }
        None
    }
}
