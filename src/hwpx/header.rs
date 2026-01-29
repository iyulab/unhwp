//! HWPX header.xml parser.
//!
//! Extracts document options including distribution flag.

use crate::error::Result;
use quick_xml::events::Event;
use quick_xml::Reader;

/// Parses header.xml to extract distribution flag.
pub fn parse_header(xml: &str) -> Result<bool> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut is_distribution = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local_name = e.local_name();
                let name = std::str::from_utf8(local_name.as_ref()).unwrap_or("");

                // Check for hh:docOption or docOption
                if name == "docOption" || name.ends_with(":docOption") {
                    for attr in e.attributes().flatten() {
                        if attr.key.local_name().as_ref() == b"distribute" {
                            if let Ok(val) = std::str::from_utf8(&attr.value) {
                                is_distribution = val == "true" || val == "1";
                            }
                        }
                    }
                    return Ok(is_distribution);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(crate::error::Error::XmlParse(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(is_distribution)
}
