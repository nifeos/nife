//! NIFEPKG2 as §307 (how long a name may be) rule 3 describes it: magic, three u16-length fields,
//! count, reserved, then a walked table of (u16 name length, name, u32 offset, u32 len, 32-byte
//! digest).
pub const MAX_MEMBERS: usize = 64;
pub const MAX_PATH: usize = 4096;
#[derive(Clone, Copy)]
pub struct Package<'a> { b: &'a [u8], count: usize, table: usize, id: [(usize, usize); 3] }
fn u16_at(b: &[u8], at: usize) -> Option<usize> { Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?) as usize) }
fn u32_at(b: &[u8], at: usize) -> Option<usize> { Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?) as usize) }
fn text(b: &[u8], at: usize, len: usize) -> Option<&str> { core::str::from_utf8(b.get(at..at.checked_add(len)?)?).ok() }
pub struct Entry<'a> { pub name: &'a str, pub data: &'a [u8], pub digest: [u8; 32] }
impl<'a> Package<'a> {
    pub fn parse(b: &'a [u8]) -> Option<Self> {
        if b.get(..8)? != b"NIFEPKG2" { return None; }
        let mut at = 8; let mut id = [(0, 0); 3];
        for f in &mut id { let n = u16_at(b, at)?; text(b, at + 2, n)?; *f = (at + 2, n); at += 2 + n; }
        let count = u32_at(b, at)?; if count > MAX_MEMBERS { return None; }
        let p = Package { b, count, table: at + 8, id };
        let mut e = p.table;
        for _ in 0..count { e = p.entry_at(e)?.1; }
        Some(p)
    }
    fn entry_at(&self, at: usize) -> Option<(Entry<'a>, usize)> {
        let b = self.b; let n = u16_at(b, at)?; if n > MAX_PATH { return None; }
        let name = text(b, at + 2, n)?; let f = at + 2 + n;
        let (off, len) = (u32_at(b, f)?, u32_at(b, f + 4)?);
        let data = b.get(off..off.checked_add(len)?)?;
        let digest = b.get(f + 8..f + 40)?.try_into().ok()?;
        Some((Entry { name, data, digest }, f + 40))
    }
    fn id(&self, i: usize) -> &'a str { let (a, n) = self.id[i]; text(self.b, a, n).unwrap_or("") }
    pub fn name(&self) -> &'a str { self.id(0) }
    pub fn version(&self) -> &'a str { self.id(1) }
    pub fn architecture(&self) -> &'a str { self.id(2) }
    pub fn len(&self) -> usize { self.count }
    pub fn member(&self, index: usize) -> Option<Entry<'a>> {
        let mut at = self.table;
        for i in 0..self.count { let (e, next) = self.entry_at(at)?; if i == index { return Some(e); } at = next; }
        None
    }
    pub fn read(&self, name: &str) -> Option<&'a [u8]> {
        (0..self.count).filter_map(|i| self.member(i)).find(|e| e.name == name).map(|e| e.data)
    }
}
