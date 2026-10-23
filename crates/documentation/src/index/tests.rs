use super::*;

/// The one property that makes a lookup possible at all: the builder's tokeniser and the
/// reader's query normaliser must agree, byte for byte, on what a term is.
#[test]
fn a_query_folds_the_way_the_text_did() {
    let mut buf = [0u8; TERM_MAX];
    let n = normalize(b"Capability!", &mut buf);
    assert_eq!(&buf[..n], b"capability");

    let mut seen: [[u8; TERM_MAX]; 4] = [[0; TERM_MAX]; 4];
    let mut lens = [0usize; 4];
    let mut i = 0;
    tokens(b"Capability, a `Frame`. x", |t| {
        if i < 4 {
            seen[i][..t.len()].copy_from_slice(t);
            lens[i] = t.len();
            i += 1;
        }
    });
    // Two terms out of four words: `a` and `x` are single characters, and a per-letter posting
    // list answers no question anybody asks a manual.
    assert_eq!(i, 2);
    assert_eq!(&seen[0][..lens[0]], b"capability");
    assert_eq!(&seen[1][..lens[1]], b"frame");
}

/// **The underscore case of the test above.** `normalize` drops every non-alphanumeric byte, so
/// a reader typing `line_editor` looks up `lineeditor`, and the tokeniser must have written that
/// term. See notes/documentation/underscore-terms.md.
#[test]
fn an_underscore_name_is_searchable_as_itself() {
    let mut key = [0u8; TERM_MAX];
    let n = normalize(b"line_editor", &mut key);
    assert_eq!(&key[..n], b"lineeditor");

    let mut seen: [[u8; TERM_MAX]; 8] = [[0; TERM_MAX]; 8];
    let mut lens = [0usize; 8];
    let mut i = 0;
    tokens(b"the line_editor crate", |t| {
        if i < 8 {
            seen[i][..t.len()].copy_from_slice(t);
            lens[i] = t.len();
            i += 1;
        }
    });
    // No `alloc` here: this module compiles without it on the guest path, so the check is a
    // closure over the fixed array rather than a `Vec`.
    let has = |want: &[u8]| (0..i).any(|k| &seen[k][..lens[k]] == want);
    let count = |want: &[u8]| (0..i).filter(|&k| &seen[k][..lens[k]] == want).count();
    assert!(has(b"lineeditor"));
    // The halves are still terms of their own, so `apropos editor` keeps working.
    assert!(has(b"line"));
    assert!(has(b"editor"));
    // And an ordinary word is not offered twice, which is what keeps the term table from
    // doubling over prose that has no underscores in it.
    assert_eq!(count(b"crate"), 1);
}

#[test]
fn a_term_longer_than_the_record_folds_to_its_prefix() {
    let long = b"averyveryverylongidentifiername";
    let mut buf = [0u8; TERM_MAX];
    assert_eq!(normalize(long, &mut buf), TERM_MAX);
}

#[test]
fn a_header_that_is_not_one_is_refused() {
    assert_eq!(Header::parse(&[0u8; 64]), Err(Error::BadMagic));
    assert_eq!(Header::parse(&[0u8; 8]), Err(Error::Truncated));
}

/// A reader that ignored the version would go on to binary-search a term table whose record
/// size it is guessing.
#[test]
fn a_shard_from_another_version_is_refused_rather_than_read() {
    let mut first = [0u8; 64];
    first[..8].copy_from_slice(&MAGIC);
    first[8..12].copy_from_slice(&VERSION.to_le_bytes());
    assert!(Header::parse(&first).is_ok());
    first[8..12].copy_from_slice(&(VERSION + 1).to_le_bytes());
    assert_eq!(
        Header::parse(&first),
        Err(Error::BadVersion { found: VERSION + 1 })
    );
}

#[test]
fn the_manifest_is_one_bundle_per_line() {
    let mut seen = [[0u8; 8]; 4];
    let mut lens = [0usize; 4];
    let mut i = 0;
    // A blank line and a stray carriage return, because the manifest is a text file a person
    // may have opened.
    bundles(b"manual\r\nswish\n\nglob\n", |b| {
        if i < 4 {
            seen[i][..b.len()].copy_from_slice(b);
            lens[i] = b.len();
            i += 1;
        }
    });
    assert_eq!(i, 3);
    assert_eq!(&seen[0][..lens[0]], b"manual");
    assert_eq!(&seen[1][..lens[1]], b"swish");
    assert_eq!(&seen[2][..lens[2]], b"glob");
}

