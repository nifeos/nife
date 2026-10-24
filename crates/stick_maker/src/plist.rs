//! **Just enough of Apple's XML property list format to read `diskutil -plist`.**
//!
//! `diskutil`'s default table is for people, with moving column widths and no contract; `-plist` is
//! the machine-readable output Apple documents for scripts, so it is what this reads.
//!
//! Reuse: write, under §46 (thin primitives or whole subsystems). Ruled by calef on 2026-10-20
//! (UTC), "Keep ours.", on milestone 784 (reuse what the tree wrote)'s item (f). The `plist` crate
//! would bring nine crates new to the tree; `diskutil`'s grammar is eight element types and five
//! character entities. See notes/boot-stick/the-program.md, "macOS".

//! # BUGS
//!
//! - **Binary plists are not read.** `diskutil -plist` writes XML, which is all this is for.
//! - **`<data>`, `<date>` and `<real>` are kept as their raw text**, undecoded.
//! - **Numeric character references (`&#65;`) are not decoded**; `diskutil` writes named entities.

use std::collections::BTreeMap;

/// One value in a property list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// `<dict>`: keys are unique and unordered in the format; a map keeps lookups honest.
    Dict(BTreeMap<String, Value>),
    /// `<array>`.
    Array(Vec<Value>),
    /// `<string>`, entities decoded.
    String(String),
    /// `<integer>`. Signed in the format; every size `diskutil` reports fits.
    Integer(i128),
    /// `<true/>` and `<false/>`.
    Bool(bool),
    /// `<data>`, `<date>` and `<real>`, kept as text (see BUGS).
    Other(String),
}

impl Value {
    /// The value under `key`, if this is a dictionary that has one.
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Dict(map) => map.get(key),
            _ => None,
        }
    }

    /// The string under `key`.
    pub fn string(&self, key: &str) -> Option<&str> {
        match self.get(key)? {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// The boolean under `key`.
    pub fn boolean(&self, key: &str) -> Option<bool> {
        match self.get(key)? {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// The integer under `key`, as a byte count or similar non-negative quantity.
    pub fn unsigned(&self, key: &str) -> Option<u64> {
        match self.get(key)? {
            Value::Integer(n) => u64::try_from(*n).ok(),
            _ => None,
        }
    }

    /// The array under `key`, or an empty slice.
    pub fn array(&self, key: &str) -> &[Value] {
        match self.get(key) {
            Some(Value::Array(items)) => items,
            _ => &[],
        }
    }
}

/// Why a document could not be read. The position is a byte offset into the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// Byte offset where reading stopped.
    pub at: usize,
    /// What was wrong there.
    pub what: &'static str,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "property list unreadable at byte {}: {}",
            self.at, self.what
        )
    }
}

/// Read a whole XML property list and return its one top-level value.
pub fn parse(text: &str) -> Result<Value, Error> {
    let mut reader = Reader { text, at: 0 };
    reader.skip_prolog()?;
    let value = reader.value()?;
    Ok(value)
}

struct Reader<'a> {
    text: &'a str,
    at: usize,
}

impl<'a> Reader<'a> {
    fn fail<T>(&self, what: &'static str) -> Result<T, Error> {
        Err(Error { at: self.at, what })
    }

