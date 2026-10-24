//! **A package fetched through the distribution's index, by host name, over pinned TLS, and
//! believed only by the index's digest** (milestone 801 (packages over the internet), items 1, 3
//! and 4).
//!
//! The kernel plays the spawner: `net_stack` over the `e1000e`, `domain_name_resolver` asking the
//! runners' name server, a badge granted the root zone (`package_index::fixture::ZONE`, as `jig`'s
//! is under calef's Q5), and `package_fetch_exerciser` holding that badge, the network, a clock and
//! entropy. The repository, inside slirp, is `helpers/tls-peer` as `basalt.test`, the image's
//! backup index address, for `basalt.nifeos.org`. It skips only where the program was not built,
//! like `pinned_tls_tests` (milestone 855 (the TLS graph enters the gated build)).

use super::name_resolver_tests::{entropy, grant, start_resolver};
use super::*;

/// The badge the program's resolver capability carries.
const GRANTED: u32 = 0x8010;

const NO_PROGRAM: &str = "no package_fetch_exerciser in this archive: `cargo xtask test` builds \
     it, as does helpers/build-pinned-tls-exerciser.sh (milestone 801)";

/// **The index arrives over TLS from the image's backup address once the first is lost, a listed
/// location on a private address is refused, the repository's own copy is fetched instead and
/// admitted by the index's digest, an altered copy is refused, and a name the index does not list
/// fetches nothing** (calef's Q1, Q2, Q4 and Q5 of 2026-10-10 (UTC) on #1884).
///
/// Only the index's digest can refuse the altered copy, a correct HTTPS exchange of a well-formed
/// package. Row 35 of `notes/confinement-claims.md` cites this test, and
/// notes/packages/over-the-internet.md, "The index fetch test", has its hand attestations.
///
/// Falsification: replayable `system_tests/falsifications/user.package_index_tests.a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest.patch`
#[test_case]
fn a_package_is_fetched_through_the_index_by_name_over_tls_and_judged_by_its_digest() {
    let Some(exerciser) = program("package_fetch_exerciser") else {
        crate::testing::skip!(NO_PROGRAM);
    };
    use core::sync::atomic::Ordering;

    use crate::arch::exceptions::USER_FAULTS;

    let Some(entropy) = entropy() else {
        crate::testing::skip!("no entropy source on this machine, and the resolver needs one");
    };
    let image = |name: &str| program(name).unwrap_or_else(|| panic!("no {name} in the archive"));
    let mut w = wired_or_skip!(e1000e_service::start_net_server(
        image("net_stack"),
        socket_contract::NO_LISTEN_GRANT,
    ));
    let _ = crate::sched::ipc_receive(w.report);
    let service = start_resolver(w.stack, entropy, &mut w.held);
    grant(service, GRANTED, package_index::fixture::ZONE);

    let faults_before = USER_FAULTS.load(Ordering::Relaxed);
    let spawned = std_service::start_networked_resolving(
        exerciser,
        image("clock"),
        image("entropy"),
        w.stack,
        Some((service, &[GRANTED])),
    );
    let run = &spawned.run;
    let mut got = [0u8; 4096];
    let len = super::std_tests::drain_sink(run.report, &mut got, "package_fetch_exerciser");
    let text = core::str::from_utf8(&got[..len]).unwrap_or("<not utf-8>");
    crate::println!("    package_fetch_exerciser printed {len} bytes:\n{text}");

    every_step_was_printed_in_order(text);
    the_rebinding_probe_reached_no_private_peer(text);
    assert!(
        super::wait_for(|| !crate::sched::is_thread_present(run.thread)),
        "package_fetch_exerciser never left",
    );
    assert_eq!(
        USER_FAULTS.load(Ordering::Relaxed),
        faults_before,
        "package_fetch_exerciser trapped instead of exiting",
    );
    let _ = crate::sched::reclaim_region(spawned.frames);
    run.give_back("package_fetch_exerciser");
    w.held
        .release_or_fail("net_stack and domain_name_resolver, for the package index client");
}