#[cfg(feature = "builder")]
#[test]
fn every_term_the_builder_wrote_is_found_by_the_reader() {
    use alloc::vec::Vec;

    // Enough terms to make the term table span several pages, because the binary search over
    // pages is the part that a single-page index would never exercise. `TERM_REC` is 32 bytes
    // and a page holds 128 records, so this is four pages of terms.
    let mut text = alloc::string::String::new();
    let mut want: Vec<alloc::string::String> = Vec::new();
    for i in 0..500 {
        let t = alloc::format!("term{i:04}");
        text.push_str(&t);
        text.push(' ');
        want.push(t);
    }
    let bytes = build(&[Source {
        path: "notes/x.md",
        title: "X",
        text: text.as_bytes(),
    }]);
    let h = Header::parse(&bytes[..PAGE]).unwrap();
    assert_eq!(h.terms as usize, want.len());

    let mut src = Slice(&bytes);
    for t in &want {
        let hit = lookup(&h, t.as_bytes(), &mut src)
            .unwrap_or_else(|| panic!("the reader cannot find {t}, which the builder wrote"));
        assert_eq!(hit.count, 1);
    }
    assert!(lookup(&h, b"absent", &mut src).is_none());
}

/// The property the whole layout exists for: a reader that holds one page never sees half a
/// record. It is a claim about the offsets, so it is checked on the offsets.
#[cfg(feature = "builder")]
#[test]
fn a_record_never_straddles_a_page() {
    let bytes = build(&[Source {
        path: "a",
        title: "A",
        text: b"one two three",
    }]);
    let h = Header::parse(&bytes[..PAGE]).unwrap();
    for off in [h.page_off, h.term_off, h.post_off] {
        assert_eq!(off % PAGE as u64, 0, "section at {off} is not page aligned");
    }
    for rec in [PAGE_REC, TERM_REC, POST_REC] {
        assert_eq!(PAGE % rec, 0, "{rec}-byte records do not divide a page");
    }
    assert_eq!(h.total as usize, bytes.len());
}

/// Two bundles, as the store really has: each is built and searched on its own, and the
/// answer is the merge. This is the roadmap's "the index is a merge of shards", and it is
/// also the property that makes installing a component make its documentation searchable
/// without a reindex pass.
#[cfg(feature = "builder")]
#[test]
fn a_search_merges_two_shards_and_ranks_them_by_count() {
    let swish = build(&[Source {
        path: "notes/pipes.md",
        title: "Pipes and redirection",
        text: b"capability capability capability pipe",
    }]);
    let glob = build(&[Source {
        path: "notes/glob.md",
        title: "The glob matcher",
        text: b"capability pattern",
    }]);

    let mut r = Ranked::new();
    search(b"swish", b"capability", &mut Slice(&swish), &mut r).unwrap();
    search(b"glob", b"capability", &mut Slice(&glob), &mut r).unwrap();

    let out = r.results();
    assert_eq!(out.len(), 2);
    assert_eq!(r.offered(), 2);
    // Strongest first, across shards, and the shard searched second wins because it matched
    // more strongly. A merge that just concatenated would answer in manifest order.
    assert_eq!(out[0].count, 3);
    assert_eq!(out[0].title(), b"Pipes and redirection");
    // **The name a reader can type**, derived from the shard the searcher opened rather than
    // read out of it, so no byte of the layout carries it.
    assert_eq!(out[0].location(), b"doc/swish/pipes.md");
    // And the provenance, which is what the index does store.
    assert_eq!(out[0].origin(), b"notes/pipes.md");
    assert_eq!(out[1].count, 1);
    assert_eq!(out[1].location(), b"doc/glob/glob.md");

    // A term no page says is an answer, not a failure.
    let mut none = Ranked::new();
    search(b"glob", b"unmentioned", &mut Slice(&glob), &mut none).unwrap();
    assert_eq!(none.results().len(), 0);
    assert_eq!(none.offered(), 0);
}

