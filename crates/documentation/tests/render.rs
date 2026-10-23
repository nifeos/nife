//! What the renderer must get right: the layout each construct takes on a terminal, and that no
//! page of this repository loses a character on the way (`every_character_survives`).
//!
//! The grammar is `pulldown-cmark`'s, so these tests do not hold CommonMark itself. Where a test
//! here gives markdown, it is either a layout claim or a pin on how the parser reads a shape this
//! repository's own pages write, which is what a reader of those pages meets.

use documentation::{Sink, Style};

struct Buf(Vec<u8>);

impl Sink for Buf {
    fn put(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}

fn plain(src: &str, width: u16) -> String {
    render(
        src,
        Style {
            width,
            color: false,
        },
    )
}

fn render(src: &str, style: Style) -> String {
    let mut out = Buf(Vec::new());
    documentation::render(src, style, &mut out);
    String::from_utf8(out.0).unwrap()
}

/// Level one is the page's name and is uppercased; level two keeps its case, because a document
/// whose every heading shouts has told the reader nothing about which one is the title.
#[test]
fn headings_shout_once() {
    assert_eq!(
        plain("# Title\n\n## Section\n\nBody.\n", 80),
        "TITLE\n\nSection\n\n  Body.\n"
    );
}

/// Half the fenced code in this repository is shell, and shell comments start with `#`. This is
/// the rule that keeps `#!/bin/sh` and `# a comment` out of the heading path.
#[test]
fn a_hash_without_a_space_is_not_a_heading() {
    assert_eq!(plain("#nope\n", 80), "  #nope\n");
}

/// Source line breaks are not output line breaks: a paragraph is re-flowed to the terminal's
/// width, which is the whole reason to render markdown rather than print it.
#[test]
fn paragraphs_rewrap() {
    let out = plain("one two\nthree four five\n", 12);
    assert_eq!(out, "  one two\n  three four\n  five\n");
}

#[test]
fn code_blocks_are_verbatim() {
    let out = plain("```sh\n  ls   -l\n```\n", 80);
    assert_eq!(out, "      ls   -l\n");
}

/// 11,281 code spans in this corpus, many of them full of the characters the other inline rules
/// look for. If this ordering were wrong, `*ptr` inside backticks would open emphasis and eat
/// the rest of the line.
#[test]
fn a_code_span_protects_its_contents() {
    let out = render(
        "a `*ptr [x](y)` b\n",
        Style {
            width: 80,
            color: true,
        },
    );
    assert!(out.contains("\x1b[36m*ptr [x](y)"));
    assert!(!out.contains("\x1b[4m"));
}

/// The corpus writes `snake_case` identifiers in running prose constantly. CommonMark reads an
/// underscore inside a word as text, and an unclosed `__` as text too, so none of these styles.
#[test]
fn underscores_are_never_emphasis() {
    let out = render(
        "the __rust_alloc symbol and filesystem_contract\n",
        Style {
            width: 80,
            color: true,
        },
    );
    assert!(!out.contains('\x1b'));
}

#[test]
fn emphasis_needs_a_closer_that_is_not_a_space() {
    let styled = render(
        "a *b* c\n",
        Style {
            width: 80,
            color: true,
        },
    );
    assert!(styled.contains("\x1b[4mb"));
    // A lone asterisk mid-sentence has no closer and must not swallow the rest of the line.
    let stray = render(
        "2 * 3 = 6\n",
        Style {
            width: 80,
            color: true,
        },
    );
    assert!(!stray.contains('\x1b'));
}

/// Three spellings of a bullet on one screen read as three different things, so they are one
/// spelling on output. The continuation aligns under the text, not under the bullet.
#[test]
fn list_markers_are_normalised_and_hang() {
    let out = plain("* one two three\n* four\n* five\n", 14);
    assert_eq!(out, "  - one two\n    three\n  - four\n  - five\n");
    // A different bullet character starts a new list in CommonMark, so these are three lists, and
    // a blank line separates each from the next.
    assert_eq!(
        plain("* one\n+ two\n- three\n", 40),
        "  - one\n\n  - two\n\n  - three\n"
    );
}

#[test]
fn ordered_lists_keep_their_numbers() {
    assert_eq!(plain("1. one\n2. two\n", 80), "  1. one\n  2. two\n");
}

#[test]
fn block_quotes_get_a_rule() {
    assert_eq!(plain("> quoted\n", 80), "  | quoted\n");
}

#[test]
fn tables_align_to_their_widest_cell() {
    let out = plain("| a | bbbb |\n|---|---|\n| cc | d |\n", 80);
    assert_eq!(out, "  a  | bbbb\n  ---+-----\n  cc | d   \n");
}

/// A paragraph that happens to start with a pipe must not be silently eaten by the table path.
#[test]
fn a_pipe_line_without_a_delimiter_is_still_printed() {
    let out = plain("| not a table\n", 80);
    assert!(out.contains("not a table"));
}

/// A terminal cannot follow a link, so a reader who cannot see the destination has been handed a
/// worse document than the source.
#[test]
fn links_show_where_they_point() {
    let out = plain("see [the note](notes/glob.md) for more\n", 80);
    assert_eq!(out, "  see the note notes/glob.md for more\n");
}

/// A terminal left bold at end of line paints the next row and the prompt with it. Every line
/// this renderer emits ends reset.
#[test]
fn an_attribute_never_survives_a_newline() {
    let out = render(
        "# Title\n\n**strong text that wraps past the edge**\n",
        Style {
            width: 20,
            color: true,
        },
    );
    for line in out.split('\n') {
        if line.contains('\x1b') {
            assert!(
                line.ends_with("\x1b[0m") || !line.contains("\x1b["),
                "line: {line:?}"
            );
        }
    }
}

/// A quoted closing fence closes the quoted block, and what follows renders as what it is. A
/// page with a quoted transcript in it must not render its next three hundred lines as code.
#[test]
fn a_fence_inside_a_block_quote_closes() {
    let src = "> intro\n>\n> ```text\n>   code here\n> ```\n\nAfter.\n\n## A heading\n";
    let out = plain(src, 80);
    assert_eq!(
        out, "  | intro\n\n    |   code here\n\n  After.\n\nA heading\n",
        "{out:?}"
    );
}

/// Outside a block quote, a `>` inside a fence is code: a mail quote, a diff or a shell
/// transcript survives intact.
#[test]
fn a_code_line_that_starts_with_a_quote_marker_keeps_it() {
    assert_eq!(plain("```text\n> quoted\n```\n", 80), "    > quoted\n");
}

#[test]
fn a_document_with_no_trailing_newline_still_ends() {
    assert_eq!(plain("# T", 80), "T\n");
}

/// Is this source line a fence opener or closer, quoted or not?
///
/// A quoted fence is a fence: the renderer strips the markers before it classifies the line, so
/// `> ` followed by three backticks opens a code block exactly as three backticks alone do.
fn is_fence(line: &str) -> bool {
    let mut s = line.trim_start();
    while let Some(rest) = s.strip_prefix('>') {
        s = rest.trim_start();
    }
    s.starts_with("```")
}

/// `line` with each character reference (`&nbsp;`, `&#160;`, `&#xA0;`) taken out.
fn without_entities(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 1..];
        let end = tail.find(';').filter(|&e| {
            e > 0
                && e <= 32
                && tail[..e]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '#')
        });
        match end {
            Some(e) => rest = &tail[e + 1..],
            None => {
                out.push('&');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

/// `line` without an ordered list item's leading number, past any block-quote markers.
fn without_list_number(line: &str) -> String {
    let mut s = line.trim_start();
    let mut prefix = String::new();
    while let Some(rest) = s.strip_prefix('>') {
        prefix.push('>');
        s = rest.trim_start();
    }
    let digits = s.chars().take_while(char::is_ascii_digit).count();
    let marked = digits > 0
        && digits <= 9
        && matches!(s[digits..].chars().next(), Some('.' | ')'))
        && s[digits + 1..].starts_with(' ');
    if marked {
        prefix + &s[digits + 1..]
    } else {
        line.to_string()
    }
}

/// Every letter and digit in the repository's own markdown reaches the rendered output, in order.
///
/// The claim is that the layout loses nothing on real pages: a layout that ate a construct (an HTML
/// block, a link destination, a table cell) drops the characters inside it. Subsequence, because
/// the renderer adds characters (an image's `image:` marker) and joins what markup split.
///
/// Runs at 4,000 columns, so no table cell is truncated, and natively only: Miri would need the
/// environment forwarded and isolation off, and adds nothing to a crate with no `unsafe`. See
/// notes/documentation/corpus-test.md for both choices.
#[test]
#[cfg_attr(
    miri,
    ignore = "547 markdown files off disk; the nineteen unit tests cover the renderer"
)]
fn every_character_survives() {
    // Runtime, not env!: the compile-time path is stale in an artifact cached before the checkout
    // moved, and would check zero files.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets this for tests");
    let root = std::path::Path::new(&manifest)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let mut checked = 0;
    for dir in ["notes", "design/decisions", "design/roadmap"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        // One level down as well: §212 (a prose budget) puts appendices under `notes/<stem>/`.
        let mut paths = Vec::new();
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if let Ok(sub) = std::fs::read_dir(&path) {
                    paths.extend(sub.flatten().map(|e| e.path()));
                }
            } else {
                paths.push(path);
            }
        }
        for path in paths {
            if path.extension().and_then(|s| s.to_str()) != Some("md") {
                continue;
            }
            let Ok(src) = std::fs::read_to_string(&path) else {
                continue;
            };
            // A fence's info string (the `bash` in a ```bash line) is a language tag rather than
            // content, so it is not offered as evidence, quoted or not.
            // Two things in the source are markup under CommonMark rather than text, so they are
            // not offered as evidence either: a character reference (`&nbsp;` is one character,
            // and its letters are not content) and an ordered list item's number, because a list
            // counts up from its first item whatever the later items say.
            let want: Vec<char> = src
                .lines()
                .filter(|l| !is_fence(l))
                .map(|l| without_list_number(&without_entities(l)))
                .flat_map(|l| l.chars().chain(core::iter::once('\n')).collect::<Vec<_>>())
                .filter(char::is_ascii_alphanumeric)
                .map(|c| c.to_ascii_lowercase())
                .collect();
            let mut out = Buf(Vec::new());
            documentation::render(
                &src,
                Style {
                    width: 4000,
                    color: false,
                },
                &mut out,
            );

            let have: Vec<char> = String::from_utf8(out.0)
                .unwrap()
                .chars()
                .filter(char::is_ascii_alphanumeric)
                .map(|c| c.to_ascii_lowercase())
                .collect();
            let mut i = 0;
            for c in &have {
                if i < want.len() && want[i] == *c {
                    i += 1;
                }
            }
            assert!(
                i == want.len(),
                "{}: rendering dropped text near {:?}",
                path.display(),
                want[i.saturating_sub(30)..(i + 30).min(want.len())]
                    .iter()
                    .collect::<String>()
            );
            checked += 1;
        }
    }
    assert!(checked > 100, "only checked {checked} files");
}

