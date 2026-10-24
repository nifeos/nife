//! **A TLS client that trusts one root for one host name, and nothing else**, for milestone 501
//! (a TLS client that speaks to one pinned peer).
//!
//! A [`Session`] is made only from a [`PinnedPeer`], one host name and one root: there is no root
//! store to add to, no verifier to swap and no "accept anyway" switch. TLS 1.3 only, through
//! `cryptography_provider`. The shape is §196 (nife carries TLS) clause 4's, one root for the one
//! source, and the pair is §250 (an image names its distribution's package index)'s: ISRG Root X1
//! for `basalt.nifeos.org`. A package's integrity is its digest's under §195 (a reviewed recipe
//! vouches for a package), not TLS's. See notes/packages/pinned-tls.md for each check and who
//! makes it.

//! # BUGS
//!
//! - Certificate expiry is checked against `std`'s clock, which nobody can vouch for; a clock
//!   held back accepts an expired chain (notes/packages/pinned-tls.md).
//! - No rotation story: when the pinned root is retired, every image carrying it loses its index.
//!   See `PinnedPeer::basalt_index`.
//! - One request per connection, HTTP/1.0, `Content-Length` required; only [`listed`] follows a
//!   redirect, on a new session with its own pin.
//! - The caller connects the stream, and the name verified is the pin's, never the resolver's.

use std::io::{self, Read, Write};
use std::sync::Arc;

use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

/// ISRG Root X1, the root §250 (an image names its distribution's package index) pins for basalt's
/// index host, as DER.
///
/// Fetched 2026-10-06 (UTC) from `https://letsencrypt.org/certs/isrgrootx1.der` and compared byte
/// for byte with the copy in macOS's `SystemRootCertificates.keychain`: identical. Its SHA-256 is
/// [`ISRG_ROOT_X1_SHA256`], which a test recomputes, so a substituted file fails a test rather than
/// a handshake on somebody's machine.
pub const ISRG_ROOT_X1: &[u8] = include_bytes!("../roots/isrg-root-x1.der");

/// The SHA-256 of [`ISRG_ROOT_X1`]: `96:BC:EC:06:...:08:C6`, the fingerprint Let's Encrypt
/// publishes for it.
pub const ISRG_ROOT_X1_SHA256: [u8; 32] = [
    0x96, 0xbc, 0xec, 0x06, 0x26, 0x49, 0x76, 0xf3, 0x74, 0x60, 0x77, 0x9a, 0xcf, 0x28, 0xc5, 0xa7,
    0xcf, 0xe8, 0xa3, 0xc0, 0xaa, 0xe1, 0x1a, 0x8f, 0xfc, 0xee, 0x05, 0xc0, 0xbd, 0xdf, 0x08, 0xc6,
];

/// **Sectigo Public Server Authentication Root E46**, as DER: the root `github.com` chains to, and
/// so the root a GitHub Release asset's download URL needs before it redirects to an X1 host.
/// calef ruled on 2026-10-12 (UTC) on #2048 (*"a"*, option C2 of `notes/packages/the-index-format.md`)
/// that the signed index may name it for a listed location; it is never a root for an index host.
///
/// Read 2026-10-13 (UTC) from macOS's `SystemRootCertificates.keychain` and compared byte for byte
/// with `http://crt.sectigo.com/SectigoPublicServerAuthenticationRootE46.crt`: identical. Its key
/// is the cross-signed E46's that `github.com` sends; valid 2021-03-22 to 2046-03-21. A test
/// recomputes [`SECTIGO_ROOT_E46_SHA256`].
///
/// Name: provisional, minted 2026-10-13 (UTC) by lane `lane/basalt-on-pages-and-releases`.
pub const SECTIGO_ROOT_E46: &[u8] =
    include_bytes!("../roots/sectigo-public-server-authentication-root-e46.der");