/// The bound that lets the shell run this in `.bss`: more matching pages than the table
/// holds, kept in count order, with the total still reported so the caller does not imply a
/// complete answer.
#[cfg(feature = "builder")]
#[test]
fn a_search_keeps_the_best_sixteen_and_counts_the_rest() {
    use alloc::string::String;
    use alloc::vec::Vec;

    let mut texts: Vec<String> = Vec::new();
    for i in 0..RESULTS_MAX + 5 {
        let mut t = String::new();
        for _ in 0..=i {
            t.push_str("capability ");
        }
        // One word of filler, the same on every page, so density (not just raw count) still
        // increases with `i`: without it every page here is *only* the search term, so every
        // page's density is 1.0 and this test would be asserting a tie-break order rather than
        // the thing it says it asserts.
        t.push_str("noise");
        texts.push(t);
    }
    let paths: Vec<String> = (0..texts.len())
        .map(|i| alloc::format!("p{i}.md"))
        .collect();
    let sources: Vec<Source<'_>> = texts
        .iter()
        .zip(&paths)
        .map(|(t, p)| Source {
            path: p,
            title: "page",
            text: t.as_bytes(),
        })
        .collect();
    let bytes = build(&sources);

    let mut r = Ranked::new();
    search(b"many", b"capability", &mut Slice(&bytes), &mut r).unwrap();
    assert_eq!(r.results().len(), RESULTS_MAX);
    assert_eq!(r.offered(), RESULTS_MAX + 5);
    // Sorted by score, and here that agrees with raw count by construction (both grow with
    // `i`), so the strongest page is still the one with the most occurrences.
    assert_eq!(r.results()[0].count as usize, texts.len());
    for pair in r.results().windows(2) {
        assert!(pair[0].count >= pair[1].count, "the merge is not sorted");
    }
}

/// The property `notes/documentation.md`'s "where this goes next" named and priced: a short page
/// where a term is dense outranks a long page where the same term is only mentioned in
/// passing, even though the long page's raw occurrence count is higher.
#[cfg(feature = "builder")]
#[test]
fn ranking_divides_by_page_length() {
    let short = build(&[Source {
        path: "short.md",
        title: "Short",
        // Two tokens, one of them the term: as dense as a page can be without being nothing
        // else.
        text: b"capability page",
    }]);
    let mut long_text = alloc::string::String::new();
    for _ in 0..99 {
        long_text.push_str("filler ");
    }
    // Three mentions, but buried in ninety-nine words of filler: a higher raw count than the
    // short page's one, and a much lower density.
    long_text.push_str("capability capability capability");
    let long = build(&[Source {
        path: "long.md",
        title: "Long",
        text: long_text.as_bytes(),
    }]);

    let mut r = Ranked::new();
    search(b"a", b"capability", &mut Slice(&short), &mut r).unwrap();
    search(b"b", b"capability", &mut Slice(&long), &mut r).unwrap();

    let out = r.results();
    assert_eq!(out.len(), 2);
    // The raw counts are honest and unchanged: three mentions is still reported as three.
    let short_found = out
        .iter()
        .find(|f| f.location() == b"doc/a/short.md")
        .unwrap();
    let long_found = out
        .iter()
        .find(|f| f.location() == b"doc/b/long.md")
        .unwrap();
    assert_eq!(short_found.count, 1);
    assert_eq!(long_found.count, 3);
    // But density, not raw count, decides the order: the short page comes first.
    assert_eq!(out[0].location(), b"doc/a/short.md");
    assert_eq!(out[1].location(), b"doc/b/long.md");
}

// ---- what the mutation sweep found nothing asserting -----------------------------------
//
// Each of these kills a mutant that survived the rest of the suite. See notes/mutation-testing.md.

/// Both of the tokeniser's buffers are exactly `TERM_MAX` and both are written by index, so
/// the bound is the difference between a truncated term and a panic in a `no_std` reader.
/// `normalize` had this test and `tokens` did not, and the two write different buffers: the
/// second is the underscore-joined compound, which overflows on a shorter source word.
#[test]
fn a_run_longer_than_a_term_record_holds_is_truncated_rather_than_written_past() {
    let mut seen = [[0u8; TERM_MAX]; 8];
    let mut lens = [0usize; 8];
    let mut i = 0;
    tokens(
        b"averyveryverylongidentifiername and_a_compound_far_past_the_limit",
        |t| {
            assert!(
                t.len() <= TERM_MAX,
                "a term longer than its record: {}",
                t.len()
            );
            if i < 8 {
                seen[i][..t.len()].copy_from_slice(t);
                lens[i] = t.len();
                i += 1;
            }
        },
    );
    let has = |want: &[u8]| (0..i).any(|k| &seen[k][..lens[k]] == want);
    assert!(
        has(b"averyveryverylongidentif"),
        "the long word folds to its prefix"
    );
}

