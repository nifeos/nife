# P2: later

An appendix to [the milestone](../1087-domain-name-system-outside-review.md).

## P2: for later

10. `Name` has `Debug` but no `Display`, and `from_dotted` but no `FromStr`.
    Finding: a stranger writes `name.to_string()` or `"a.test".parse::<Name>()` first, as both
    `hickory-proto` and `domain` allow (unverified). `Debug` prints the dotted form with master-file
    escapes (lines 315 to 342), which is what `Display` should print. There is no `Hash`, and
    with case-insensitive `Eq` there cannot be one without folding; the doc does not say so.
    Change: `impl Display` sharing the `Debug` body; `impl FromStr` delegating to `from_dotted`;
    one sentence under `Name` on why `Hash` is absent. Additive.
    Door: two-way. Effort: XS.

11. Type sizes and copies lack documentation. Finding: `Name` is 256 bytes and `Copy`.
    `Reader::record` returns a `Record` by value, so `accept` copies a `Name` once per record on
    each of its three passes over the answer section. `Answer` is 296 bytes and `Result<Answer,
    Reject>` 300. A caller on a one-page stack wants to know this before it puts a `Query` and an
    `Answer` beside a 514-byte buffer. Change: a sizes paragraph in the README, as milestone 1048's
    review did for `generational_table`; no code change. Door: two-way. Effort: XS.

12. `Error` and `Reject` are exhaustive. Finding: EDNS(0) or AAAA support would add variants to
    `Error`. Adding `#[non_exhaustive]` after a release is a breaking change, removing it is not.
    `Reject` is different: `domain_name_resolution_contract::reject_code` numbers each variant and
    wants the compile error a new one causes, and the `Reject` doc promises the list in check order.
    Change: `#[non_exhaustive]` on `Error` now; leave `Reject` exhaustive and say in its doc that
    the list is the contract. Two-way today, one-way at the tag. Door: two-way. Effort: XS.

13. The documentation drifts from the code in four places. Finding: line 6 says Kani checks the
    name decoder and the fuzz target the rest, while `proofs.rs` proves the header codec and TCP
    reassembly too. Line 622 says a request is at most 17 + 255 bytes, and `HEADER_LEN +
    MAXIMUM_NAME_BYTES + 4` is 271. Lines 262 to 263 say nothing applies `is_within` (item 4).
    `Record::rdata_off`'s doc on line 434 says the data "may be compressed against the whole
    message", which is true of CNAME data only, as line 433 says one sentence earlier. Change: three
    sentence edits and one number. Door: two-way. Effort: XS.

14. `Name::from_dotted` checks DNS label syntax, not host-name syntax, and does not say so.
    Finding: the crate accepts `-a.test` and `a-.test` (probed), and accepts `_` anywhere. RFC 952
    and RFC 1123 host names forbid a leading or trailing hyphen. For a resolver this is right (a DNS
    name may hold any byte, and `_` names are real: `_acme-challenge`), but a stranger expecting
    host-name validation gets none and learns that only from the BUGS list. Change: one sentence on
    `from_dotted`: it accepts the DNS label alphabet the crate chose, not RFC 1123 host-name rules.
    A caller that wants those checks them. Door: two-way. Effort: XS.

15. The captured fixtures cannot be re-captured from the tree. Finding: `tests/replies.rs` lines
    3 to 6 say the three `.dns` fixtures came from `1.1.1.1` with a twenty-line Python stub sending
    exactly the bytes `Query::request` writes. The stub is not in the tree. A stranger who wants a
    fourth fixture, or wants to check the three, has nothing to run. Change: a
    `tests/fixtures/README.md` or a header paragraph with the capture command (a `python3 -c`
    one-liner over a UDP socket, or `dig +noedns` with its raw output), and the id each fixture
    carries. Door: two-way. Effort: XS.

16. Callers size the TCP buffer from the UDP maximum. Finding: `TcpReply`'s doc says the buffer
    bounds the reply and says nothing about how large a TCP reply may be. RFC 7766 allows 65,535
    bytes. `components/src/socket_test_client.rs` line 409 gives `TcpReply` a `2 +
    MAXIMUM_UDP_MESSAGE_BYTES` buffer, so the TCP retry that the BUGS list promises for a truncated
    UDP reply can never hold a reply larger than UDP's. The resolver itself uses 2,048 bytes
    (`domain_name_resolver.rs` line 84) and is not affected. The test client is nife's to fix, not
    the crate's; the crate can stop the next caller making the same choice. Change: a
    `MAXIMUM_TCP_MESSAGE_BYTES: usize = 65_535` constant beside the UDP one, and one sentence on
    `TcpReply::new` saying a buffer no larger than the UDP maximum defeats the retry. Then a nife
    follow-up for the test client. Door: two-way (additive). Effort: XS.