/// The SHA-256 of [`SECTIGO_ROOT_E46`]: `C9:0F:26:F0:...:53:83`. Name: provisional, as above.
pub const SECTIGO_ROOT_E46_SHA256: [u8; 32] = [
    0xc9, 0x0f, 0x26, 0xf0, 0xfb, 0x1b, 0x40, 0x18, 0xb2, 0x22, 0x27, 0x51, 0x9b, 0x5c, 0xa2, 0xb5,
    0x3e, 0x2c, 0xa5, 0xb3, 0xbe, 0x5c, 0xf1, 0x8e, 0xfe, 0x1b, 0xef, 0x47, 0x38, 0x0c, 0x53, 0x83,
];

/// basalt's index host, from §250.
pub const BASALT_INDEX_HOST: &str = "basalt.nifeos.org";

/// One host name and the one root its certificate must chain to. The whole of what a [`Session`]
/// trusts.
#[derive(Clone, Debug)]
pub struct PinnedPeer {
    name: ServerName<'static>,
    roots: Arc<RootCertStore>,
}

impl PinnedPeer {
    /// Pin `root_der` (one certificate, DER) as the only root for `host_name`. Refuses a host name
    /// that is not a DNS name (an IP address is legal for TLS and is not what a pin names here) and
    /// a root that does not parse as a trust anchor. A test pins its own root, as `tests/peer.rs`
    /// and `pinned_tls_exerciser` do to reach `helpers/tls-peer`:
    /// ```
    /// let root = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/pinned-root.der")).unwrap();
    /// let peer = pinned_tls_client::PinnedPeer::new("basalt.test", &root).unwrap();
    /// assert_eq!(peer.host_name(), "basalt.test");
    /// ```
    pub fn new(host_name: &str, root_der: &[u8]) -> Result<Self, Error> {
        let name = ServerName::try_from(host_name.to_owned()).map_err(|_| Error::HostName)?;
        if !matches!(name, ServerName::DnsName(_)) {
            return Err(Error::HostName);
        }
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from(root_der.to_vec()))
            .map_err(|_| Error::Root)?;
        Ok(Self {
            name,
            roots: Arc::new(roots),
        })
    }

    /// **The production pin**: [`BASALT_INDEX_HOST`] and [`ISRG_ROOT_X1`], §250's clause 3.
    ///
    /// Measured 2026-10-06 (UTC), and it changes what pinning X1 means. Let's Encrypt now issues
    /// from its newer roots: `letsencrypt.org` presented leaf, `YE2`, `Root YE`, then `ISRG Root X2`
    /// cross-signed by X1; a GitHub Pages host (`*.github.io`, the host §250 names for
    /// `nifeos/basalt`) presented leaf, `YR1`, then `Root YR` cross-signed by X1. Both reach X1,
    /// so the pin works today, but only because the server sends the cross-signatures, and both
    /// cross-signatures say they end on 2032-09-02. Past that, or on a host that stops sending
    /// them, this pin refuses a correct server. Recorded in milestone 501's `BUGS`.
    pub fn basalt_index() -> Self {
        Self::new(BASALT_INDEX_HOST, ISRG_ROOT_X1).expect("ISRG Root X1 is a valid trust anchor")
    }

    /// The host name the certificate must carry, which is also the name sent as SNI.
    pub fn host_name(&self) -> &str {
        match &self.name {
            ServerName::DnsName(n) => n.as_ref(),
            _ => unreachable!("PinnedPeer::new admits DNS names only"),
        }
    }

    /// The `rustls` configuration this pin makes: TLS 1.3, this tree's provider, one root, and no
    /// client certificate.
    fn config(&self) -> Arc<ClientConfig> {
        let provider = Arc::new(cryptography_provider::provider());
        let config = ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])
            .expect("cryptography_provider offers TLS 1.3 suites")
            .with_root_certificates(self.roots.clone())
            .with_no_client_auth();
        Arc::new(config)
    }
}

