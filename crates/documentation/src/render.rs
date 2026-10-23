//! The terminal renderer: markdown in, styled terminal bytes out.
//!
//! `pulldown-cmark` parses CommonMark, with tables and strikethrough, into events, and this lays
//! them out: a two-column margin, level-one headings in capitals, hanging list items, a `|` rule
//! per quote level, verbatim code, link destinations beside their text, and tables fitted to the
//! width. The whole document is in memory before a byte is drawn, because a link's destination
//! can be defined at the foot of the page; that is why `mdr` is a `std` program with a heap.

use alloc::string::String;
use alloc::vec::Vec;

use pulldown_cmark::{Alignment, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd};

use crate::Sink;

/// How the renderer is asked to draw.
///
/// # Examples
///
/// Colour on, which is what a terminal wants:
///
/// ```
/// # use documentation::{Sink, Style, render};
/// # struct Buf(String);
/// # impl Sink for Buf {
/// #     fn put(&mut self, bytes: &[u8]) { self.0.push_str(core::str::from_utf8(bytes).unwrap()); }
/// # }
/// let mut out = Buf(String::new());
/// render("a **b** c\n", Style { width: 80, color: true }, &mut out);
/// assert!(out.0.contains("\x1b[1mb"));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Style {
    /// The terminal's width in columns. Text wraps to it; a code block and a long word do not.
    pub width: u16,
    /// Whether to emit SGR escape sequences.
    ///
    /// **This is a capability question wearing a boolean's clothes.** `mdr` writes to whatever
    /// endpoint it was handed and has no way to ask what is behind it, which is the sink contract
    /// working as designed. So the answer is not sniffed the way `isatty` sniffs it: the *shell*
    /// knows whether it wired this program to a terminal or to a pipe, and passes the answer in.
    pub color: bool,
}

impl Style {
    /// Eighty columns with colour, which is the serial console.
    pub const DEFAULT: Style = Style {
        width: 80,
        color: true,
    };
}

/// A run of output that shares one terminal attribute.
///
/// Every sequence here is one `video_terminal` draws: bold, the eight colours, the underline
/// [`Attr::Emph`] uses and the strikethrough [`Attr::Strike`] uses.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Attr {
    /// Body text.
    Plain,
    /// A heading.
    Heading,
    /// `**strong**`.
    Strong,
    /// `*emphasis*`.
    Emph,
    /// `~~strikethrough~~`.
    Strike,
    /// An inline `` `code` `` span.
    Code,
    /// A code block's contents.
    Fence,
    /// A link's destination.
    Link,
    /// A list bullet or number, and a block quote's rule.
    Marker,
}

impl Attr {
    /// The escape sequence that turns this attribute on, assuming a reset came first.
    ///
    /// Each is a single-parameter sequence on purpose: `video_terminal` drops any CSI carrying more
    /// than sixteen parameters, and keeping every sequence to one parameter means no terminal's
    /// limit ever has to be remembered.
    const fn sgr(self) -> &'static [u8] {
        match self {
            Attr::Plain => b"",
            Attr::Heading | Attr::Strong => b"\x1b[1m",
            Attr::Emph => b"\x1b[4m",
            Attr::Strike => b"\x1b[9m",
            Attr::Code => b"\x1b[36m",
            Attr::Fence => b"\x1b[32m",
            Attr::Link => b"\x1b[34m",
            Attr::Marker => b"\x1b[33m",
        }
    }
}

/// **Render `markdown` to `out`**, wrapped to `style.width`.
///
/// # Examples
///
/// ```
/// use documentation::{Sink, Style, render};
///
/// struct Buf(String);
/// impl Sink for Buf {
///     fn put(&mut self, bytes: &[u8]) {
///         self.0.push_str(core::str::from_utf8(bytes).unwrap());
///     }
/// }
///
/// let mut out = Buf(String::new());
/// render(
///     "# Title\n\nSome **strong** text.\n",
///     Style { width: 80, color: false },
///     &mut out,
/// );
/// assert_eq!(out.0, "TITLE\n\n  Some strong text.\n");
/// ```
///
/// Name: provisional (milestone 784 (reuse what the tree wrote), item (f2), 2026-10-20 UTC).
pub fn render(markdown: &str, style: Style, out: &mut impl Sink) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let mut layout = Layout::new(style);
    for event in Parser::new_ext(markdown, options) {
        layout.event(event, out);
    }
    // `close_line` and not `open_block`: a blank line owed to a block that never arrived is a blank
    // line nobody wants at the foot of a page.
    layout.close_line(out);
}

