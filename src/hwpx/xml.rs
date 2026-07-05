//! Shared quick-xml decoding helpers for HWPX parsing.
//!
//! quick-xml 0.40+ redesigned entity handling: a text node such as
//! `A &amp; B` is no longer delivered as a single `Event::Text` with the
//! entity resolved inline. Instead the reader splits it into
//! `Text("A ")`, `GeneralRef("amp")`, `Text(" B")`. Callers that accumulate
//! text therefore MUST handle [`Event::GeneralRef`] as well, or every
//! `&amp; &lt; &gt; &#NN;` in a document silently disappears.
//!
//! These helpers centralize that logic so every text-collecting loop stays
//! consistent.

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesRef, BytesText};
use quick_xml::XmlVersion;

/// Decodes the content of an [`Event::Text`] event.
///
/// In quick-xml 0.40+ text events no longer carry entity references (those are
/// emitted as separate [`Event::GeneralRef`] events — see [`resolve_general_ref`]),
/// so this only decodes the raw bytes and normalizes end-of-line sequences per
/// the XML 1.0 rules. Decoding failures yield an empty string rather than
/// aborting extraction, matching the library's graceful-degradation goal.
///
/// [`Event::Text`]: quick_xml::events::Event::Text
/// [`Event::GeneralRef`]: quick_xml::events::Event::GeneralRef
pub(crate) fn decode_text(t: &BytesText) -> String {
    t.xml_content(XmlVersion::Implicit1_0)
        .map(|c| c.into_owned())
        .unwrap_or_default()
}

/// Resolves an [`Event::GeneralRef`] entity reference to its string value.
///
/// Handles numeric character references (`&#48;`, `&#x30;`) and the five
/// predefined XML entities (`&amp; &lt; &gt; &quot; &apos;`). An unrecognized
/// entity is preserved literally as `&name;` to avoid silent data loss.
///
/// [`Event::GeneralRef`]: quick_xml::events::Event::GeneralRef
pub(crate) fn resolve_general_ref(r: &BytesRef) -> String {
    let name = match r.decode() {
        Ok(n) => n,
        Err(_) => return String::new(),
    };

    if let Some(num) = name.strip_prefix('#') {
        let codepoint = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => num.parse::<u32>().ok(),
        };
        return codepoint
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default();
    }

    if let Some(value) = resolve_predefined_entity(&name) {
        return value.to_string();
    }

    // Unknown (custom DTD) entity: HWPX does not define these, but preserve the
    // literal reference rather than dropping content.
    format!("&{name};")
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::events::BytesRef;

    fn resolve(name: &str) -> String {
        resolve_general_ref(&BytesRef::new(name))
    }

    #[test]
    fn resolves_predefined_entities() {
        assert_eq!(resolve("amp"), "&");
        assert_eq!(resolve("lt"), "<");
        assert_eq!(resolve("gt"), ">");
        assert_eq!(resolve("quot"), "\"");
        assert_eq!(resolve("apos"), "'");
    }

    #[test]
    fn resolves_numeric_character_references() {
        assert_eq!(resolve("#48"), "0"); // decimal
        assert_eq!(resolve("#x41"), "A"); // lower-case hex
        assert_eq!(resolve("#X41"), "A"); // upper-case hex
        assert_eq!(resolve("#44032"), "가"); // multi-byte codepoint (U+AC00)
    }

    #[test]
    fn unknown_entity_is_preserved_literally() {
        assert_eq!(resolve("nbsp"), "&nbsp;");
        assert_eq!(resolve("custom"), "&custom;");
    }

    #[test]
    fn malformed_references_degrade_without_panicking() {
        // Adversarial inputs must never panic — the parser's robustness goal.
        assert_eq!(resolve("#"), ""); // empty numeric ref
        assert_eq!(resolve("#x"), ""); // empty hex ref
        assert_eq!(resolve("#xZZ"), ""); // non-hex digits
        assert_eq!(resolve("#x110000"), ""); // above the Unicode maximum
        assert_eq!(resolve("#xD800"), ""); // lone surrogate (not a scalar value)
        assert_eq!(resolve("#xFFFFFFFFFFFF"), ""); // overflows u32 parse
    }
}
