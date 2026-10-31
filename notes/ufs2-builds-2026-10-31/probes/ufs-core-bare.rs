// The no_std probe for ufs-core: the same calls as ufs-core.rs, as a freestanding binary with a
// bump allocator, so the linker resolves the reader against core and alloc alone.
#![no_std]
#![no_main]
extern crate alloc;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let image = core::hint::black_box(alloc::vec![0u8; 1 << 20]);
    if let Ok(sb) = ufs::Superblock::parse(&image[ufs::SBLOCK_UFS2..]) {
        let file = ufs::read_path_content(&image, &sb, core::hint::black_box("/activation/current"));
        let dir = ufs::list_dir(&image, &sb, ufs::UFS_ROOTINO);
        core::hint::black_box((file.is_ok(), dir.is_ok()));
    }
    halt()
}