/// How a table column is aligned, read off the delimiter row (`|---|:--:|---:|`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Align {
    Left,
    Center,
    Right,
}

/// A table held until its widths are known: only the last row says how wide a column is.
struct Table {
    align: Vec<Align>,
    rows: Vec<Vec<String>>,
    /// The cell being read, while one is open.
    cell: Option<String>,
}

/// One open link or image: where it points, whether its destination is worth showing, and whether
/// it is an image.
struct Link {
    dest: String,
    show: bool,
    image: bool,
}

/// The layout state: where the cursor is, what the block it is in looks like, and what is open.
struct Layout {
    style: Style,

    // Output cursor state.
    col: usize,
    /// The column text starts at on a wrapped line, including the block quote rule.
    start: usize,
    /// The left margin the current block indents to, before any quote rule.
    margin: usize,
    /// Block quote depth of the block being emitted.
    quote: usize,
    attr: Attr,
    /// Whether words are being uppercased, which only a level-one heading turns on.
    upper_on: bool,
    /// A space is owed before the next word, because words are separated on output rather than
    /// copied with their source spacing.
    space: bool,
    /// Anything at all has been emitted, so a blank line would separate rather than lead.
    started: bool,
    /// A blank line is owed before the next block.
    gap: bool,

    // Block state.
    /// Inside a code block, where text is lines to print verbatim.
    code: bool,
    /// No line of the open code block has been printed yet.
    code_first: bool,
    /// The margin a list item's continuation lines align to, or `None` outside a list.
    hang: Option<usize>,
    /// Each open list item's enclosing `hang`, restored when the item ends.
    hangs: Vec<Option<usize>>,
    /// Each open list's next number, or `None` for a bullet list.
    lists: Vec<Option<u64>>,
    /// A list item's marker was just written, so the block that opens its text continues the
    /// marker's line instead of starting one.
    item_fresh: bool,
    /// The attribute each open span asked for, innermost last.
    attrs: Vec<Attr>,
    links: Vec<Link>,
    table: Option<Table>,
}

impl Layout {
    fn new(style: Style) -> Layout {
        Layout {
            style,
            col: 0,
            start: 0,
            margin: BODY,
            quote: 0,
            attr: Attr::Plain,
            upper_on: false,
            space: false,
            started: false,
            gap: false,
            code: false,
            code_first: false,
            hang: None,
            hangs: Vec::new(),
            lists: Vec::new(),
            item_fresh: false,
            attrs: Vec::new(),
            links: Vec::new(),
            table: None,
        }
    }

    /// The attribute running text takes here: the innermost span's, or plain.
    fn base(&self) -> Attr {
        self.attrs.last().copied().unwrap_or(Attr::Plain)
    }