/// Why a fetch did not produce a response. Every variant fails closed: nothing a caller receives
/// came from a peer this pin did not admit.
#[derive(Debug)]
pub enum Error {
    /// The host name is not a DNS name.
    HostName,
    /// The root is not a certificate `rustls-webpki` accepts as a trust anchor.
    Root,
    /// The handshake or a record was refused. A certificate from another root, for another
    /// name, or out of date lands here as `rustls::Error::InvalidCertificate`.
    Tls(rustls::Error),
    /// The transport failed underneath TLS.
    Io(io::Error),
    /// The response was not one `http_response` reads.
    Http(http_response::Error),
    /// The connection ended before the body `Content-Length` promised.
    Truncated,
    /// The request line did not fit (a path longer than the request buffer).
    RequestTooLong,
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        // `rustls::Stream` reports a TLS failure as an `io::Error` wrapping the `rustls::Error`;
        // unwrap it, so a refused certificate reads as a refused certificate and not as a socket
        // that misbehaved.
        match e
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>())
        {
            Some(tls) => Error::Tls(tls.clone()),
            None => Error::Io(e),
        }
    }
}

/// An established TLS 1.3 session with a pinned peer, over any byte stream.
pub struct Session<S: Read + Write> {
    host: String,
    tls: StreamOwned<ClientConnection, S>,
}

impl<S: Read + Write> Session<S> {
    /// Run the handshake to completion over `stream`, refusing any peer the pin does not admit;
    /// not lazily, so a caller can time it apart from the transfer (notes/packages/pinned-tls.md).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pinned_tls_client::{PinnedPeer, Session};
    /// use std::net::TcpStream;
    ///
    /// // The production pin: basalt's index host and the one root it must chain to (§250).
    /// let peer = PinnedPeer::basalt_index();
    /// // The caller resolves the name and connects the stream.
    /// let tcp = TcpStream::connect("192.0.2.1:443").unwrap();
    /// let mut session = Session::handshake(&peer, tcp).unwrap();
    /// let mut index = Vec::new();
    /// let status = session.get("/index", |body| index.extend_from_slice(body)).unwrap();
    /// assert_eq!(status, 200);
    /// ```
    pub fn handshake(peer: &PinnedPeer, stream: S) -> Result<Self, Error> {
        let conn = ClientConnection::new(peer.config(), peer.name.clone()).map_err(Error::Tls)?;
        let mut tls = StreamOwned::new(conn, stream);
        while tls.conn.is_handshaking() {
            tls.conn.complete_io(&mut tls.sock)?;
        }
        Ok(Self {
            host: peer.host_name().to_owned(),
            tls,
        })
    }

    /// The negotiated cipher suite, for a caller that reports what it spoke.
    pub fn cipher_suite(&self) -> Option<rustls::CipherSuite> {
        self.tls.conn.negotiated_cipher_suite().map(|s| s.suite())
    }

    /// The negotiated key exchange group.
    pub fn key_exchange_group(&self) -> Option<rustls::NamedGroup> {
        self.tls
            .conn
            .negotiated_key_exchange_group()
            .map(|g| g.name())
    }

    /// Send one `GET path` and hand the body to `body` as it arrives, never holding it. Returns
    /// the status; the caller decides what a status other than 200 means. Consumes the session,
    /// because `http_response` speaks one request per connection.
    pub fn get(self, path: &str, body: impl FnMut(&[u8])) -> Result<u16, Error> {
        self.fetch(path, body).map(|reply| reply.status)
    }