/// The tokeniser emits the compound when a separator ends the name; a name that ends the
/// TEXT reaches the tail block instead, which is a second copy of the same three tests. A
/// page whose last word is an identifier is ordinary, and nothing exercised that path.
#[test]
fn an_underscore_name_at_the_end_of_the_text_is_still_joined() {
    let mut seen = [[0u8; TERM_MAX]; 8];
    let mut lens = [0usize; 8];
    let mut i = 0;
    tokens(b"defined in grant_plan", |t| {
        if i < 8 {
            seen[i][..t.len()].copy_from_slice(t);
            lens[i] = t.len();
            i += 1;
        }
    });
    let has = |want: &[u8]| (0..i).any(|k| &seen[k][..lens[k]] == want);
    assert!(
        has(b"grantplan"),
        "the compound is not emitted at end of text"
    );
    assert!(has(b"grant") && has(b"plan"));
}

/// The part counter is what stops an ordinary word being offered twice as its own compound,
/// and it only counts runs that had characters in them. A pair of separators (`", "`, which
/// this repository's prose is full of) increments nothing, and a trailing underscore is the
/// same test in the tail block.
#[test]
fn a_word_is_offered_once_however_much_punctuation_precedes_it() {
    let count = |text: &'static [u8], want: &[u8]| {
        let mut n = 0;
        tokens(text, |t| {
            if t == want {
                n += 1;
            }
        });
        n
    };
    assert_eq!(count(b"ab, cd", b"cd"), 1);
    assert_eq!(count(b"ab_", b"ab"), 1);
}

/// Two refusals that share one `if`, so a test for either alone leaves the other's mutant
/// alive: a query that folds to no term at all, and a well-formed query against a shard with
/// no terms in it. Both are `None`, and neither may reach the binary search, which would be
/// comparing against a key of no bytes or probing a table that is not there.
#[cfg(feature = "builder")]
#[test]
fn a_query_that_names_nothing_is_answered_rather_than_searched() {
    let bytes = build(&[Source {
        path: "a.md",
        title: "A",
        text: b"one two",
    }]);
    let h = Header::parse(&bytes[..PAGE]).unwrap();
    assert!(
        lookup(&h, b"!!!", &mut Slice(&bytes)).is_none(),
        "punctuation is not a term"
    );

    let empty = build(&[Source {
        path: "b.md",
        title: "B",
        text: b"",
    }]);
    let eh = Header::parse(&empty[..PAGE]).unwrap();
    assert_eq!(eh.terms, 0);
    assert!(lookup(&eh, b"one", &mut Slice(&empty)).is_none());
}

/// The binary search is over pages, and its last page is `(terms - 1) / per`. Every arity
/// the other tests use hides the `- 1`: it only changes the answer when the record count is
/// an exact multiple of the 128 records a page holds, which is where the table ends flush
/// with a page boundary and a search that rounded up would probe past it.
#[cfg(feature = "builder")]
#[test]
fn a_term_table_that_exactly_fills_a_page_is_searched_to_its_last_record() {
    let per = PAGE / TERM_REC;
    let mut text = alloc::string::String::new();
    let mut want: alloc::vec::Vec<alloc::string::String> = alloc::vec::Vec::new();
    for i in 0..per {
        let t = alloc::format!("term{i:04}");
        text.push_str(&t);
        text.push(' ');
        want.push(t);
    }
    let bytes = build(&[Source {
        path: "x.md",
        title: "X",
        text: text.as_bytes(),
    }]);
    let h = Header::parse(&bytes[..PAGE]).unwrap();
    assert_eq!(
        h.terms as usize, per,
        "the table has to be exactly one page"
    );
    let mut src = Slice(&bytes);
    for t in &want {
        assert!(
            lookup(&h, t.as_bytes(), &mut src).is_some(),
            "{t} is in the table and was not found"
        );
    }
}