// ---- tables ---------------------------------------------------------------------------------

/// `|:---|---:|:-:|` is left, right and centre, and each pads its cell on the side it names.
#[test]
fn table_columns_align_the_way_the_delimiter_row_says() {
    let out = plain("| aaa | bbb | ccc |\n|:---|---:|:-:|\n| x | y | z |\n", 80);
    assert_eq!(
        out, "  aaa | bbb | ccc\n  ----+-----+----\n  x   |   y |  z \n",
        "{out:?}"
    );
}

/// Alignment is a property of one table's delimiter row. A left-aligned table follows a
/// right-aligned one here, and its cells sit on the left.
#[test]
fn alignment_does_not_outlive_its_table() {
    let out = plain(
        "| a | b |\n|---:|---|\n| x | y |\n\n| pp | q |\n|---|---|\n| r | s |\n",
        80,
    );
    assert!(out.ends_with("  pp | q\n  ---+--\n  r  | s\n"), "{out:?}");
}

/// `notes/scripts/build-test-and-gate.md` has one (`--arch aarch64\| riscv64`), and reading it as
/// a separator would give that table a third column nobody wrote. The backslash is the escape and
/// not the text, so it does not reach the output either.
#[test]
fn an_escaped_pipe_is_cell_text_and_not_a_column_boundary() {
    let out = plain("| cmd | note |\n|---|---|\n| a \\| b | two |\n", 80);
    assert_eq!(
        out, "  cmd   | note\n  ------+-----\n  a | b | two \n",
        "{out:?}"
    );
}