    fn rest(&self) -> &'a str {
        &self.text[self.at..]
    }

    fn skip_space(&mut self) {
        let trimmed = self.rest().trim_start();
        self.at = self.text.len() - trimmed.len();
    }

    /// Skip `<?xml ...?>`, `<!DOCTYPE ...>`, comments and the `<plist ...>` wrapper, leaving the
    /// reader at the first value element.
    fn skip_prolog(&mut self) -> Result<(), Error> {
        loop {
            self.skip_space();
            let rest = self.rest();
            if rest.starts_with("<?") {
                self.skip_past("?>")?;
            } else if rest.starts_with("<!--") {
                self.skip_past("-->")?;
            } else if rest.starts_with("<!") {
                self.skip_past(">")?;
            } else if rest.starts_with("<plist") {
                self.skip_past(">")?;
                return Ok(());
            } else if rest.starts_with('<') {
                // A bare value with no wrapper: accepted, since nothing is lost by it.
                return Ok(());
            } else {
                return self.fail("expected a property list");
            }
        }
    }

    fn skip_past(&mut self, needle: &str) -> Result<(), Error> {
        match self.rest().find(needle) {
            Some(i) => {
                self.at += i + needle.len();
                Ok(())
            }
            None => self.fail("unterminated markup"),
        }
    }

    /// Read `<name>` or `<name/>` and report which, with the name.
    fn open_tag(&mut self) -> Result<(&'a str, bool), Error> {
        self.skip_space();
        let rest = self.rest();
        if !rest.starts_with('<') || rest.starts_with("</") {
            return self.fail("expected an opening tag");
        }
        let Some(end) = rest.find('>') else {
            return self.fail("unterminated tag");
        };
        let inner = &rest[1..end];
        let (inner, empty) = match inner.strip_suffix('/') {
            Some(stripped) => (stripped, true),
            None => (inner, false),
        };
        let name = inner.split_whitespace().next().unwrap_or("");
        self.at += end + 1;
        Ok((name, empty))
    }

    fn close_tag(&mut self, name: &str) -> Result<(), Error> {
        self.skip_space();
        let expected_len = name.len() + 3;
        let rest = self.rest();
        if rest.len() >= expected_len
            && rest.starts_with("</")
            && rest.get(2..2 + name.len()) == Some(name)
            && rest
                .get(2 + name.len()..)
                .is_some_and(|r| r.trim_start().starts_with('>'))
        {
            let gt = rest.find('>').unwrap_or(expected_len - 1);
            self.at += gt + 1;
            Ok(())
        } else {
            self.fail("expected a closing tag")
        }
    }

    /// Text up to the next `<`, entities decoded.
    fn text_until_tag(&mut self) -> Result<String, Error> {
        let rest = self.rest();
        let Some(end) = rest.find('<') else {
            return self.fail("unterminated text");
        };
        let raw = &rest[..end];
        self.at += end;
        Ok(decode_entities(raw))
    }

    fn value(&mut self) -> Result<Value, Error> {
        let (name, empty) = self.open_tag()?;
        let name = name.to_owned();
        match (name.as_str(), empty) {
            ("true", true) => Ok(Value::Bool(true)),
            ("false", true) => Ok(Value::Bool(false)),
            ("dict", true) => Ok(Value::Dict(BTreeMap::new())),
            ("array", true) => Ok(Value::Array(Vec::new())),
            ("string", true) => Ok(Value::String(String::new())),
            ("dict", false) => {
                let mut map = BTreeMap::new();
                loop {
                    self.skip_space();
                    if self.rest().starts_with("</") {
                        self.close_tag("dict")?;
                        return Ok(Value::Dict(map));
                    }
                    let (tag, empty) = self.open_tag()?;
                    if tag != "key" {
                        return self.fail("a dict holds a key before each value");
                    }
                    let key = if empty {
                        String::new()
                    } else {
                        let key = self.text_until_tag()?;
                        self.close_tag("key")?;
                        key
                    };
                    let value = self.value()?;
                    map.insert(key, value);
                }
            }
            ("array", false) => {
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    if self.rest().starts_with("</") {
                        self.close_tag("array")?;
                        return Ok(Value::Array(items));
                    }
                    items.push(self.value()?);
                }
            }
            ("string", false) => {
                let text = self.text_until_tag()?;
                self.close_tag("string")?;
                Ok(Value::String(text))
            }
            ("integer", false) => {
                let text = self.text_until_tag()?;
                self.close_tag("integer")?;
                match text.trim().parse::<i128>() {
                    Ok(n) => Ok(Value::Integer(n)),
                    Err(_) => self.fail("an integer that is not a number"),
                }
            }
            ("data" | "date" | "real", false) => {
                let text = self.text_until_tag()?;
                self.close_tag(&name)?;
                Ok(Value::Other(text.trim().to_owned()))
            }
            _ => self.fail("an element this reader does not know"),
        }
    }
}