/// Both sections are paged and both divisions were exercised only in their first page. A
/// page record is 128 bytes, so 32 fit a page; a posting is 4 bytes, so 1024 do. This corpus
/// is past both, which is what makes `i / per` and `i % per` different from `i` and makes
/// the postings loop cross a page boundary mid-term.
#[cfg(feature = "builder")]
#[test]
fn a_store_with_more_pages_and_postings_than_fit_one_page_reads_them_all() {
    use alloc::string::String;
    use alloc::vec::Vec;
    let n = PAGE / POST_REC + 40;
    let paths: Vec<String> = (0..n).map(|i| alloc::format!("p{i:05}.md")).collect();
    let sources: Vec<Source<'_>> = paths
        .iter()
        .map(|p| Source {
            path: p,
            title: "page",
            text: b"capability",
        })
        .collect();
    let bytes = build(&sources);
    let h = Header::parse(&bytes[..PAGE]).unwrap();
    assert!(
        h.pages as usize > PAGE / PAGE_REC,
        "the fixture must span page-record pages"
    );

    let hit = lookup(&h, b"capability", &mut Slice(&bytes)).expect("every page says it");
    assert_eq!(hit.count as usize, n, "a posting per page");

    // Read them in the sixteen-posting batches `search` uses, which is what walks the boundary.
    let mut src = Slice(&bytes);
    let mut batch = [Posting { page: 0, count: 0 }; 16];
    let mut done = 0usize;
    let mut seen = alloc::vec![false; n];
    while done < n {
        let rest = Hit {
            first: hit.first + done as u32,
            count: (n - done) as u16,
        };
        let got = postings(&h, &rest, &mut src, &mut batch);
        assert!(got > 0, "the postings ran out at {done} of {n}");
        for p in &batch[..got] {
            let rec = page_record(&h, p.page, &mut src).expect("every posting names a page");
            let path = rec.path();
            let i: usize = core::str::from_utf8(&path[1..6]).unwrap().parse().unwrap();
            assert!(!seen[i], "page {i} read twice");
            seen[i] = true;
        }
        done += got;
    }
    assert!(seen.iter().all(|&b| b), "a page record was never reached");
}

/// `Found::is_truncated` had no test at all, in either direction, so both of its mutants
/// survived. It is the same promise the renderer makes about a long line: losing text is
/// allowed, losing it silently is not.
#[cfg(feature = "builder")]
#[test]
fn a_location_too_long_for_the_field_is_truncated_and_says_so() {
    let long_name = "a".repeat(60);
    let path = alloc::format!("notes/{long_name}.md");
    let bytes = build(&[Source {
        path: &path,
        title: "T",
        text: b"capability page",
    }]);

    let bundle = "b".repeat(40);
    let mut r = Ranked::new();
    search(bundle.as_bytes(), b"capability", &mut Slice(&bytes), &mut r).unwrap();
    let f = &r.results()[0];
    assert_eq!(
        f.location().len(),
        LOCATION_MAX,
        "a truncated location fills the field"
    );
    assert!(
        f.is_truncated(),
        "a location that lost its tail must say so"
    );

    // And the other side, which is every real store: `doc/<bundle>/<page>` fits.
    let mut fits = Ranked::new();
    search(b"swish", b"capability", &mut Slice(&bytes), &mut fits).unwrap();
    assert!(!fits.results()[0].is_truncated());
    assert!(fits.results()[0].location().starts_with(b"doc/swish/"));
}

/// Every existing merge test offers its results strongest first, so nothing ever moved: the
/// shift that makes room for a late strong result had no test, and neither did the walk
/// running the full length of a full table. `offer` takes fields rather than a record for
/// exactly this reason, so neither needs a shard.
#[test]
fn the_merge_shifts_what_a_stronger_result_displaces() {
    let mut r = Ranked::new();
    // Weak first, then strong: the second has to displace the first rather than land after it.
    r.offer(b"b", b"weak.md", b"Weak", 1, 100);
    r.offer(b"b", b"strong.md", b"Strong", 10, 100);
    assert_eq!(r.results().len(), 2);
    assert_eq!(r.results()[0].title(), b"Strong");
    assert_eq!(
        r.results()[1].title(),
        b"Weak",
        "the displaced result was not shifted down"
    );

    // Descending strength past the table's end, which walks `at` to the last slot held every
    // time and is what a bound that is off by one falls off.
    let mut full = Ranked::new();
    for i in 0..RESULTS_MAX + 4 {
        let count = (RESULTS_MAX + 4 - i) as u16;
        full.offer(b"b", b"p.md", b"page", count, 100);
    }
    assert_eq!(full.results().len(), RESULTS_MAX);
    assert_eq!(full.offered(), RESULTS_MAX + 4);
    assert_eq!(full.results()[0].count as usize, RESULTS_MAX + 4);
}