/// `| a | b |` and `| a | b` are the same two-column row: the cell after the last pipe is one
/// nobody typed, and keeping it would widen every table in the repository by a blank column.
#[test]
fn a_trailing_pipe_does_not_add_a_column() {
    assert_eq!(
        plain("| a | b |\n|---|---|\n| x | y |\n", 80),
        plain("| a | b\n|---|---\n| x | y\n", 80)
    );
}

/// Shrinking the widest column rather than all of them is what keeps a table of one long prose
/// column and three short labels readable: the prose loses characters and the labels do not.
#[test]
fn a_table_wider_than_the_terminal_shrinks_its_widest_column() {
    let out = plain(
        "| aaaaaaaaaa | bb |\n|---|---|\n| cccccccccccccccccccc | d |\n",
        20,
    );
    assert_eq!(
        out, "  aaaaaaaaaa    | bb\n  --------------+---\n  ccccccccccccc | d \n",
        "{out:?}"
    );
    // Every line fits the terminal, which is the property the shrink exists for.
    for line in out.lines() {
        assert!(line.chars().count() <= 20, "line over width: {line:?}");
    }
}

/// Losing text is the one failure a documentation service cannot have, and a long table is one
/// block: no row is lost and no blank line splits it. `notes/releasable-crates-2026-10-07/inventory.md`
/// is about 100 rows, so this is real.
#[test]
fn a_long_table_keeps_every_row_in_one_block() {
    let mut src = String::from("| n | v |\n|---|---|\n");
    let rows = 120;
    for i in 0..rows {
        src.push_str(&format!("| r{i} | v{i} |\n"));
    }
    let out = plain(&src, 80);
    for i in 0..rows {
        assert!(
            out.contains(&format!("r{i} ")) || out.contains(&format!("r{i}\n")),
            "row {i} lost"
        );
    }
    assert!(!out.contains("\n\n"), "a table is one block: {out:?}");
}