fn decode_entities(raw: &str) -> String {
    raw.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_shapes_diskutil_writes() {
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Internal</key>
	<false/>
	<key>RemovableMedia</key>
	<true/>
	<key>Size</key>
	<integer>67108864</integer>
	<key>MediaName</key>
	<string>Tom &amp; Jerry&apos;s &lt;stick&gt;</string>
	<key>MountPoint</key>
	<string></string>
	<key>Empty</key>
	<string/>
	<key>Partitions</key>
	<array>
		<dict>
			<key>DeviceIdentifier</key>
			<string>disk7s1</string>
		</dict>
	</array>
	<key>Nothing</key>
	<array/>
</dict>
</plist>
"#;
        let v = parse(doc).unwrap();
        assert_eq!(v.boolean("Internal"), Some(false));
        assert_eq!(v.boolean("RemovableMedia"), Some(true));
        assert_eq!(v.unsigned("Size"), Some(67_108_864));
        assert_eq!(v.string("MediaName"), Some("Tom & Jerry's <stick>"));
        assert_eq!(v.string("MountPoint"), Some(""));
        assert_eq!(v.string("Empty"), Some(""));
        assert_eq!(
            v.array("Partitions")[0].string("DeviceIdentifier"),
            Some("disk7s1")
        );
        assert!(v.array("Nothing").is_empty());
        assert!(v.array("Absent").is_empty());
    }

    #[test]
    fn refuses_rather_than_guesses() {
        assert!(parse("<plist><dict><key>a</key>").is_err());
        assert!(parse("<plist><dict><string>no key</string></dict></plist>").is_err());
        assert!(parse("<plist><integer>twelve</integer></plist>").is_err());
        assert!(parse("not xml").is_err());
    }

    /// The documents `diskutil` wrote, captured on 2026-09-19. The fixtures are trimmed to the
    /// keys the offer rule reads, with UUIDs removed and identifiers, names and sizes made
    /// neutral; every key, and every value the offer rule decides on, is as captured.
    #[test]
    fn reads_the_captured_fixtures() {
        for fixture in [
            include_str!("../tests/fixtures/macos/disk-image-info.plist"),
            include_str!("../tests/fixtures/macos/internal-info.plist"),
            include_str!("../tests/fixtures/macos/usb-hard-disk-info.plist"),
            include_str!("../tests/fixtures/macos/list-external.plist"),
            include_str!("../tests/fixtures/macos/usb-apfs-container-info.plist"),
            include_str!("../tests/fixtures/macos/usb-flash-stick-info.plist"),
            include_str!("../tests/fixtures/macos/disk-image-fat32-volume-info.plist"),
            include_str!("../tests/fixtures/macos/usb-hard-disk-volume-info.plist"),
            include_str!("../tests/fixtures/macos/usb-portable-disk-info.plist"),
        ] {
            parse(fixture).unwrap();
        }
    }

    // Milestone 326 (turn a mutation score upward), 2026-09-24; see
    // notes/mutation-testing/stick-maker.md for the mutants each of these was seen to kill.

    /// **Every element `diskutil` can write reads**, including the two shapes no captured fixture
    /// happened to hold: an empty `<dict/>` and the `<data>`, `<date>` and `<real>` this reader
    /// keeps as text rather than refusing the whole document over.
    #[test]
    fn empty_dicts_and_opaque_elements_read() {
        assert_eq!(
            parse("<plist><dict/></plist>").unwrap(),
            Value::Dict(BTreeMap::new())
        );
        for (tag, text) in [
            ("data", "AAEC"),
            ("date", "2026-09-24T00:00:00Z"),
            ("real", "1.5"),
        ] {
            let doc = format!("<plist><{tag}> {text} </{tag}></plist>");
            assert_eq!(parse(&doc).unwrap(), Value::Other(text.to_owned()), "{tag}");
        }
    }

    /// **A closing tag must name the element it closes**, whatever else surrounds it; and a
    /// closing tag that ends the document, with only `</plist>` after it, still reads.
    #[test]
    fn a_closing_tag_names_its_element() {
        assert_eq!(
            parse("<plist><string>NIFE</string></plist>").unwrap(),
            Value::String("NIFE".into())
        );
        assert!(
            parse("<plist><string>a</strung></plist>").is_err(),
            "same length"
        );
        assert!(
            parse("<plist><string>a<!string></plist>").is_err(),
            "not a closing tag"
        );
    }

    /// **A refusal says where and what**, because it is shown to the person whose `diskutil`
    /// wrote something this cannot read: a key with no value fails at the `</dict>` standing
    /// where the value should be, and says an opening tag was expected there.
    #[test]
    fn a_refusal_names_the_byte_and_the_reason() {
        let doc = "<plist><dict><key>a</key></dict></plist>";
        let err = parse(doc).unwrap_err();
        assert_eq!(err.at, doc.find("</dict>").unwrap());
        assert_eq!(err.what, "expected an opening tag");
        assert_eq!(
            err.to_string(),
            "property list unreadable at byte 25: expected an opening tag"
        );
    }
}
