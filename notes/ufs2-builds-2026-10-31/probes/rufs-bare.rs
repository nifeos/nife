// The no_std probe for rufs: names the crate's root inode so the build has to compile it. rufs as
// published needs std, so this probe stops at the crate itself; the patched port's own probe is
// rufs-bare-port.rs.
#![no_std]
#![no_main]
extern crate alloc;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    core::hint::black_box(rufs::InodeNum::ROOT.get());
    halt()
}
