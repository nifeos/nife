//! Fuzz `ext4_server::Server::handle` with whole client sessions over a corrupted corpus image,
//! and check the server's rules after every request.
//!
//! An input picks a generated image and a few corruptions of it (`ext4_oracle_fuzz::image`), then
//! a session of up to [`MAXIMUM_REQUESTS`] requests as the kernel would deliver them. A request's
//! name is drawn from the corpus tree's own names (`tools/ext4_oracle/corpus/populate.py`), the
//! names a walk must refuse, or raw bytes, so a session reaches real files and directories and
//! not only refusals.
//!
//! # The rules it checks, beyond "no panic"
//!
//! 1. *Read only.* Every verb the contract marks as mutating answers `EROFS`; the binding verbs
//!    and an unknown code answer `EINVAL`.
//! 2. *Handles.* A minted handle is in `1..=MAXIMUM_HANDLE`, is not one already held, and no more
//!    than 256 are held at once. `CLOSE` of `ROOT` answers `EINVAL`, and of a held handle
//!    succeeds. A request on a handle that is neither `ROOT` nor held succeeds at nothing. The
//!    model of what is held is this file's own, kept from the replies.
//! 3. *Scope.* A request that succeeds through a path named only names: no `.`, `..`, empty
//!    component, leading or trailing `/`, or the attribute store's reserved name.
//! 4. *Bounds.* A reply never claims more bytes than the window it could fill, and every error
//!    word carries a code in `1..=MAXIMUM_ERRNO`.
//! 5. *Stable reads.* The image never changes, so the same `READ` asked twice answers the same
//!    bytes.
//!
//! Name: provisional, minted 2026-11-26 (UTC) by lane/ext4-fuzz-rotation; calef names things.

#![no_main]

use std::collections::BTreeSet;

use ext4_oracle_fuzz::{image, is_relative_path_of_names, request, result, serve};
use file_system_contract::directory::{self, rights};
use file_system_contract::{extended_attribute, file_system, verb};
use libfuzzer_sys::arbitrary::{Result, Unstructured};
use libfuzzer_sys::fuzz_target;

/// Requests per session. Long enough to open, descend, read, close and reuse a slot.
const MAXIMUM_REQUESTS: usize = 48;
/// The most handles a server holds at once (`ext4_server`'s own bound).
const MAXIMUM_OPEN_HANDLES: usize = 256;
/// The page a listing, an attribute or the statistics reply fills at most.
const PAGE: usize = file_system_contract::block::BLOCK_BYTES;

/// **Path components the corpus holds**, and the ones a walk must refuse.
const COMPONENTS: &[&[u8]] = &[
    b"files",
    b"size-0",
    b"size-1",
    b"size-60",
    b"size-1048579",
    b"hard-c",
    b"extents",
    b"depth-0",
    b"depth-1",
    b"depth-2",
    b"hole-only",
    b"htree",
    b"entry-00000-",
    b"links",
    b"fast",
    b"slow",
    b"fast-59",
    b"slow-60",
    b"dangling",
    b"hard-a",
    b"hard-b",
    b"attributes",
    b"small",
    b"large",
    b"directory",
    b"names",
    b"with space",
    "café".as_bytes(),
    "日本語".as_bytes(),
    b".hidden",
    b"-dash",
    b"fifo",
    b"empty-directory",
    b"deep",
    b"level-0",
    b"level-1",
    b"leaf",
    b"timestamps",
    b"nanoseconds",
    b"before-1970",
    b"after-2106",
    b"lost+found",
    b".",
    b"..",
    b"",
    b".extended-attributes",
    b"a\0b",
];

/// **Attribute names the corpus holds**, and a few it does not.
const ATTRIBUTES: &[&[u8]] = &[
    b"user.color",
    b"user.empty",
    b"user.blob",
    b"user.second",
    b"user.on-a-directory",
    b"user.absent",
    b"",
    b"trusted.x",
    b"user.\0",
];