    /// **[`Session::get`], and the `Location` of a redirect.** The body reaches `body` only when
    /// the status is 200, so a redirect's own body (GitHub's is empty) is read and dropped rather
    /// than mistaken for the package. The head may be up to
    /// `http_response::REDIRECT_HEAD_MAX` bytes, because `github.com`'s `302` is 5,291.
    ///
    /// Name: provisional, minted 2026-10-13 (UTC) by lane `lane/basalt-on-pages-and-releases`,
    /// with [`Reply`].
    pub fn fetch(mut self, path: &str, mut body: impl FnMut(&[u8])) -> Result<Reply, Error> {
        // Sized for the path: a redirect's target can be a 930-byte signed URL (GitHub's, measured
        // 2026-10-13 UTC), and the bound that matters is the caller's, not a fixed buffer here.
        let mut request = vec![0u8; path.len() + self.host.len() + 64];
        let n = http_response::get_request(&self.host, path, &mut request)
            .ok_or(Error::RequestTooLong)?;
        self.tls.write_all(&request[..n])?;
        self.tls.flush()?;

        let mut response = Box::new(http_response::Response::<
            { http_response::REDIRECT_HEAD_MAX },
        >::with_room());
        let mut buf = vec![0u8; 16 * 1024];
        while !response.is_complete() {
            let got = match self.tls.read(&mut buf) {
                Ok(0) => return Err(Error::Truncated),
                Ok(got) => got,
                // A peer that hangs up without close_notify after a complete body is harmless,
                // but this loop only gets here before the body is complete, so it is a truncation.
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(Error::Truncated),
                Err(e) => return Err(e.into()),
            };
            let part = response.feed(&buf[..got]).map_err(Error::Http)?;
            if response.status() == Some(200) {
                body(part);
            }
        }
        let status = response.status().ok_or(Error::Truncated)?;
        let location = if (300..400).contains(&status) {
            response
                .header(b"location")
                .and_then(|l| core::str::from_utf8(l).ok())
                .map(str::to_owned)
        } else {
            None
        };
        // Say goodbye properly; a peer that is already gone does not change the answer.
        self.tls.conn.send_close_notify();
        let _ = self.tls.conn.complete_io(&mut self.tls.sock);
        Ok(Reply { status, location })
    }
}

/// What [`Session::fetch`] read besides the body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reply {
    /// The status code.
    pub status: u16,
    /// The `Location` header of a `3xx`, as sent, when it is UTF-8. `None` for any other status.
    pub location: Option<String>,
}

// Inline rather than a child file, so this file stays one module and not a split parent (§282
// (when a file is split, the parent becomes an index)).
/// **A package's bytes from a location the index listed, through at most one redirect** (calef's
/// C2, ruled *"a"* on #2048, 2026-10-12 UTC; `notes/packages/the-index-format.md`, question C).
///
/// The listed hop is pinned to the root the signed index named for it, among those the image
/// carries (`package_index::Sources::root_for`). The one redirect, an absolute `https://` URL
/// (`package_index::Location::redirect`), is pinned to the index's own root (X1 in production),
/// whatever the first hop's was; a second is refused. Every hop dials only what Q1's check passed.
/// The caller judges the bytes by the index's digest. `jig` and `package_fetch_exerciser` both use
/// `listed::fetch`, so the boot test proves `jig`'s path (notes/packages/pinned-tls.md).
///
/// Name: provisional, minted 2026-10-13 (UTC) by lane `lane/basalt-on-pages-and-releases`, with
/// everything in it.
pub mod listed {
    use std::net::{SocketAddr, TcpStream, ToSocketAddrs};

    use package_index::{AddressRefusal, Location, MAX_REDIRECTS, Root};

    use crate::{ISRG_ROOT_X1, PinnedPeer, SECTIGO_ROOT_E46, Session};

    /// **A root as DER**: one this client carries, by the name an image's sources and a signed index
    /// use (`isrg-root-x1`, `sectigo-root-e46`), or the hex an image gave. Every name here is one
    /// calef ruled a client carries: X1 for the index (§250 (an image names its distribution's package index)),
    /// E46 for a listed location (C2, #2048).
    pub fn root_der(root: &Root<'_>) -> Result<Vec<u8>, &'static str> {
        match root {
            Root::Named("isrg-root-x1") => Ok(ISRG_ROOT_X1.to_vec()),
            Root::Named("sectigo-root-e46") => Ok(SECTIGO_ROOT_E46.to_vec()),
            Root::Named(_) => Err("a root this client does not carry"),
            Root::Der(_) => {
                let mut out = vec![0u8; 4096];
                let n = root
                    .der(&mut out)
                    .ok_or("a root that cannot be decoded")?
                    .len();
                out.truncate(n);
                Ok(out)
            }
        }
    }

    /// **An I/O error as a client prints it**: its kind, then its message when it carries one.
    /// The kind alone hides which of the name resolver's four network steps refused it
    /// (`NetworkUnreachable` covers all four, and the std PAL names the step in the message), and
    /// a bench transcript is often the only evidence there is. A caller prefixes a failed lookup
    /// with `not resolved: `, so a `TimedOut` says whether the name server or the peer was silent.
    ///
    /// Name: provisional, minted 2026-10-18 (UTC) by lane `lane/jig-resolves-by-name`.
    pub fn reason(e: &std::io::Error) -> String {
        let kind = format!("{:?}", e.kind());
        let text = e.to_string();
        if text == e.kind().to_string() {
            kind
        } else {
            format!("{kind} ({text})")
        }
    }

    /// Where the bytes came from.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct Fetched {
        /// The host that sent them.
        pub host: String,
        /// The listed host that redirected there, when one did.
        pub redirected_from: Option<String>,
    }