/// The delimiter row is what makes a run of pipe-led lines a table; without one the lines are a
/// paragraph that happens to contain pipes, and emphasising the first would claim a structure the
/// author did not write.
#[test]
fn a_header_row_is_emphasised_only_when_a_delimiter_row_says_it_is_one() {
    let table = render(
        "| h |\n|---|\n| c |\n",
        Style {
            width: 40,
            color: true,
        },
    );
    assert!(table.contains("\x1b[1mh"), "{table:?}");
    let not_a_table = render(
        "| h |\n| c |\n",
        Style {
            width: 40,
            color: true,
        },
    );
    assert!(!not_a_table.contains("\x1b[1m"), "{not_a_table:?}");
}

/// Three of `-`, `*` or `_` with only spaces between them, and nothing else, after a blank line
/// (directly under a paragraph, `---` is a setext underline instead).
#[test]
fn thematic_breaks_are_three_or_more_of_one_mark() {
    let rule = "  ------------------\n";
    for src in ["---", "***", "___", "- - -", "----------"] {
        let out = plain(&format!("a\n\n{src}\n\nb\n"), 20);
        assert_eq!(
            out,
            format!("  a\n\n{rule}\n  b\n"),
            "{src} is a thematic break"
        );
    }
    // Two marks is not three, a different mark is not a rule at all, and a mark with text beside it
    // is a paragraph. Each of these renders as its own source text.
    for src in ["--", "+++", "-a-", "** *x"] {
        let out = plain(&format!("a\n\n{src}\n\nb\n"), 20);
        assert!(!out.contains("-----"), "{src} rendered as a rule: {out:?}");
    }
}

/// Markdown stops at six, and past that the line is text. The rule sits beside the one that
/// keeps `#!/bin/sh` out of the heading path, and neither had a test on its upper edge.
#[test]
fn a_seventh_hash_is_not_a_heading() {
    assert_eq!(plain("###### six\n", 80), "        six\n");
    assert_eq!(plain("####### seven\n", 80), "  ####### seven\n");
}

/// A tab counts as four columns, so a tab-indented line after a blank one is indented code,
/// exactly as four spaces are.
#[test]
fn a_tab_indents_as_four_columns() {
    assert_eq!(
        plain("para\n\n\tmore text\n", 40),
        plain("para\n\n    more text\n", 40)
    );
}

/// `1.`, `1)` and `10.` are all list markers. A list counts up from its first number, and is
/// written with a dot whatever its source used: the parser does not say which it was.
#[test]
fn an_ordered_marker_may_be_a_parenthesis_or_two_digits() {
    assert_eq!(plain("1) one\n2) two\n", 80), "  1. one\n  2. two\n");
    assert_eq!(
        plain("10. ten\n11. eleven\n", 80),
        "  10. ten\n  11. eleven\n"
    );
}