    fn event(&mut self, event: Event<'_>, out: &mut impl Sink) {
        match event {
            Event::Start(tag) => self.start_tag(tag, out),
            Event::End(tag) => self.end_tag(tag, out),
            Event::Text(text) => self.text(&text, out),
            Event::Code(text) => {
                if let Some(cell) = self.cell() {
                    cell.push_str(&text);
                } else {
                    // One unbreakable unit, like a word: a code span split across lines would read as
                    // two pieces of code.
                    self.unit(text.as_bytes(), Attr::Code, out);
                }
            }
            // HTML is not interpreted; a terminal shows it as the text it is.
            Event::Html(text) | Event::InlineHtml(text) => self.text(&text, out),
            Event::SoftBreak => {
                if let Some(cell) = self.cell() {
                    cell.push(' ');
                } else {
                    self.space = true;
                }
            }
            Event::HardBreak => self.newline(out),
            Event::Rule => self.rule(out),
            // Footnotes, task lists and maths are not enabled, so the parser does not produce these.
            Event::FootnoteReference(_)
            | Event::TaskListMarker(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_) => {}
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>, out: &mut impl Sink) {
        match tag {
            Tag::Paragraph | Tag::HtmlBlock => self.open_text_block(out),
            Tag::Heading { level, .. } => self.heading(level, out),
            Tag::BlockQuote(_) => {
                self.item_fresh = false;
                self.close_line(out);
                self.quote += 1;
            }
            // A fenced block's info string (`bash` in a ```bash line) is a tag for tools rather
            // than content, so it is not shown.
            // The blank line owed before it is spent by its first line, so an empty block owes
            // the page nothing.
            Tag::CodeBlock(_) => {
                self.item_fresh = false;
                self.close_line(out);
                self.margin = CODE;
                self.code = true;
                self.code_first = true;
            }
            // The first item's `open_block` ends any open line, so the list itself writes nothing.
            Tag::List(first) => {
                self.item_fresh = false;
                self.lists.push(first);
            }
            Tag::Item => self.item(out),
            Tag::Table(align) => {
                self.item_fresh = false;
                self.close_line(out);
                self.table = Some(Table {
                    align: align
                        .into_iter()
                        .map(|a| match a {
                            Alignment::Center => Align::Center,
                            Alignment::Right => Align::Right,
                            Alignment::Left | Alignment::None => Align::Left,
                        })
                        .collect(),
                    rows: Vec::new(),
                    cell: None,
                });
            }
            Tag::TableHead | Tag::TableRow => {
                if let Some(t) = self.table.as_mut() {
                    t.rows.push(Vec::new());
                }
            }
            Tag::TableCell => {
                if let Some(t) = self.table.as_mut() {
                    t.cell = Some(String::new());
                }
            }
            Tag::Emphasis => self.attrs.push(Attr::Emph),
            Tag::Strong => self.attrs.push(Attr::Strong),
            Tag::Strikethrough => self.attrs.push(Attr::Strike),
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => self.links.push(Link {
                // An autolink's text is its destination, so showing both prints it twice.
                show: !matches!(link_type, LinkType::Autolink | LinkType::Email),
                dest: dest_url.into_string(),
                image: false,
            }),
            Tag::Image { dest_url, .. } => {
                if let Some(cell) = self.cell() {
                    cell.push_str("[image: ");
                } else {
                    self.unit(b"[image:", Attr::Marker, out);
                    self.space = true;
                }
                self.links.push(Link {
                    dest: dest_url.into_string(),
                    show: true,
                    image: true,
                });
            }
            // Not enabled; nothing produces them.
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript
            | Tag::MetadataBlock(_) => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd, out: &mut impl Sink) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock => {
                self.close_line(out);
                self.gap = true;
            }
            TagEnd::Heading(_) => {
                self.attrs.pop();
                self.upper_on = false;
                self.close_line(out);
                self.gap = true;
            }
            TagEnd::BlockQuote(_) => {
                self.close_line(out);
                self.quote = self.quote.saturating_sub(1);
                self.gap = true;
            }
            TagEnd::CodeBlock => {
                self.code = false;
                self.margin = BODY;
                // An empty block printed nothing, so the blank line owed before it is still owed
                // and no second one is.
                if !self.code_first {
                    self.gap = true;
                }
                self.code_first = false;
            }
            TagEnd::List(_) => {
                self.lists.pop();
                // A nested list ends inside its parent's item, whose next sibling follows on the
                // next line; only the outermost list's end owes the page a blank line.
                if self.lists.is_empty() {
                    self.close_line(out);
                    self.gap = true;
                }
            }
            TagEnd::Item => {
                self.item_fresh = false;
                self.close_line(out);
                self.hang = self.hangs.pop().flatten();
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    self.flush_table(&t, out);
                }
            }
            TagEnd::TableHead | TagEnd::TableRow => {}
            TagEnd::TableCell => {
                if let Some(t) = self.table.as_mut()
                    && let Some(cell) = t.cell.take()
                    && let Some(row) = t.rows.last_mut()
                {
                    row.push(cell);
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.attrs.pop();
            }
            TagEnd::Link | TagEnd::Image => self.end_link(out),
            TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::MetadataBlock(_) => {}
        }
    }

    /// The table cell being read, if one is open.
    fn cell(&mut self) -> Option<&mut String> {
        self.table.as_mut().and_then(|t| t.cell.as_mut())
    }

    // ---- block emitters ------------------------------------------------------------------

