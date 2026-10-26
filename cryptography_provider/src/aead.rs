//! The TLS 1.3 record layer: AES-128-GCM, AES-256-GCM and ChaCha20-Poly1305 behind `rustls`'
//! `Tls13AeadAlgorithm`.
//!
//! This file computes nothing: every byte of confidentiality and authenticity comes from `aes-gcm`
//! or `chacha20poly1305`. It assembles the record as RFC 8446 section 5.2 says: plaintext, the real
//! content type as one byte, then the tag, with the five-byte *outer* header as additional data. It
//! uses the detached form (`encrypt_in_place_detached`, on a plain `&mut [u8]`) because `rustls`'
//! buffers implement no `aead::Buffer`, so the record is not copied twice. The nonce and additional
//! data are `rustls`' own (`Nonce::new`, `make_tls13_aad`), used rather than rewritten.
//! notes/cryptography-provider/the-glue.md has the reasons.

use alloc::boxed::Box;

use aes_gcm::AeadInOut;
use aes_gcm::aead::consts::{U12, U16};
use aes_gcm::aead::{AeadCore, KeyInit};
use rustls::crypto::cipher::{
    AeadKey, InboundOpaqueMessage, InboundPlainMessage, Iv, MessageDecrypter, MessageEncrypter,
    Nonce, OutboundOpaqueMessage, OutboundPlainMessage, PrefixedPayload, Tls13AeadAlgorithm,
    UnsupportedOperationError, make_tls13_aad,
};
use rustls::{ConnectionTrafficSecrets, ContentType, Error, ProtocolVersion};

/// Every AEAD in TLS 1.3 has a 16-byte tag. Named rather than spelled four times, and asserted
/// against each cipher's own associated constant in `tests`.
const TAG_LEN: usize = 16;

/// `TLS13_AES_128_GCM_SHA256`'s cipher.
pub static AES128_GCM: Algorithm<aes_gcm::Aes128Gcm> = Algorithm {
    key_len: 16,
    kind: Kind::Aes128Gcm,
    _marker: core::marker::PhantomData,
};

/// `TLS13_AES_256_GCM_SHA384`'s cipher.
pub static AES256_GCM: Algorithm<aes_gcm::Aes256Gcm> = Algorithm {
    key_len: 32,
    kind: Kind::Aes256Gcm,
    _marker: core::marker::PhantomData,
};

/// `TLS13_CHACHA20_POLY1305_SHA256`'s cipher.
pub static CHACHA20_POLY1305: Algorithm<chacha20poly1305::ChaCha20Poly1305> = Algorithm {
    key_len: 32,
    kind: Kind::ChaCha20Poly1305,
    _marker: core::marker::PhantomData,
};

/// Which of the three this is, kept only so `extract_keys` can name the right
/// `ConnectionTrafficSecrets` variant. It is not used to dispatch any cryptography.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Aes128Gcm,
    Aes256Gcm,
    ChaCha20Poly1305,
}

/// One AEAD, generic over the crate type that implements it.
// **`PhantomData<fn() -> T>` rather than `PhantomData<T>`, and it is a safety choice rather than a
// style one.** These types are `static`s, so they must be `Send` and `Sync`. With `PhantomData<T>`
// they inherit `T`'s auto traits and would need a hand-written `unsafe impl` asserting the
// compiler is wrong, which `script/lint`'s unsafe-obligation count is right to object to. A
// function pointer returning `T` carries the same type information to the reader and is
// unconditionally `Send + Sync`, because it owns nothing.
#[derive(Debug)]
pub struct Algorithm<C> {
    key_len: usize,
    kind: Kind,
    _marker: core::marker::PhantomData<fn() -> C>,
}


// **The nonce and tag sizes are in the bound rather than checked at run time**, so a cipher whose
// shape did not match TLS 1.3's record layer could not be named above at all. TLS 1.3 fixes both:
// a 96-bit nonce (RFC 8446 section 5.3) and a 128-bit tag for all three of its AEADs.
impl<C> Tls13AeadAlgorithm for Algorithm<C>
where
    C: KeyInit + AeadInOut + AeadCore<NonceSize = U12, TagSize = U16> + Send + Sync + 'static,
{
    fn encrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageEncrypter> {
        Box::new(Encrypter::<C> {
            cipher: new_cipher::<C>(&key),
            iv,
        })
    }

    fn decrypter(&self, key: AeadKey, iv: Iv) -> Box<dyn MessageDecrypter> {
        Box::new(Decrypter::<C> {
            cipher: new_cipher::<C>(&key),
            iv,
        })
    }

    fn key_len(&self) -> usize {
        self.key_len
    }

    fn extract_keys(
        &self,
        key: AeadKey,
        iv: Iv,
    ) -> Result<ConnectionTrafficSecrets, UnsupportedOperationError> {
        // Only reached by a caller that asked for the raw traffic secrets, which a package client
        // never does. Answered honestly rather than refused, because the information is available.
        Ok(match self.kind {
            Kind::Aes128Gcm => ConnectionTrafficSecrets::Aes128Gcm { key, iv },
            Kind::Aes256Gcm => ConnectionTrafficSecrets::Aes256Gcm { key, iv },
            Kind::ChaCha20Poly1305 => ConnectionTrafficSecrets::Chacha20Poly1305 { key, iv },
        })
    }
}