/// A word is never broken and a URL has to be: it is one unbreakable run that no terminal is
/// wide enough for, and the alternative to breaking it is a line that runs off the screen.
#[test]
fn a_destination_too_long_for_the_line_is_broken_rather_than_overrun() {
    let out = plain(
        "see [x](aaaaaaaaaabbbbbbbbbbccccccccccdddddddddd) end\n",
        20,
    );
    assert_eq!(
        out, "  see x\n  aaaaaaaaaabbb\n  bbbbbbbccccccccccd\n  ddddddddd end\n",
        "{out:?}"
    );
}

/// Inside a fence the quote markers come off and nothing else does: a code line's own leading
/// spaces are its meaning.
#[test]
fn a_quoted_code_line_keeps_its_own_indentation() {
    assert_eq!(
        plain("> ```text\n>     indented\n> ```\n", 40),
        "    |     indented\n"
    );
}

// ---- inline forms --------------------------------------------------------------------------

#[test]
fn strikethrough_is_rendered_and_a_lone_tilde_is_text() {
    let out = render(
        "a ~~gone~~ b\n",
        Style {
            width: 40,
            color: true,
        },
    );
    assert!(out.contains("\x1b[9mgone"), "{out:?}");
    assert_eq!(plain("a ~~gone~~ b\n", 40), "  a gone b\n");
    // GitHub's strikethrough takes one tilde as well as two, so a pair of single tildes strikes.
    // The corpus writes `~` in paths, and an unpaired one is text.
    assert_eq!(plain("a ~gone~ b\n", 40), "  a gone b\n");
    assert_eq!(plain("see ~/notes now\n", 40), "  see ~/notes now\n");
}

/// A terminal cannot draw a picture, so the destination is the only thing a reader can act on:
/// it is the file they would open on the host.
#[test]
fn an_image_shows_what_it_is_and_where_it_points() {
    assert_eq!(
        plain("![a picture](notes/x.png) after\n", 60),
        "  [image: a picture] notes/x.png after\n"
    );
    // The `!` only means an image when a `[` follows it immediately, and a `]` only opens a
    // destination when a `(` follows it immediately. Neither is an image, and both are ordinary
    // enough in prose that reading them as one would misrender.
    assert_eq!(plain("!x[a](y.png)\n", 40), "  !xa y.png\n");
    assert_eq!(plain("![a] (x.png)\n", 40), "  ![a] (x.png)\n");
}

/// CommonMark's flanking rule: a `*` with a space before it does not close, which is what stops a
/// stray `*` mid-sentence from swallowing the rest of a paragraph.
#[test]
fn a_closer_preceded_by_a_space_is_not_one() {
    assert_eq!(plain("*a * b*\n", 40), "  a * b\n");
    assert_eq!(plain("a **b ** c**\n", 40), "  a b ** c\n");
    assert_eq!(plain("a ****b\n", 40), "  a ****b\n");
}

/// `#` to `######` alone is an empty heading in CommonMark, which prints nothing; digits alone
/// are text.
#[test]
fn a_line_of_marks_alone_is_an_empty_heading_or_text() {
    assert_eq!(plain("###\n", 40), "");
    assert_eq!(plain("#\n", 40), "");
    assert_eq!(plain("123\n", 40), "  123\n");
}

/// Each clause on its own: no digits at all, digits followed by something else, and the marker
/// length, which only shows where a wrapped item's continuation lines align.
#[test]
fn an_ordered_marker_is_digits_then_a_dot_or_a_bracket_then_a_space() {
    assert_eq!(plain(". text\n", 40), "  . text\n");
    assert_eq!(plain("1a text\n", 40), "  1a text\n");
    assert_eq!(
        plain("1. a very long ordered item that wraps around\n", 24),
        "  1. a very long ordered\n     item that wraps\n     around\n"
    );
}

/// CommonMark reads a line indented four columns past its container, after a blank line, as
/// indented code, so it takes the code margin and keeps its own text verbatim. Inside a quote the
/// marker's space is the container's, and the four after it are code.
#[test]
fn four_columns_of_indent_make_code_inside_a_block_quote_or_out() {
    assert_eq!(
        plain("para\n\n    more text\n", 40),
        "  para\n\n    more text\n"
    );
    assert_eq!(
        plain(">     quoted indented\n", 40),
        "    | quoted indented\n"
    );
}

/// A fence opened inside a quote ends with its quote. A line with fewer markers closes the inner
/// quote, and the fence with it, rather than continuing the code lazily; an unpaired opener later
/// is an empty block and prints nothing.
#[test]
fn a_line_with_fewer_quote_markers_ends_the_quoted_fence() {
    assert_eq!(
        plain(">> ```text\n>>   deep\n>> ```\n", 40),
        "    | |   deep\n"
    );
    assert_eq!(plain(">> ```text\n> one\n>> ```\n", 40), "  | one\n");
    assert_eq!(
        plain("> ```text\n>   two\nlazy\n> ```\n", 40),
        "    |   two\n\n  lazy\n"
    );
}