    /// A paragraph or an HTML block: running text at the body margin, or after a list item's marker.
    fn open_text_block(&mut self, out: &mut impl Sink) {
        if self.item_fresh {
            self.item_fresh = false;
            return;
        }
        if self.hang.is_none() {
            self.margin = BODY;
        }
        self.open_block(out);
    }

    /// Open a heading. Level one is the page's own name and is uppercased, as a man page does, so
    /// a reader scanning a screenful finds the title; lower levels are sentences and keep their case.
    fn heading(&mut self, level: HeadingLevel, out: &mut impl Sink) {
        let level = level as usize;
        self.item_fresh = false;
        self.open_block(out);
        self.hang = None;
        self.margin = if level <= 2 {
            0
        } else {
            BODY + 2 * (level - 3)
        };
        self.upper_on = level == 1;
        self.attrs.push(Attr::Heading);
    }

    fn rule(&mut self, out: &mut impl Sink) {
        self.item_fresh = false;
        self.open_block(out);
        self.hang = None;
        self.margin = BODY;
        self.line_start(out);
        self.set(Attr::Marker, out);
        let n = (self.style.width as usize).saturating_sub(self.start);
        dashes(out, n);
        self.col += n;
        self.close_line(out);
        self.gap = true;
    }

    /// One line of a code block, verbatim: its line breaks are its meaning, so it is never wrapped
    /// or re-spaced, and a line wider than the terminal runs over as it does in every pager.
    fn code_line(&mut self, line: &[u8], out: &mut impl Sink) {
        if self.code_first {
            self.code_first = false;
            self.open_block(out);
        }
        self.line_start(out);
        self.set(Attr::Fence, out);
        out.put(line);
        self.col += visible(line);
        // `newline` rather than `close_line`: an empty line inside a code block is a line of the
        // code, and the margin `line_start` wrote does not end it.
        self.newline(out);
    }

    /// Open a list item. A bullet is written `-` whichever of `-`, `*` and `+` the author typed, so
    /// one thing has one spelling on screen, and continuation lines hang under the item's text.
    fn item(&mut self, out: &mut impl Sink) {
        self.item_fresh = false;
        self.open_block(out);
        // A nested item's marker sits where its parent's text does.
        let margin = self.hang.unwrap_or(BODY);
        self.hangs.push(self.hang);
        self.hang = None;
        self.margin = margin;
        self.line_start(out);
        self.set(Attr::Marker, out);
        let marker: String = match self.lists.last_mut() {
            Some(Some(n)) => {
                let s = alloc::format!("{n}. ");
                *n += 1;
                s
            }
            _ => String::from("- "),
        };
        out.put(marker.as_bytes());
        self.col += marker.len();
        self.hang = Some(margin + marker.len());
        self.start = self.col;
        self.space = false;
        self.item_fresh = true;
    }

    /// Close a link or image and show its destination: a terminal cannot follow a link or draw a
    /// picture, so the path is the only thing its reader can act on.
    fn end_link(&mut self, out: &mut impl Sink) {
        let Some(link) = self.links.pop() else {
            return;
        };
        if let Some(cell) = self.cell() {
            if link.image {
                cell.push(']');
            }
            if link.show && !link.dest.is_empty() {
                cell.push(' ');
                cell.push_str(&link.dest);
            }
            return;
        }
        if link.image {
            self.unit(b"]", Attr::Marker, out);
        }
        if link.show && !link.dest.is_empty() {
            self.space = true;
            self.unit_wrapped(link.dest.as_bytes(), Attr::Link, out);
        }
    }

    // ---- text ----------------------------------------------------------------------------