/// The truncation flag is a single comparison over a sum of five terms, and a location far
/// over the limit says the same thing as one a byte over: every mutant of that sum still
/// lands above it. The two sides of the boundary are what tell them apart, so they are what
/// this asserts. `offer` takes the fields rather than a record, which is why no shard is
/// needed to build a path of an exact length.
/// `doc/` and `/` are the two separators, so a location is 3 + 1 + bundle + 1 + base.
#[test]
fn a_location_exactly_as_long_as_the_field_is_not_truncated() {
    let base = "a".repeat(40);
    let exact = "b".repeat(LOCATION_MAX - STORE_DIR.len() - 2 - base.len());
    let mut r = Ranked::new();
    r.offer(exact.as_bytes(), base.as_bytes(), b"T", 1, 10);
    let f = &r.results()[0];
    assert_eq!(f.location().len(), LOCATION_MAX);
    assert!(
        !f.is_truncated(),
        "a location that exactly fills the field lost nothing"
    );

    // One byte more, and it is the one byte that has to flip the answer.
    let mut over = Ranked::new();
    over.offer(
        alloc::format!("{exact}b").as_bytes(),
        base.as_bytes(),
        b"T",
        1,
        10,
    );
    let g = &over.results()[0];
    assert_eq!(g.location().len(), LOCATION_MAX);
    assert!(
        g.is_truncated(),
        "a location one byte over the field lost its tail"
    );
}

/// The part counter again, on the case the first test could not reach: a separator that is
/// an underscore does NOT reset the compound, so two of them in a row leave the counter
/// where a mutant that counts empty runs would have moved it. `__rust_alloc` is how this
/// repository writes the symbol, so a leading pair is ordinary prose here.
#[test]
fn a_word_preceded_by_underscores_is_still_offered_once() {
    let mut n = 0;
    tokens(b"see __alloc", |t| {
        if t == b"alloc" {
            n += 1;
        }
    });
    assert_eq!(n, 1);
}

/// The narrowest arity in the layout: one record more than a page holds, so the last page
/// carries exactly one term and the count of records to search there is exactly one. Every
/// other size in these tests leaves that count comfortably large, which is what let an
/// arithmetic mutant that drives it to zero survive; here a term that is really there is
/// simply not found.
#[cfg(feature = "builder")]
#[test]
fn a_term_on_the_last_page_of_a_table_that_just_spilled_is_still_found() {
    let per = PAGE / TERM_REC;
    let mut text = alloc::string::String::new();
    for i in 0..per + 1 {
        text.push_str(&alloc::format!("term{i:04} "));
    }
    let bytes = build(&[Source {
        path: "x.md",
        title: "X",
        text: text.as_bytes(),
    }]);
    let h = Header::parse(&bytes[..PAGE]).unwrap();
    assert_eq!(h.terms as usize, per + 1, "one record past a full page");
    let last = alloc::format!("term{:04}", per);
    assert!(
        lookup(&h, last.as_bytes(), &mut Slice(&bytes)).is_some(),
        "the only term on the last page was not found"
    );
}

/// `notes/documentation.md` prices page alignment at a four-page floor (16 KiB) under every
/// bundle however small: the header, the page records, the terms and the postings, one page
/// each. It is the cost that decided bundle granularity, and nothing asserted it, so a
/// mutant that multiplied the section offsets instead of adding them produced a
/// sixteen-megabyte shard that every other test read back perfectly, because the reader
/// takes its offsets from the same header the builder wrote.
#[cfg(feature = "builder")]
#[test]
fn a_small_bundle_is_the_four_page_floor_and_not_a_megabyte() {
    let bytes = build(&[Source {
        path: "a.md",
        title: "A",
        text: b"one two three",
    }]);
    assert_eq!(bytes.len(), 4 * PAGE);
}

#[cfg(feature = "builder")]
#[test]
fn a_title_is_the_first_level_one_heading() {
    assert_eq!(
        title_of(b"intro\n\n# The title\n\n## no\n"),
        Some("The title")
    );
    assert_eq!(title_of(b"## only a section\n"), None);
}