/// The verbs a session draws from: every code the contract defines, mutating ones included, and
/// a few it does not.
const CODES: &[u64] = &[
    file_system::OPEN,
    file_system::READ,
    file_system::WRITE,
    file_system::CLOSE,
    file_system::FSTAT,
    file_system::CREATE,
    file_system::TRUNCATE,
    file_system::OPENDIR,
    file_system::READDIR,
    file_system::MKDIR,
    file_system::RENAME,
    file_system::UNLINK,
    file_system::RMDIR,
    file_system::GETXATTR,
    file_system::SETXATTR,
    file_system::LISTXATTR,
    file_system::REMOVEXATTR,
    file_system::GET_VOLUME_STATISTICS,
    file_system::SYNC,
    file_system::GET_MODIFICATION_TIME_UNIX_SECONDS,
    file_system::SET_MODIFICATION_TIME,
    file_system::SET_MODIFICATION_TIME_AT_UNIX_SECONDS,
    file_system::NARROW,
    64,
    65,
    66,
    0,
    24,
    63,
    67,
    0xff,
];

/// Whether the name a verb carries is a path the walk resolves (as opposed to an attribute
/// name, or no name).
fn takes_path(code: u64) -> bool {
    matches!(
        code,
        file_system::OPEN | file_system::OPENDIR | file_system::GET_MODIFICATION_TIME_UNIX_SECONDS
    )
}

fn path(u: &mut Unstructured) -> Result<Vec<u8>> {
    if u.ratio(1, 8)? {
        let len = u.int_in_range(0..=300)?;
        return Ok(u.bytes(len)?.to_vec());
    }
    let mut out = Vec::new();
    for i in 0..u.int_in_range(1..=4)? {
        if i > 0 {
            out.push(b'/');
        }
        out.extend_from_slice(u.choose(COMPONENTS)?);
    }
    if u.ratio(1, 16)? {
        out.insert(0, b'/');
    }
    Ok(out)
}

fn step_rights(u: &mut Unstructured) -> Result<u64> {
    Ok(match u.int_in_range(0..=3)? {
        0 => rights::PARENT_RIGHTS,
        1 => rights::READ,
        2 => rights::ENUMERATE | rights::DESCEND | rights::READ,
        _ => u.arbitrary()?,
    })
}

/// One request as drawn: its verb, handle, name, second word and length.
struct Drawn {
    code: u64,
    handle: u64,
    name: Vec<u8>,
    w1: u64,
    len: u64,
}

fn draw(u: &mut Unstructured, held: &BTreeSet<u64>) -> Result<Drawn> {
    let code = *u.choose(CODES)?;
    let handle = match u.int_in_range(0..=5)? {
        0 | 1 => file_system::ROOT,
        2..=4 if !held.is_empty() => *held.iter().nth(u.choose_index(held.len())?).unwrap(),
        _ => u.int_in_range(0..=file_system::MAXIMUM_HANDLE)?,
    };
    let mut name: Vec<u8> = match code {
        file_system::GETXATTR => u.choose(ATTRIBUTES)?.to_vec(),
        _ if takes_path(code) || u.ratio(1, 4)? => path(u)?,
        _ => Vec::new(),
    };
    name.truncate(PAGE);
    let w1: u64 = match code {
        file_system::OPEN | file_system::OPENDIR | file_system::NARROW => step_rights(u)?,
        file_system::READ | file_system::READDIR => u.int_in_range(0..=1 << 21)?,
        _ => u.arbitrary()?,
    };
    let len = match code {
        file_system::READ => u.int_in_range(0..=file_system::MAXIMUM_TRANSFER_BYTES as u64 + 1)?,
        _ => name.len() as u64,
    };
    Ok(Drawn {
        code,
        handle,
        name,
        w1,
        len,
    })
}

