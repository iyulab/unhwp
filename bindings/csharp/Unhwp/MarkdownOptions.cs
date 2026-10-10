using System.Text.Json;
using System.Text.Json.Serialization;

namespace Unhwp;

/// <summary>
/// What becomes of a table with merged cells, which Markdown cannot express.
/// </summary>
public enum TableFallback
{
    /// <summary>A Markdown table with the merges dropped (the default).</summary>
    SimplifiedMarkdown,

    /// <summary>An HTML table that keeps rowspan and colspan.</summary>
    Html,

    /// <summary>The table is left out.</summary>
    Skip,
}

/// <summary>
/// Markers written at section boundaries.
/// </summary>
public enum SectionMarkerStyle
{
    /// <summary>No markers (the default).</summary>
    None,

    /// <summary>An <c>&lt;!-- section N --&gt;</c> comment before each section.</summary>
    Comment,
}

/// <summary>
/// The configurations of the cleanup pipeline run over the rendered Markdown: string
/// normalization, line cleaning (page numbers, repeated headers and footers), structural
/// filtering and whitespace normalization.
/// </summary>
public enum CleanupPreset
{
    /// <summary>Normalization only.</summary>
    Minimal,

    /// <summary>Every stage.</summary>
    Standard,

    /// <summary>Every stage, removing headers and footers more eagerly.</summary>
    Aggressive,
}

/// <summary>
/// Options for markdown rendering. Every property reaches the native library, and the
/// defaults are the library's own.
/// </summary>
public class MarkdownOptions
{
    /// <summary>
    /// Prefix of the image paths written in the Markdown. Default: <c>"assets/"</c>.
    /// </summary>
    public string ImagePathPrefix { get; set; } = "assets/";

    /// <summary>
    /// What becomes of a table with merged cells. Default:
    /// <see cref="Unhwp.TableFallback.SimplifiedMarkdown"/>.
    /// </summary>
    public TableFallback TableFallback { get; set; } = TableFallback.SimplifiedMarkdown;

    /// <summary>
    /// The deepest heading level written (1-6); deeper headings take this level. Default: 4.
    /// A value outside 1-6 makes <see cref="UnhwpDocument.ToMarkdown"/> throw
    /// <see cref="UnhwpException"/> with <see cref="UnhwpErrorKind.InvalidArgument"/>.
    /// </summary>
    public int MaxHeadingLevel { get; set; } = 4;

    /// <summary>
    /// Include YAML frontmatter with document metadata. Default: <see langword="false"/>.
    /// </summary>
    public bool IncludeFrontmatter { get; set; } = false;

    /// <summary>
    /// Keep line breaks inside paragraphs as Markdown hard breaks; <see langword="false"/>
    /// joins the lines with a space. Default: <see langword="true"/>.
    /// </summary>
    public bool PreserveLineBreaks { get; set; } = true;

    /// <summary>
    /// Keep empty paragraphs as blank lines. Default: <see langword="false"/>.
    /// </summary>
    public bool IncludeEmptyParagraphs { get; set; } = false;

    /// <summary>
    /// The character that marks an unordered list item. Default: <c>'-'</c>.
    /// </summary>
    public char ListMarker { get; set; } = '-';

    /// <summary>
    /// A blank line after each paragraph. Default: <see langword="true"/>.
    /// </summary>
    public bool ParagraphSpacing { get; set; } = true;

    /// <summary>
    /// Escape special markdown characters, so text that reads as Markdown syntax stays text.
    /// Default: <see langword="true"/>, as in the Rust API.
    /// </summary>
    public bool EscapeSpecialChars { get; set; } = true;

    /// <summary>
    /// Markers at section boundaries. Default: <see cref="SectionMarkerStyle.None"/>.
    /// </summary>
    public SectionMarkerStyle SectionMarkers { get; set; } = SectionMarkerStyle.None;

    /// <summary>
    /// The cleanup pipeline to run over the output; <see langword="null"/> (the default)
    /// for none.
    /// </summary>
    public CleanupPreset? Cleanup { get; set; } = null;

    /// <summary>
    /// Apply the lossless, idempotent markdown shape-refinement pass (table
    /// shape, ordered-list numbering, link/image paths, frontmatter, section
    /// anchors) after rendering. Default: <see langword="false"/>.
    /// </summary>
    public bool Refine { get; set; } = false;

    /// <summary>
    /// The options as <c>unhwp_to_markdown_with_options</c> reads them — every property.
    /// </summary>
    internal string ToJson() =>
        JsonSerializer.Serialize(
            new RenderOptionsJson(
                ImagePathPrefix,
                TableFallback switch
                {
                    TableFallback.SimplifiedMarkdown => "simplified_markdown",
                    TableFallback.Html => "html",
                    TableFallback.Skip => "skip",
                    _ => throw new ArgumentOutOfRangeException(nameof(TableFallback)),
                },
                MaxHeadingLevel,
                IncludeFrontmatter,
                PreserveLineBreaks,
                IncludeEmptyParagraphs,
                ListMarker.ToString(),
                ParagraphSpacing,
                EscapeSpecialChars,
                SectionMarkers switch
                {
                    SectionMarkerStyle.None => "none",
                    SectionMarkerStyle.Comment => "comment",
                    _ => throw new ArgumentOutOfRangeException(nameof(SectionMarkers)),
                },
                Cleanup switch
                {
                    null => null,
                    CleanupPreset.Minimal => "minimal",
                    CleanupPreset.Standard => "standard",
                    CleanupPreset.Aggressive => "aggressive",
                    _ => throw new ArgumentOutOfRangeException(nameof(Cleanup)),
                },
                Refine),
            UnhwpJsonContext.Default.RenderOptionsJson);
}

/// <summary>
/// The JSON object <c>unhwp_to_markdown_with_options</c> reads.
/// </summary>
internal sealed record RenderOptionsJson(
    [property: JsonPropertyName("image_path_prefix")] string ImagePathPrefix,
    [property: JsonPropertyName("table_fallback")] string TableFallback,
    [property: JsonPropertyName("max_heading_level")] int MaxHeadingLevel,
    [property: JsonPropertyName("include_frontmatter")] bool IncludeFrontmatter,
    [property: JsonPropertyName("preserve_line_breaks")] bool PreserveLineBreaks,
    [property: JsonPropertyName("include_empty_paragraphs")] bool IncludeEmptyParagraphs,
    [property: JsonPropertyName("list_marker")] string ListMarker,
    [property: JsonPropertyName("paragraph_spacing")] bool ParagraphSpacing,
    [property: JsonPropertyName("escape_special_chars")] bool EscapeSpecialChars,
    [property: JsonPropertyName("section_markers")] string SectionMarkers,
    [property: JsonPropertyName("cleanup_preset")] string? CleanupPreset,
    [property: JsonPropertyName("refine")] bool Refine);
