//! FileHeader parsing for HWP 5.0 documents.

use crate::error::{Error, Result};

/// HWP 5.0 file header signature.
const HWP_SIGNATURE: &[u8] = b"HWP Document File";

/// FileHeader size is always 256 bytes.
const FILE_HEADER_SIZE: usize = 256;

/// Property flags bit positions.
mod flags {
    /// Document is compressed
    pub const COMPRESSED: u32 = 1 << 0;
    /// Document is encrypted
    pub const ENCRYPTED: u32 = 1 << 1;
    /// Document is a distribution document
    pub const DISTRIBUTION: u32 = 1 << 2;
    /// Script present
    pub const SCRIPT: u32 = 1 << 3;
    /// DRM protected
    pub const DRM: u32 = 1 << 4;
    /// XML template storage
    pub const XML_TEMPLATE: u32 = 1 << 5;
    /// Document history present
    pub const HISTORY: u32 = 1 << 6;
    /// Digital signature present
    pub const SIGNATURE: u32 = 1 << 7;
    /// Public key encryption
    pub const PUBLIC_KEY_ENCRYPT: u32 = 1 << 8;
    /// Reserved space to store digital signature
    pub const SIGNATURE_RESERVED: u32 = 1 << 9;
    /// Certificate DRM
    pub const CERTIFICATE_DRM: u32 = 1 << 10;
    /// CCL document
    pub const CCL: u32 = 1 << 11;
    /// Mobile optimized
    pub const MOBILE: u32 = 1 << 12;
    /// Privacy protection
    pub const PRIVACY: u32 = 1 << 13;
    /// Change tracking enabled
    pub const TRACK_CHANGES: u32 = 1 << 14;
    /// KOGL copyright
    pub const KOGL: u32 = 1 << 15;
    /// Video control present
    pub const VIDEO_CONTROL: u32 = 1 << 16;
    /// Order field control present
    pub const ORDER_FIELD: u32 = 1 << 17;
}

/// HWP 5.0 FileHeader structure.
#[derive(Debug, Clone)]
pub struct FileHeader {
    /// Document version (major.minor.build.revision)
    pub version: Version,
    /// Property flags
    pub properties: u32,
    /// License information
    pub license: Option<String>,
}

impl FileHeader {
    /// Parses a FileHeader from raw bytes.
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < FILE_HEADER_SIZE {
            return Err(Error::InvalidData(format!(
                "FileHeader too small: {} bytes, expected {}",
                data.len(),
                FILE_HEADER_SIZE
            )));
        }

        // Verify signature (first 32 bytes, null-padded)
        if !data[..17].eq(HWP_SIGNATURE) {
            return Err(Error::InvalidData("Invalid HWP signature".into()));
        }

        // Version at offset 0x20 (32), 4 bytes little-endian
        // Format: [revision, build, minor, major]
        let version = Version {
            major: data[35],
            minor: data[34],
            build: data[33],
            revision: data[32],
        };

        // Properties at offset 0x24 (36), 4 bytes little-endian
        let properties = u32::from_le_bytes([data[36], data[37], data[38], data[39]]);

        Ok(Self {
            version,
            properties,
            license: None,
        })
    }

    /// Returns the version as a string (e.g., "5.1.0.1").
    pub fn version_string(&self) -> String {
        self.version.to_string()
    }

    /// Returns true if the document is compressed.
    pub fn is_compressed(&self) -> bool {
        self.properties & flags::COMPRESSED != 0
    }

    /// Returns true if the document is encrypted.
    pub fn is_encrypted(&self) -> bool {
        self.properties & flags::ENCRYPTED != 0
    }

    /// Returns true if the document is a distribution document.
    pub fn is_distribution(&self) -> bool {
        self.properties & flags::DISTRIBUTION != 0
    }

    /// Returns true if the document has DRM protection.
    pub fn is_drm_protected(&self) -> bool {
        self.properties & flags::DRM != 0
    }

    /// Returns true if scripts are present.
    pub fn has_scripts(&self) -> bool {
        self.properties & flags::SCRIPT != 0
    }

    /// Returns true if change tracking is enabled.
    pub fn has_track_changes(&self) -> bool {
        self.properties & flags::TRACK_CHANGES != 0
    }
}

/// HWP document version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub build: u8,
    pub revision: u8,
}

impl Version {
    /// Creates a new version.
    pub fn new(major: u8, minor: u8, build: u8, revision: u8) -> Self {
        Self {
            major,
            minor,
            build,
            revision,
        }
    }

    /// Returns true if this version is at least the specified version.
    pub fn at_least(&self, major: u8, minor: u8, build: u8, revision: u8) -> bool {
        (self.major, self.minor, self.build, self.revision)
            >= (major, minor, build, revision)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}.{}",
            self.major, self.minor, self.build, self.revision
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_display() {
        let v = Version::new(5, 1, 0, 1);
        assert_eq!(v.to_string(), "5.1.0.1");
    }

    #[test]
    fn test_version_comparison() {
        let v = Version::new(5, 1, 0, 1);
        assert!(v.at_least(5, 0, 0, 0));
        assert!(v.at_least(5, 1, 0, 0));
        assert!(v.at_least(5, 1, 0, 1));
        assert!(!v.at_least(5, 1, 0, 2));
        assert!(!v.at_least(5, 2, 0, 0));
    }

    #[test]
    fn test_parse_header() {
        let mut data = vec![0u8; 256];
        // Set signature
        data[..17].copy_from_slice(b"HWP Document File");
        // Set version 5.1.0.1 at offset 32
        data[32] = 1; // revision
        data[33] = 0; // build
        data[34] = 1; // minor
        data[35] = 5; // major
        // Set properties (compressed) at offset 36
        data[36] = 0x01;

        let header = FileHeader::parse(&data).unwrap();
        assert_eq!(header.version.major, 5);
        assert_eq!(header.version.minor, 1);
        assert!(header.is_compressed());
        assert!(!header.is_encrypted());
    }
}
