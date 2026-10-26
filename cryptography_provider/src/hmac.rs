//! HMAC behind `rustls`' `Hmac` trait, which is all `rustls` needs to build HKDF itself.
//!
//! `rustls::crypto::tls13::HkdfUsingHmac` turns an `Hmac` into the whole TLS 1.3 key schedule, so
//! this file is the only thing standing between `hmac` and a working schedule. That is worth
//! saying because the obvious alternative, implementing `Hkdf` directly over the `hkdf` crate, is
//! more code and more places to be wrong about a key derivation.

use alloc::boxed::Box;

use hmac::Mac;
use rustls::crypto::hmac::{Hmac, Key, Tag};

/// HMAC-SHA256.
pub static SHA256: Algorithm<hmac::Hmac<sha2::Sha256>> = Algorithm {
    output_len: 32,
    _marker: core::marker::PhantomData,
};

/// HMAC-SHA384.
pub static SHA384: Algorithm<hmac::Hmac<sha2::Sha384>> = Algorithm {
    output_len: 48,
    _marker: core::marker::PhantomData,
};

/// HMAC over one hash function.
///
/// The parameter is the **HMAC** type (`hmac::Hmac<Sha256>`) rather than the hash type, which is
/// not cosmetic: `Mac` and `KeyInit` are implemented on the former, and naming the latter makes
/// every bound here a restatement of `hmac`'s own internal `CoreProxy` plumbing.
// **`PhantomData<fn() -> T>` rather than `PhantomData<T>`, and it is a safety choice rather than a
// style one.** These types are `static`s, so they must be `Send` and `Sync`. With `PhantomData<T>`
// they inherit `T`'s auto traits and would need a hand-written `unsafe impl` asserting the
// compiler is wrong, which `script/lint`'s unsafe-obligation count is right to object to. A
// function pointer returning `T` carries the same type information to the reader and is
// unconditionally `Send + Sync`, because it owns nothing.
#[derive(Debug)]
pub struct Algorithm<M> {
    output_len: usize,
    _marker: core::marker::PhantomData<fn() -> M>,
}


impl<M> Hmac for Algorithm<M>
where
    M: Mac + hmac::digest::KeyInit + Clone + Send + Sync + 'static,
{
    fn with_key(&self, key: &[u8]) -> Box<dyn Key> {
        // `new_from_slice` on HMAC accepts a key of any length: RFC 2104 hashes one that is too
        // long and pads one that is too short, so the error arm is unreachable rather than
        // unhandled. `expect` rather than a silent fallback, because a future `hmac` that could
        // fail here should stop the handshake rather than continue with a key nobody chose.
        Box::new(Keyed::<M>(
            <M as hmac::KeyInit>::new_from_slice(key).expect("HMAC accepts a key of any length (RFC 2104)"),
        ))
    }

    fn hash_output_len(&self) -> usize {
        self.output_len
    }
}

/// One HMAC key, reusable across taggings.
struct Keyed<M>(M);

impl<M> Key for Keyed<M>
where
    M: Mac + Clone + Send + Sync + 'static,
{
    fn sign_concat(&self, first: &[u8], middle: &[&[u8]], last: &[u8]) -> Tag {
        // The key is cloned per tagging rather than the state being reset, because `rustls` hands
        // out one `Key` and taggings must not see each other's input.
        let mut ctx = self.0.clone();
        ctx.update(first);
        for part in middle {
            ctx.update(part);
        }
        ctx.update(last);
        Tag::new(&ctx.finalize().into_bytes()[..])
    }

    fn tag_len(&self) -> usize {
        <M as hmac::digest::OutputSizeUser>::output_size()
    }
}