    fn text(&mut self, text: &str, out: &mut impl Sink) {
        if self.code {
            for line in text.split_inclusive('\n') {
                let line = line.strip_suffix('\n').unwrap_or(line);
                let line = line.strip_suffix('\r').unwrap_or(line);
                self.code_line(line.as_bytes(), out);
            }
            return;
        }
        if let Some(cell) = self.cell() {
            cell.push_str(text);
            return;
        }
        let attr = self.base();
        let bytes = text.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i].is_ascii_whitespace() {
                self.space = true;
                i += 1;
                continue;
            }
            let word = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            self.unit(&bytes[word..i], attr, out);
        }
    }

    /// Place one unbreakable run at the cursor, wrapping first if it does not fit.
    fn unit(&mut self, bytes: &[u8], attr: Attr, out: &mut impl Sink) {
        if self.item_fresh {
            self.item_fresh = false;
        }
        let vis = visible(bytes);
        self.before_unit(vis, out);
        self.set(attr, out);
        if self.upper_on {
            let mut upper = bytes.to_vec();
            upper.make_ascii_uppercase();
            out.put(&upper);
        } else {
            out.put(bytes);
        }
        self.col += vis;
        self.space = false;
    }

    /// A run that may be broken across lines if it is longer than the terminal, which is what a
    /// long URL needs and what a word does not want.
    fn unit_wrapped(&mut self, bytes: &[u8], attr: Attr, out: &mut impl Sink) {
        let width = self.style.width as usize;
        let mut k = 0;
        while k < bytes.len() {
            let room = width.saturating_sub(self.col.max(self.start)).max(1);
            let mut take = 0;
            let mut n = 0;
            while k + take < bytes.len() && n < room {
                take += char_len(bytes, k + take);
                n += 1;
            }
            self.unit(&bytes[k..k + take], attr, out);
            k += take;
            if k < bytes.len() {
                self.newline(out);
            }
        }
    }

    fn before_unit(&mut self, vis: usize, out: &mut impl Sink) {
        if self.col > self.start && self.space {
            if self.col + 1 + vis > self.style.width as usize {
                self.newline(out);
            } else {
                self.set(Attr::Plain, out);
                out.put(b" ");
                self.col += 1;
            }
        }
        if self.col == 0 {
            self.line_start(out);
        }
        self.space = false;
    }

    // ---- tables --------------------------------------------------------------------------

    fn flush_table(&mut self, t: &Table, out: &mut impl Sink) {
        let ncols = t.rows.iter().map(Vec::len).max().unwrap_or(0);
        if ncols == 0 {
            return;
        }
        self.open_block(out);
        let w = self.table_widths(t, ncols);
        let margin = self.hang.unwrap_or(BODY);
        self.margin = margin;
        for (r, row) in t.rows.iter().enumerate() {
            self.line_start(out);
            for (c, width) in w.iter().enumerate() {
                if c > 0 {
                    self.set(Attr::Marker, out);
                    out.put(b" | ");
                    self.col += 3;
                }
                let text = row.get(c).map_or("", String::as_str);
                // The first row is the header: CommonMark has no table without one.
                let attr = if r == 0 { Attr::Strong } else { Attr::Plain };
                let align = t.align.get(c).copied().unwrap_or(Align::Left);
                self.write_table_cell(text.as_bytes(), *width, attr, align, out);
            }
            self.close_line(out);
            if r == 0 {
                self.write_table_rule(&w, out);
            }
        }
        self.gap = true;
    }

    /// A table's column widths: each column's natural width, then the widest shrunk until a row
    /// fits the page. Shrinking the widest keeps a table of one prose column and short labels
    /// readable: the prose loses characters and the labels do not.
    fn table_widths(&self, t: &Table, ncols: usize) -> Vec<usize> {
        let mut w = alloc::vec![0usize; ncols];
        for row in &t.rows {
            for (c, cell) in row.iter().enumerate() {
                w[c] = w[c].max(visible(cell.as_bytes()));
            }
        }
        let avail =
            (self.style.width as usize).saturating_sub(self.hang.unwrap_or(BODY) + 2 * self.quote);
        // Each column costs its width plus the three bytes of " | " that follow it, less the last.
        let overhead = 3 * (ncols - 1);
        let mut total: usize = w.iter().sum::<usize>() + overhead;
        while total > avail {
            let (widest, _) = w
                .iter()
                .enumerate()
                .max_by_key(|&(_, v)| *v)
                .unwrap_or((0, &0));
            if w[widest] <= MIN_CELL {
                break;
            }
            w[widest] -= 1;
            total -= 1;
        }
        w
    }

    /// Write one table cell in `width` characters. A cell is never wrapped: a wrapped cell would
    /// not line up with its column, which is the only thing a table is for.
    fn write_table_cell(
        &mut self,
        text: &[u8],
        width: usize,
        attr: Attr,
        align: Align,
        out: &mut impl Sink,
    ) {
        // Counted before any character goes out: a right- or centre-aligned cell pads in front.
        let mut n = 0;
        let mut end = 0;
        while end < text.len() && n < width {
            end += char_len(text, end);
            n += 1;
        }
        let slack = width - n;
        let (before, after) = match align {
            Align::Left => (0, slack),
            Align::Right => (slack, 0),
            Align::Center => (slack / 2, slack - slack / 2),
        };
        self.set(attr, out);
        pad(out, before);
        out.put(&text[..end]);
        pad(out, after);
        self.col += width;
    }

    /// The rule under a table's header row: dashes the width of each column, `-+-` between them.
    fn write_table_rule(&mut self, w: &[usize], out: &mut impl Sink) {
        self.line_start(out);
        self.set(Attr::Marker, out);
        for (c, width) in w.iter().enumerate() {
            if c > 0 {
                out.put(b"-+-");
                self.col += 3;
            }
            dashes(out, *width);
            self.col += *width;
        }
        self.close_line(out);
    }

    // ---- cursor --------------------------------------------------------------------------

    /// Open a line: the margin, then one `|` rule per level of block quote.
    fn line_start(&mut self, out: &mut impl Sink) {
        let margin = self.hang.unwrap_or(self.margin);
        pad(out, margin);
        self.col = margin;
        for _ in 0..self.quote {
            self.set(Attr::Marker, out);
            out.put(b"| ");
            self.col += 2;
        }
        self.start = self.col;
        self.started = true;
    }

    fn newline(&mut self, out: &mut impl Sink) {
        // The attribute is reset *before* the newline, never after, because a terminal that is left
        // bold at end of line paints the rest of the row and the next prompt with it.
        self.set(Attr::Plain, out);
        out.put(b"\n");
        self.col = 0;
        self.space = false;
    }

    /// End whatever line is open, leaving any blank line owed to the next block still owed.
    fn close_line(&mut self, out: &mut impl Sink) {
        if self.col > 0 {
            self.newline(out);
        }
        self.space = false;
    }

    /// Begin a block: end the open line and spend the blank line owed to it.
    ///
    /// The separation from [`Layout::close_line`] is what stops the end of one block and the start
    /// of the next from each emitting a gap, which is one blank line too many.
    fn open_block(&mut self, out: &mut impl Sink) {
        self.close_line(out);
        if self.gap && self.started {
            out.put(b"\n");
        }
        self.gap = false;
    }

    fn set(&mut self, attr: Attr, out: &mut impl Sink) {
        if !self.style.color || attr == self.attr {
            self.attr = attr;
            return;
        }
        if self.attr != Attr::Plain {
            out.put(b"\x1b[0m");
        }
        out.put(attr.sgr());
        self.attr = attr;
    }
}