/// Build the cipher from key material `rustls` already sized for us.
///
/// `rustls` allocates the `AeadKey` at exactly `key_len()` bytes, which is this provider's own
/// answer above, so the length cannot disagree unless this file does. `expect` says that out loud
/// rather than inventing an error path that would be unreachable and untested.
fn new_cipher<C: KeyInit>(key: &AeadKey) -> C {
    C::new_from_slice(key.as_ref()).expect("rustls sizes the key from this provider's key_len()")
}

struct Encrypter<C> {
    cipher: C,
    iv: Iv,
}

impl<C> MessageEncrypter for Encrypter<C>
where
    C: AeadInOut + AeadCore<NonceSize = U12, TagSize = U16> + Send + Sync + 'static,
{
    fn encrypt(
        &mut self,
        msg: OutboundPlainMessage<'_>,
        seq: u64,
    ) -> Result<OutboundOpaqueMessage, Error> {
        let total_len = self.encrypted_payload_len(msg.payload.len());
        let mut payload = PrefixedPayload::with_capacity(total_len);

        // RFC 8446 section 5.2's `TLSInnerPlaintext`: the content, then the real content type as
        // one byte. There is no padding, which is allowed and is what every implementation does
        // unless it is deliberately hiding lengths.
        payload.extend_from_chunks(&msg.payload);
        payload.extend_from_slice(&msg.typ.to_array());

        // The additional data is the *outer* record header, and its length field counts the tag
        // that does not exist yet, which is why `total_len` is computed first.
        let aad = make_tls13_aad(total_len);
        let nonce = aes_gcm::Nonce::<U12>::from(Nonce::new(&self.iv, seq).0);

        let tag = self
            .cipher
            .encrypt_inout_detached(&nonce, &aad, payload.as_mut().into())
            .map_err(|_| Error::EncryptError)?;
        payload.extend_from_slice(&tag);

        Ok(OutboundOpaqueMessage::new(
            ContentType::ApplicationData,
            // Every TLS 1.3 record carries 0x0303 as its legacy record version, whatever it
            // actually is (RFC 8446 section 5.1).
            ProtocolVersion::TLSv1_2,
            payload,
        ))
    }

    fn encrypted_payload_len(&self, payload_len: usize) -> usize {
        payload_len + 1 + TAG_LEN
    }
}

struct Decrypter<C> {
    cipher: C,
    iv: Iv,
}

impl<C> MessageDecrypter for Decrypter<C>
where
    C: AeadInOut + AeadCore<NonceSize = U12, TagSize = U16> + Send + Sync + 'static,
{
    fn decrypt<'a>(
        &mut self,
        mut msg: InboundOpaqueMessage<'a>,
        seq: u64,
    ) -> Result<InboundPlainMessage<'a>, Error> {
        let payload = &mut msg.payload;
        if payload.len() < TAG_LEN {
            return Err(Error::DecryptError);
        }

        // The additional data covers the record as it arrived, tag included, so it is computed
        // before the tag is split off.
        let aad = make_tls13_aad(payload.len());
        let nonce = aes_gcm::Nonce::<U12>::from(Nonce::new(&self.iv, seq).0);

        let cipher_len = payload.len() - TAG_LEN;
        let (ciphertext, tag) = payload.split_at_mut(cipher_len);
        let tag = aes_gcm::Tag::<U16>::try_from(&*tag).map_err(|_| Error::DecryptError)?;

        self.cipher
            .decrypt_inout_detached(&nonce, &aad, ciphertext.into(), &tag)
            .map_err(|_| Error::DecryptError)?;

        payload.truncate(cipher_len);
        // `rustls` strips the padding and reads the real content type back off the end. Left to it
        // rather than done here: that is parsing of its own wire format, not cryptography.
        msg.into_tls13_unpadded_message()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `USIZE` lives on `Unsigned`, typenum's trait for a compile-time number. Imported rather than
    // hard-coding 16 twice, which is what this test exists to stop.
    use aes_gcm::aead::array::typenum::Unsigned;

    /// **The tag length is asserted rather than trusted**, because `TAG_LEN` is a constant this
    /// file wrote and everything above sizes buffers with it. A cipher whose tag were a different
    /// length would produce records that are silently the wrong shape.
    #[test]
    fn every_cipher_agrees_with_the_tag_length_this_file_assumes() {
        assert_eq!(<aes_gcm::Aes128Gcm as AeadCore>::TagSize::USIZE, TAG_LEN);
        assert_eq!(<aes_gcm::Aes256Gcm as AeadCore>::TagSize::USIZE, TAG_LEN);
        assert_eq!(
            <chacha20poly1305::ChaCha20Poly1305 as AeadCore>::TagSize::USIZE,
            TAG_LEN
        );
    }

    /// The key lengths this provider advertises are the ones the ciphers want.
    #[test]
    fn every_key_length_advertised_is_the_one_the_cipher_takes() {
        use aes_gcm::KeySizeUser;
        assert_eq!(
            <aes_gcm::Aes128Gcm as KeySizeUser>::KeySize::USIZE,
            AES128_GCM.key_len
        );
        assert_eq!(
            <aes_gcm::Aes256Gcm as KeySizeUser>::KeySize::USIZE,
            AES256_GCM.key_len
        );
        assert_eq!(
            <chacha20poly1305::ChaCha20Poly1305 as KeySizeUser>::KeySize::USIZE,
            CHACHA20_POLY1305.key_len
        );
    }
}
