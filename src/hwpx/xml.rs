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
