namespace Unhwp;

/// <summary>
/// Why an unhwp call failed, so callers can branch on the reason instead of matching
/// on message text.
/// </summary>
/// <remarks>
/// Values 1–13 and 400–403 mirror the library's own failure reasons; values 100+ are
/// raised at the interop boundary and have no library-side counterpart. The numbers are
/// part of the native ABI (<c>UnhwpErrorKind</c> in <c>unhwp.h</c>): a new reason takes
/// the next free number and existing ones are never renumbered, so treat an unrecognised
/// value as a generic failure rather than as an error — a newer native library stays
/// usable by older callers, and <see cref="object.ToString"/> preserves the raw number.
/// </remarks>
public enum UnhwpErrorKind
{
    /// <summary>The last call succeeded — no error is recorded.</summary>
    None = 0,

    /// <summary>A failure with no more specific classification.</summary>
    Other = 1,

    /// <summary>An I/O failure, such as a missing or unreadable file.</summary>
    Io = 2,

    /// <summary>The input is not a recognised HWP/HWPX document.</summary>
    UnknownFormat = 3,

    /// <summary>The format is recognised but not supported (e.g., HWP 2.x).</summary>
    UnsupportedFormat = 4,

    /// <summary>The HWPX container (ZIP archive) could not be read — a damaged file.</summary>
    ZipArchive = 5,

    /// <summary>XML content inside an HWPX document could not be parsed.</summary>
    XmlParse = 6,

    /// <summary>The document contains invalid or malformed data.</summary>
    InvalidData = 7,

    /// <summary>A required document part is absent.</summary>
    MissingComponent = 8,

    /// <summary>A text encoding conversion failed.</summary>
    Encoding = 9,

    /// <summary>A referenced style is absent.</summary>
    StyleNotFound = 10,

    /// <summary>A referenced resource is absent.</summary>
    ResourceNotFound = 11,

    /// <summary>The document is encrypted and cannot be processed.</summary>
    Encrypted = 12,

    /// <summary>Producing output failed, such as serialising a rendered result.</summary>
    Render = 13,

    /// <summary>Decompressing a stream inside an HWP 5.0 document failed.</summary>
    Decompression = 400,

    /// <summary>The HWP 5.0 OLE (CFB) container could not be read.</summary>
    OleContainer = 401,

    /// <summary>A binary record in an HWP 5.0 document could not be parsed.</summary>
    RecordParse = 402,

    /// <summary>The document is a distribution copy with restrictions that block parsing.</summary>
    DistributionRestricted = 403,

    /// <summary>An argument was null or not valid UTF-8.</summary>
    InvalidArgument = 100,

    /// <summary>A panic was caught at the interop boundary.</summary>
    Panic = 101,

    /// <summary>The produced output holds an interior NUL byte and cannot cross the ABI.</summary>
    InvalidOutput = 102,
}