/// Width is counted in characters and not bytes, so a two-byte character occupies one column and
/// a column sized in bytes would be one too wide. The BUGS section is honest that a CJK
/// character then occupies two columns and is counted as one; this is the Latin case, which is
/// the one the corpus has.
#[test]
fn a_wide_character_is_one_column_of_the_table_it_sits_in() {
    assert_eq!(
        plain("| é | bb |\n|---|---|\n| x | y |\n", 80),
        "  é | bb\n  --+---\n  x | y \n"
    );
}

/// The escape scan reads the byte after the backslash, and the byte before nothing: a cell that
/// opens with a backslash and a cell that ends with one are the two edges of it. Both are real,
/// because this repository's tables carry paths and regular expressions.
#[test]
fn a_backslash_that_is_not_escaping_a_pipe_is_ordinary_cell_text() {
    assert_eq!(
        plain("| \\x | b |\n|---|---|\n| p | q |\n", 40),
        "  \\x | b\n  ---+--\n  p  | q\n"
    );
    assert_eq!(
        plain("| a\\ | b |\n|---|---|\n| x | y |\n", 40),
        "  a\\ | b\n  ---+--\n  x  | y\n"
    );
}

/// The room a table has is the terminal less the margin AND less the quote rules drawn down the
/// left of every row, which is two columns per level. A table sized against the bare terminal
/// would overrun by exactly that, and only a quoted table wide enough to be shrunk shows it.
#[test]
fn a_table_inside_a_block_quote_fits_inside_the_rule() {
    let quoted = plain(
        "> | aaaaaaaaaa | bb |\n> |---|---|\n> | cccccccccccccccccccc | d |\n",
        24,
    );
    let bare = plain(
        "| aaaaaaaaaa | bb |\n|---|---|\n| cccccccccccccccccccc | d |\n",
        24,
    );
    for line in quoted.lines() {
        assert!(line.chars().count() <= 24, "a quoted row overran: {line:?}");
    }
    assert!(
        quoted.contains("ccccccccccccccc |") && !quoted.contains("cccccccccccccccc |"),
        "{quoted:?}"
    );
    // And the bare table gets the two columns back, which is the other side of the same sum.
    assert!(bare.contains("ccccccccccccccccc |"), "{bare:?}");
}

/// Two rows whose cells are long: they are laid out together, so the short row is padded to the
/// long one's width and every output line is the same length.
#[test]
fn a_table_is_one_block_however_long_its_cells() {
    let src = format!(
        "| {} | b |\n|---|---|\n| {} | c |\n",
        "w".repeat(100),
        "W".repeat(150)
    );
    let out = plain(&src, 4000);
    let widths: Vec<usize> = out.lines().map(str::len).collect();
    assert_eq!(widths.len(), 3);
    assert!(
        widths.iter().all(|&w| w == widths[0]),
        "not one block: {widths:?}"
    );
}

// ---- the output cursor ---------------------------------------------------------------------
//
// `col` is the layout's only piece of arithmetic with two consumers: `close_line`'s "is a line
// open" test, and the wrap decision. These are the inputs where a wrong cursor moves the output,
// and each one is an ordinary page rather than a contrivance.

/// The one code line whose visible width is zero. It is still a line of the code, so it ends
/// with a newline of its own. Most fenced blocks in this repository have one.
#[test]
fn a_blank_line_inside_a_fence_is_a_blank_line() {
    assert_eq!(
        plain("```text\none\n\ntwo\n```\n", 40),
        "    one\n    \n    two\n"
    );
}

/// `[image:` and `]` are runs this renderer adds, and the cursor has to move by their width like
/// any other word's, or the wrap that follows lands in the wrong place.
#[test]
fn an_image_leaves_the_cursor_where_its_text_ended() {
    assert_eq!(
        plain("![pic](d.png) then some more words here\n", 24),
        "  [image: pic] d.png\n  then some more words\n  here\n"
    );
}