/// Rules 1 and 2 for a request whose answer they settle: whether the request is done with.
fn refused_as_it_must_be(
    code: u64,
    handle: u64,
    answer: Reply,
    r0: i64,
    held: &BTreeSet<u64>,
) -> bool {
    // Rule 1: read only.
    match verb::of(code) {
        Some(v) if v.mutates() => {
            assert_eq!(
                answer,
                Err(directory::EROFS),
                "mutating verb {code} answered {r0}"
            );
            return true;
        }
        Some(_) if matches!(code, 64..=66) => {
            assert_eq!(
                answer,
                Err(directory::EINVAL),
                "binding verb {code} answered {r0}"
            );
            return true;
        }
        None => {
            assert_eq!(
                answer,
                Err(directory::EINVAL),
                "unknown code {code} answered {r0}"
            );
            return true;
        }
        Some(_) => {}
    }

    // Rule 2: ROOT stays bound, and no handle but ROOT and the held ones does anything.
    if code == file_system::CLOSE && handle == file_system::ROOT {
        assert_eq!(
            answer,
            Err(directory::EINVAL),
            "CLOSE of ROOT answered {r0}"
        );
        return true;
    }
    if handle != file_system::ROOT && !held.contains(&handle) {
        assert!(
            answer.is_err(),
            "verb {code} on handle {handle}, never minted, answered {r0}"
        );
        return true;
    }
    if code == file_system::CLOSE {
        assert_eq!(answer, Ok(0), "CLOSE of held handle {handle} answered {r0}");
    }
    false
}

type Reply = core::result::Result<u64, i32>;

fn session(u: &mut Unstructured) -> Result<()> {
    let image = image(u)?;
    let Some(mut server) = serve(image) else {
        return Ok(());
    };
    let mut window = vec![0u8; file_system::MAXIMUM_TRANSFER_BYTES];
    let mut held: BTreeSet<u64> = BTreeSet::new();

    for _ in 0..u.int_in_range(1..=MAXIMUM_REQUESTS)? {
        let Drawn {
            code,
            handle,
            name,
            w1,
            len,
        } = draw(u, &held)?;
        window[..name.len()].copy_from_slice(&name);
        let w0 = request(code, handle, len);
        let (r0, _) = server.handle(w0, w1, &mut window);
        let answer = result(r0);
        if refused_as_it_must_be(code, handle, answer, r0, &held) {
            continue;
        }
        let Ok(value) = answer else { continue };

        // Rule 3: scope.
        if takes_path(code) {
            assert!(
                is_relative_path_of_names(&name)
                    && !name
                        .split(|&b| b == b'/')
                        .any(|c| c == extended_attribute::STORE_DIRECTORY.as_bytes()),
                "verb {code} succeeded through path {name:?}"
            );
        }

        match code {
            file_system::OPEN | file_system::OPENDIR | file_system::NARROW => {
                assert!(
                    (1..=file_system::MAXIMUM_HANDLE).contains(&value),
                    "minted handle {value} is out of range"
                );
                assert!(held.insert(value), "minted handle {value} is already held");
                assert!(
                    held.len() <= MAXIMUM_OPEN_HANDLES,
                    "{} handles held",
                    held.len()
                );
            }
            file_system::CLOSE => {
                held.remove(&handle);
            }
            // Rules 4 and 5: bounds, and a stable read.
            file_system::READ => {
                let n = value as usize;
                assert!(
                    n as u64 <= len && n <= window.len(),
                    "READ of {len} answered {n}"
                );
                let first = window[..n].to_vec();
                let (again, _) = server.handle(w0, w1, &mut window);
                assert_eq!(again, r0, "the same READ answered {r0}, then {again}");
                assert_eq!(
                    &window[..n],
                    &first[..],
                    "the same READ answered other bytes"
                );
            }
            file_system::READDIR | file_system::LISTXATTR | file_system::GET_VOLUME_STATISTICS => {
                assert!(
                    value as usize <= PAGE,
                    "verb {code} filled {value} bytes of a page"
                );
            }
            file_system::GETXATTR => {
                let n = extended_attribute::reply_value_len(r0);
                assert!(n <= PAGE, "GETXATTR filled {n} bytes of a page");
            }
            _ => {}
        }
    }
    Ok(())
}

fuzz_target!(|data: &[u8]| {
    let _ = session(&mut Unstructured::new(data));
});