// ---- free helpers ------------------------------------------------------------------------

/// The left margin body text takes. Two rather than a man page's five, because the other terminal
/// this system has is 32 columns wide and five of them is a sixth of the screen.
const BODY: usize = 2;
/// The margin a code block takes, two in from body text so it reads as set apart without a border
/// character that a 32-column terminal cannot spare.
const CODE: usize = 4;
/// The narrowest a table column may be squeezed before the renderer gives up and lets the row
/// overflow, because a two-character column tells a reader nothing.
const MIN_CELL: usize = 3;

fn pad(out: &mut impl Sink, n: usize) {
    repeat(out, b' ', n);
}

fn dashes(out: &mut impl Sink, n: usize) {
    repeat(out, b'-', n);
}

fn repeat(out: &mut impl Sink, byte: u8, n: usize) {
    let run = [byte; 64];
    let mut left = n;
    while left > 0 {
        let take = left.min(64);
        out.put(&run[..take]);
        left -= take;
    }
}

/// Visible width: bytes, less UTF-8 continuation bytes, which occupy no column of their own.
fn visible(bytes: &[u8]) -> usize {
    bytes.iter().filter(|&&b| (b & 0xC0) != 0x80).count()
}

/// How many bytes the character starting at `i` occupies.
fn char_len(bytes: &[u8], i: usize) -> usize {
    let mut n = 1;
    while i + n < bytes.len() && (bytes[i + n] & 0xC0) == 0x80 {
        n += 1;
    }
    n
}