    /// **Fetch from `at`**, its first hop pinned to `root` and a redirect's target to
    /// `redirect_root`, every address judged by `public` (`package_index::public_address`, except in
    /// the boot test: see `package_index::Location::check_by`). The body of the one `200` goes to
    /// `body`; no redirect's body ever does. The error is why, in words a client prints.
    pub fn fetch(
        at: &Location<'_>,
        root: &[u8],
        redirect_root: &[u8],
        public: fn([u8; 4]) -> bool,
        body: &mut dyn FnMut(&[u8]),
    ) -> Result<Fetched, String> {
        let Hop::Redirect(mut to) = hop(at, root, public, body)? else {
            return Ok(Fetched {
                host: at.host.to_owned(),
                redirected_from: None,
            });
        };
        let mut followed = 0;
        loop {
            let next = Location::redirect(&to)
                .map_err(|why| format!("a redirect to {} refused: {why}", shown(&to)))?;
            followed += 1;
            match hop(&next, redirect_root, public, body)
                .map_err(|why| format!("redirected to {}: {why}", next.host))?
            {
                Hop::Bytes => {
                    return Ok(Fetched {
                        host: next.host.to_owned(),
                        redirected_from: Some(at.host.to_owned()),
                    });
                }
                Hop::Redirect(_) if followed == MAX_REDIRECTS => {
                    return Err(format!(
                        "redirected to {}, which redirected again: {MAX_REDIRECTS} redirect is followed, \
                         no more",
                        next.host
                    ));
                }
                Hop::Redirect(again) => to = again,
            }
        }
    }

    /// A redirect's URL as a person reads it: the scheme and host, not a kilobyte of signature.
    fn shown(url: &str) -> &str {
        let scheme = url.len() - url.strip_prefix("https://").unwrap_or(url).len();
        let end = url[scheme..].find('/').map_or(url.len(), |i| scheme + i);
        &url[..end]
    }

    enum Hop {
        Bytes,
        Redirect(String),
    }

    /// One `GET` at one location: resolved once, checked, dialed only at what passed, pinned to `root`.
    fn hop(
        at: &Location<'_>,
        root: &[u8],
        public: fn([u8; 4]) -> bool,
        body: &mut dyn FnMut(&[u8]),
    ) -> Result<Hop, String> {
        let resolved: Vec<SocketAddr> = (at.host, at.port)
            .to_socket_addrs()
            .map_err(|e| format!("not resolved: {}", reason(&e)))?
            .collect();
        let checked = at.check_by(&resolved, public).map_err(|why| match why {
            AddressRefusal::Private(ip) => format!("a private address ({ip})"),
            other => format!("{other:?}"),
        })?;
        let peer = PinnedPeer::new(checked.host(), root).map_err(|e| format!("{e:?}"))?;
        let tcp = TcpStream::connect(checked.addresses()).map_err(|e| reason(&e))?;
        let session = Session::handshake(&peer, tcp).map_err(|e| format!("{e:?}"))?;
        let reply = session
            .fetch(checked.path(), |part| body(part))
            .map_err(|e| format!("{e:?}"))?;
        match reply.status {
            200 => Ok(Hop::Bytes),
            301 | 302 | 303 | 307 | 308 => reply
                .location
                .map(Hop::Redirect)
                .ok_or_else(|| format!("status {} with no Location", reply.status)),
            status => Err(format!("status {status}")),
        }
    }
}