/// Each level of block quote costs two columns of every line, and the cursor has to start past
/// them: a paragraph wrapped against the bare margin overruns by exactly two per level. Two
/// levels, because one level cannot tell an addition from the constant it adds.
#[test]
fn a_quoted_paragraph_wraps_inside_its_rule() {
    assert_eq!(
        plain("> one two three four five six seven\n", 20),
        "  | one two three\n  | four five six\n  | seven\n"
    );
    assert_eq!(
        plain(">> one two three four five six seven\n", 20),
        "  | | one two three\n  | | four five six\n  | | seven\n"
    );
}

/// A markdown file that has been through an editor on another system arrives with CRLF, and the
/// `\r` is framing rather than text: kept, it would print a stray control character.
#[test]
fn a_carriage_return_at_end_of_line_is_not_content() {
    assert_eq!(plain("a\r\nb\r\n", 40), "  a b\n");
    assert_eq!(plain("# Title\r\n", 40), "TITLE\n");
}

/// A quoted line's own indentation before its marker is not structure, so the rule replaces it.
#[test]
fn an_indented_block_quote_indents_by_its_rule_and_not_by_its_spaces() {
    assert_eq!(
        plain("  >   quoted indented\n", 40),
        "  | quoted indented\n"
    );
}

/// Reduced from `notes/rented-metal.md`, a twelve-column table: every column is its own.
#[test]
fn a_table_of_twelve_columns_keeps_every_character() {
    let src = "| a | b | c | d | e | f | g | h | i | j | k | l |\n\
               |---|---|---|---|---|---|---|---|---|---|---|---|\n\
               | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 0 | y | z |\n";
    let out = plain(src, 200);
    for want in ["h | i | j | k | l", "8 | 9 | 0 | y | z"] {
        assert!(out.contains(want), "{want:?} was dropped: {out:?}");
    }
}

/// The row's closing pipe ends it and is not cell text, so no row ends in a stray bar.
#[test]
fn a_row_has_no_trailing_pipe_of_its_own() {
    let src = "| a | b | c | d | e | f | g | h | i |\n\
               |---|---|---|---|---|---|---|---|---|\n\
               | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |\n";
    let out = plain(src, 200);
    for line in out.lines() {
        assert!(
            !line.trim_end().ends_with('|'),
            "the row's closing pipe was printed: {line:?}"
        );
    }
}

/// Markdown in this repository is written by hand and a short row is ordinary. The missing cells
/// are blanks of the column's width, so the rows below still line up under their headings.
#[test]
fn a_row_with_fewer_cells_than_the_table_is_padded_rather_than_ragged() {
    assert_eq!(
        plain("| a | b | c |\n|---|---|---|\n| x |\n| p | q | r |\n", 40),
        "  a | b | c\n  --+---+--\n  x |   |  \n  p | q | r\n"
    );
}

// ---- shapes that look right on one line ------------------------------------------------------
//
// A line read as a list item it is not would print the same bytes on its first line; the margin
// its continuation aligns to is the difference, so it takes two lines to see.

/// `. text` has no digits before its dot and `1a text` has no dot after its digits: neither is an
/// ordered marker, so nothing after them hangs under a bullet nobody wrote.
#[test]
fn a_line_that_is_not_a_list_does_not_hang_like_one() {
    assert_eq!(
        plain(". one two three four five six\n", 20),
        "  . one two three\n  four five six\n"
    );
    assert_eq!(
        plain("1a one two three four five six\n", 20),
        "  1a one two three\n  four five six\n"
    );
}

/// The commonest line in a quoted transcript: a `>` with nothing after it, which is an empty line
/// of the code, at both depths.
#[test]
fn a_blank_line_inside_a_quoted_fence_is_not_read_past_its_marker() {
    assert_eq!(
        plain("> ```text\n>\n> after\n> ```\n", 40),
        "    | \n    | after\n"
    );
    assert_eq!(
        plain(">> ```text\n>>\n>> after\n>> ```\n", 40),
        "    | | \n    | | after\n"
    );
}

/// The text after a code span is ordinary text again, and the backticks are not printed.
#[test]
fn a_code_span_hands_back_the_line_past_its_closing_backtick() {
    assert_eq!(plain("a `code` b\n", 40), "  a code b\n");
}

// ---- a row's last cell ---------------------------------------------------------------------
//
// A row's end strips trailing blanks, then one closing pipe unless a backslash escapes it.

const NINE: &str = "|-|-|-|-|-|-|-|-|-|\n";

