//! The staged glob fixtures against the matcher: the sets the filesystem tests state literally are
//! only trustworthy if they are the expansion of the pattern they are staged for, so this checks
//! them with the matcher itself. It lives here, with the matcher, because the fixture and the
//! filesystem contract it rides on are crates that do not depend on the matcher.

/// **The set the globbing lane grants is exactly what the pattern matches**, checked against the
/// matcher rather than asserted twice.
///
/// The kernel test states its set literally (a `[(&[u8], bool); 2]` it hands to
/// [`filesystem_contract::nameset::encode`]), which is the only way to build a grant without an
/// enumeration. That literal is only trustworthy if it *is* the expansion, so this pins it: the pattern matches the
/// two names in the set and neither of the two controls, over the fixture as staged.
#[test]
fn the_glob_fixture_is_matched_by_the_pattern_it_is_staged_for() {
    use filesystem_contract::grant;
    use filesystem_fixture::tree;
    let p = tree::GLOB_PATTERN;
    for name in [tree::GLOB_ONE, tree::GLOB_TWO] {
        assert!(
            glob::matches(p, name.as_bytes()),
            "{name} is in the granted set and the pattern does not match it",
        );
    }
    for name in [tree::GLOB_MISS, tree::GLOB_DIR, tree::GLOB_INNER] {
        assert!(
            !glob::matches(p, name.as_bytes()),
            "{name} is a control and the pattern matches it, so it proves nothing",
        );
    }
    // Every name in the set has to be able to travel in one, or the grant cannot be built.
    for name in [
        tree::GLOB_ONE,
        tree::GLOB_TWO,
        tree::GLOB_MISS,
        tree::GLOB_DIR,
    ] {
        assert!(grant::fits(name.as_bytes()), "{name} does not fit a grant");
    }
}

/// **The batching fixture is over the bound, and the two batches printed in the gate are the
/// ones the rule produces** .
///
/// The shell gate asserts two literal strings at a real prompt, and a literal is only
/// worth asserting if it is the answer rather than a second opinion about it. So this pins all
/// three claims: the pattern matches every name and not the control, there are more names than
/// one grant can carry, and the batch boundary falls where [`filesystem_fixture::tree::MANY_FIRST_BATCH`]
/// says it does.
#[test]
fn the_batching_fixture_is_over_the_bound_and_splits_where_the_gate_says() {
    use filesystem_contract::{grant, nameset};
    use filesystem_fixture::tree;
    let p = tree::MANY_PATTERN;
    for name in tree::MANY_NAMES {
        assert!(
            glob::matches(p, name.as_bytes()),
            "{name} is staged for the sweep and the pattern does not match it",
        );
        assert!(grant::fits(name.as_bytes()), "{name} does not fit a grant");
    }
    assert!(
        !glob::matches(p, tree::MANY_MISS.as_bytes()),
        "the control matches, so 'the batches are the match' proves nothing",
    );
    assert!(
        tree::MANY_NAMES.len() > nameset::MAXIMUM_NAMES,
        "the fixture fits in one grant, so nothing here is batched",
    );
    // The names are staged in sorted order, which is what lets the two batches below be read off
    // the array rather than sorted at the assertion.
    assert!(tree::MANY_NAMES.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(
        tree::MANY_FIRST_BATCH,
        tree::MANY_NAMES[..nameset::MAXIMUM_NAMES].join(" "),
    );
    assert_eq!(
        tree::MANY_SECOND_BATCH,
        tree::MANY_NAMES[nameset::MAXIMUM_NAMES..].join(" "),
    );
}
