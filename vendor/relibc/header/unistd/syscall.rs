// Seeded from relibc (MIT, vendor/relibc/LICENSE) at 893a3b9133ac, 2026-10-10 (UTC), for milestone 835; nife owns it from here, and its edits say `nife:` where they are (vendor/README.md).
use crate::platform::types::c_long;

/// Non-POSIX, see <https://www.man7.org/linux/man-pages/man3/getopt.3.html>.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn syscall(sysno: c_long, mut args: ...) -> c_long {
    let a1 = unsafe { args.next_arg::<usize>() };
    let a2 = unsafe { args.next_arg::<usize>() };
    let a3 = unsafe { args.next_arg::<usize>() };
    let a4 = unsafe { args.next_arg::<usize>() };
    let a5 = unsafe { args.next_arg::<usize>() };
    let a6 = unsafe { args.next_arg::<usize>() };

    (unsafe { sc::syscall6(sysno as usize, a1, a2, a3, a4, a5, a6) }) as c_long
}