/// Blanks after the closing pipe are part of the line, so the pipe is not its last byte; a tab
/// is blank too.
#[test]
fn a_row_with_trailing_blanks_still_loses_its_closing_pipe() {
    let src = format!(
        "| a | b | c | d | e | f | g | h | i |\n{NINE}| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | \t \n"
    );
    let out = plain(&src, 200);
    assert_eq!(
        out.lines().last().unwrap(),
        "  1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9"
    );
}

/// `\|` is a pipe in the cell, not the row's end, and the backslash is not text. The last cell
/// here is `9 |`, so the pipe stays and the row has no closing pipe of its own.
#[test]
fn a_row_keeps_an_escaped_closing_pipe_as_text() {
    let src = format!(
        "| a | b | c | d | e | f | g | h | i |\n{NINE}| 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 \\|\n"
    );
    let out = plain(&src, 200);
    assert_eq!(
        out.lines().last().unwrap(),
        "  1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |"
    );
}

/// A backslash that ends a cell escapes nothing and is cell text.
#[test]
fn a_cell_that_ends_in_a_backslash_is_not_read_past_its_end() {
    assert_eq!(
        plain("| a\\ | b |\n|---|---|\n| x\\ | y\\ |\n", 40),
        "  a\\ | b \n  ---+---\n  x\\ | y\\\n"
    );
}

// ---- the cursor at the edges of a line ------------------------------------------------------

/// A heading's first word starts at the margin, owing no space. A margin of zero is where a
/// space owed and the cursor's position agree, so it is the edge.
#[test]
fn a_heading_with_two_spaces_after_its_hashes_has_no_leading_blank() {
    assert_eq!(plain("#  Title\n", 40), "TITLE\n");
}

/// `[image: pic]` is 12 columns from a margin of 2 and `d.png` brings the line to 20. Any other
/// width puts that word on one side of the wrap or the other, so a width of 21 is the edge.
#[test]
fn an_image_marker_moves_the_cursor_by_its_own_width() {
    assert_eq!(
        plain("![pic](d.png) x\n", 21),
        "  [image: pic] d.png\n  x\n"
    );
}

/// The wrap steps one character at a time by asking how long the character at the cursor is. A
/// two-byte character asked about the wrong byte is split in half and the output stops being text.
#[test]
fn a_long_destination_is_wrapped_on_character_boundaries() {
    let out = plain("[a](éééééééééééééé)\n", 12);
    assert_eq!(out, "  a\n  ééééééééé\n  ééééé\n");
    // Mixed widths, so a question about any byte but the cursor's gets a different length.
    let out = plain("[a](aéaéaéaéaéaéaé)\n", 12);
    assert_eq!(out, "  a\n  aéaéaéaéa\n  éaéaé\n");
}

/// A fence opened one quote deep takes one marker from each line. A second `>` on a line is the
/// code's, as in a quoted mail inside a quoted block.
#[test]
fn a_quoted_fence_keeps_a_marker_of_its_own() {
    assert_eq!(plain("> ```text\n> > keep\n> ```\n", 40), "    | > keep\n");
}

/// A column whose every cell is empty has width zero, so only the margin moves the cursor off
/// zero, and `close_line` ends a line only past zero: each row still ends.
#[test]
fn an_empty_one_column_table_still_ends_each_of_its_lines() {
    assert_eq!(plain("| |\n|-|\n| |\n", 80), "  \n  \n  \n");
}

/// A block that printed a line is separated from what follows it, like any other block; only an
/// empty one leaves the blank line before it still owed instead of adding a second.
#[test]
fn a_code_block_owes_the_next_block_a_blank_line() {
    assert_eq!(
        plain("before\n\n```\nx\n```\n\nafter\n", 40),
        "  before\n\n    x\n\n  after\n"
    );
}

/// An autolink's text is its destination, so printing the destination beside it, as every other
/// link gets, would say the address twice.
#[test]
fn an_autolink_is_printed_once_in_running_text_and_in_a_cell() {
    assert_eq!(plain("x <http://a.b> y\n", 40), "  x http://a.b y\n");
    assert_eq!(
        plain("| a |\n|---|\n| <http://a.b> |\n", 40),
        "  a         \n  ----------\n  http://a.b\n"
    );
}

/// A space separates two words on one line. Owed at the start of a line, after a list marker or
/// at the page's first word, it would only shift the text off its margin.
#[test]
fn a_space_owed_before_the_first_word_on_a_line_is_dropped() {
    assert_eq!(plain("&#32;x\n", 40), "  x\n");
    assert_eq!(plain("- &#32;x\n", 40), "  - x\n");
}