/// The exerciser's report, `text`, holds every step's line in order: the index, its refusals, the
/// fetch, the six redirect probes through a second test authority (one fetched and five refused,
/// falsified in `notes/packages/the-index-format.md`, question C, per C2 on #2048), and the final
/// line last. The altered copy must be refused as `NotCataloged`, which only the index's digest
/// gives; the package's own table of contents would say `MemberMismatch`.
fn every_step_was_printed_in_order(text: &str) {
    let arch = crate::arch::NAME;
    let mut passed_over = [0u8; 128];
    let mut fetched = [0u8; 64];
    let mut refused = [0u8; 64];
    let lines: [&str; 8] = [
        "package_fetch_exerciser start",
        "index unreachable at gone.basalt.test",
        "from basalt.test over TLS",
        format_into(
            &mut passed_over,
            &[
                "passed over https://packages.basalt.test:8443/rolling/targets/greeting-0.1.0-",
                arch,
                ".nifepkg: a private address (10.0.2.9)",
            ],
        ),
        format_into(
            &mut fetched,
            &[
                "fetched greeting-0.1.0-",
                arch,
                " from basalt.test's targets",
            ],
        ),
        format_into(
            &mut refused,
            &["refused uptime-0.1.0-", arch, ": NotCataloged"],
        ),
        "refused nosuch: not in the index",
        "package_fetch_exerciser done",
    ];
    // calef's C2 (2026-10-12 UTC, #2048): a listed location naming the second root, one redirect.
    let mut redirected = [0u8; 112];
    let redirected = format_into(
        &mut redirected,
        &[
            "redirect releases: fetched greeting-0.1.0-",
            arch,
            " from basalt.test via releases.basalt.test",
        ],
    );
    let probes: [&str; 6] = [
        redirected,
        // Without its root named, the index's root does not admit github.com's stand-in.
        "redirect unnamed: refused: Tls(InvalidCertificate(",
        "redirect uncarried: refused: it names a root this image does not carry",
        "redirect twice: refused: redirected to basalt.test, which redirected again",
        "redirect private: refused: redirected to private.basalt.test: a private address (10.0.2.2)",
        "redirect offroot: refused: redirected to offroot.basalt.test: Tls(InvalidCertificate(",
    ];
    let mut from = 0;
    let (before_done, done) = lines.split_at(lines.len() - 1);
    for line in before_done.iter().chain(probes.iter()).chain(done) {
        match text[from..].find(line) {
            Some(at) => from += at + line.len(),
            None => {
                panic!("package_fetch_exerciser never printed `{line}` (after what came before it)")
            }
        }
    }
}

/// The rebinding probe (milestone 871), read from the exerciser's report `text`.
/// `rebind.basalt.test` answers a public address first and the private peer after, and the
/// resolver has no cache, so a client that checked one answer and then connected by name would
/// reach the private peer. The client dials only the address its check passed
/// (`package_index::Location::check`); slirp has nothing at the public one.
fn the_rebinding_probe_reached_no_private_peer(text: &str) {
    let arch = crate::arch::NAME;
    // The rebinding probe (milestone 871): where the client's refusal of the listed location
    // under rebind.basalt.test landed. The line must exist, and its reason must not be a TLS
    // error: only a TLS server that answered can produce one, and the only TLS peer in slirp is
    // the private one the address check refused to name. Sought in the whole report, because it
    // prints between the lines `every_step_was_printed_in_order` reads and the final "done".
    let mut rebind = [0u8; 104];
    let prefix = format_into(
        &mut rebind,
        &[
            "passed over https://rebind.basalt.test:8443/rolling/targets/rebound-0.1.0-",
            arch,
            ".nifepkg: ",
        ],
    );
    let at = text.find(prefix).unwrap_or_else(|| {
        panic!("package_fetch_exerciser never printed `{prefix}...` (the rebinding probe)")
    });
    let reason = &text[at + prefix.len()..];
    let end = reason.find('\n').unwrap_or(reason.len());
    let reason = &reason[..end];
    assert!(
        !reason.contains("Tls("),
        "CONFINEMENT ESCAPE: the client reached the private peer at 10.0.2.9 through a listed \
         location whose checked resolution was public (192.0.2.1): a TLS error ({reason}) means a \
         server answered, and slirp's only TLS server is the private one, so the connection \
         disagreed with the check",
    );
    // The probe proves something only if the client's one resolution saw the PUBLIC answer: a
    // client refused at the check never dials, so neither the escape nor the fix is exercised.
    // That happened on the UEFI x86_64 leg, whose runner did not export the boot tag, so every
    // boot after the first shared one state file and got the private answer first (2026-10-10).
    assert!(
        !reason.starts_with("a private address"),
        "the rebinding probe proved nothing: the client's check saw the private answer ({reason}), \
         so the name server did not answer this boot's first query public. Is NIFE_BOOT_TAG \
         exported by this architecture's runner?",
    );
}

/// `parts` joined into `buf`, which a `no_std` test has instead of `format!`.
fn format_into<'b>(buf: &'b mut [u8], parts: &[&str]) -> &'b str {
    let mut at = 0;
    for part in parts {
        buf[at..at + part.len()].copy_from_slice(part.as_bytes());
        at += part.len();
    }
    core::str::from_utf8(&buf[..at]).unwrap_or("")
}
