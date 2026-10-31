// The std probe for ufs-core: parse a superblock, look a path up and read it, so the linker has to
// resolve the reader. The image is zeroes; this probe measures the build, not a read.
use std::hint::black_box;

fn main() {
    let image = black_box(vec![0u8; 1 << 20]);
    match ufs::Superblock::parse(&image[ufs::SBLOCK_UFS2..]) {
        Ok(sb) => {
            let file = ufs::read_path_content(&image, &sb, black_box("/activation/current"));
            let dir = ufs::list_dir(&image, &sb, ufs::UFS_ROOTINO);
            println!("{:?} {:?}", file.map(|f| f.map(|b| b.len())), dir.map(|d| d.len()));
        }
        Err(e) => println!("{e}"),
    }
}
