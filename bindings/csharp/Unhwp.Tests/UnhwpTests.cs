using System.Text.Json;
using Xunit;

namespace Unhwp.Tests;

/// <summary>
/// Tests for version function.
/// </summary>
public class VersionTests
{
    [Fact]
    public void Version_ReturnsNonEmptyString()
    {
        var version = UnhwpDocument.Version;
        Assert.NotNull(version);
        Assert.NotEmpty(version);
    }

    [Fact]
    public void Version_HasSemverFormat()
    {
        var version = UnhwpDocument.Version;
        var parts = version.Split('.');
        Assert.True(parts.Length >= 2, "Version should have at least major.minor");
    }
}

/// <summary>
/// <see cref="MarkdownOptions"/> reach the native library: each property is sent, and each
/// one changes the Markdown.
/// </summary>
public class MarkdownOptionsTests
{
    private static string Paragraph(string text) =>
        $"<hp:p><hp:run><hp:t>{text}</hp:t></hp:run></hp:p>";

    /// <summary>
    /// Chapter, section and article markers (headings three levels deep), Markdown syntax in
    /// text with a private-use character, an empty paragraph, an image, and a table whose
    /// first row merges across.
    /// </summary>
    private static readonly string EveryOptionSection =
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
        + "<hs:sec xmlns:hs=\"http://www.hancom.co.kr/hwpml/2011/section\""
        + " xmlns:hp=\"http://www.hancom.co.kr/hwpml/2011/paragraph\""
        + " xmlns:hc=\"http://www.hancom.co.kr/hwpml/2011/core\">"
        + Paragraph("제1장 총칙")
        + Paragraph("제1절 목적")
        + Paragraph("제1조 정의")
        + Paragraph("see [x](y) and a*b*c\uE000")
        + "<hp:p></hp:p>"
        + Paragraph("plain words")
        + "<hp:p><hp:run><hp:pic><hc:img binaryItemIDRef=\"image1\"/></hp:pic></hp:run></hp:p>"
        + "<hp:p><hp:run><hp:tbl>"
        + $"<hp:tr>{Hwpx.Cell("Region and sales", "<hp:cellSpan colSpan=\"2\" rowSpan=\"1\"/>")}</hp:tr>"
        + $"<hp:tr>{Hwpx.Cell("North")}{Hwpx.Cell("10")}</hp:tr>"
        + $"<hp:tr>{Hwpx.Cell("South")}{Hwpx.Cell("12")}</hp:tr>"
        + "</hp:tbl></hp:run></hp:p>"
        + "</hs:sec>";

    private static string Render(MarkdownOptions? options = null)
    {
        using var doc = UnhwpDocument.ParseBytes(Hwpx.Build(EveryOptionSection, title: "Quarterly report"));
        return doc.ToMarkdown(options);
    }

    [Fact]
    public void TheDefaults_AreTheLibraryDefaults()
    {
        Assert.Equal(Render(), Render(new MarkdownOptions()));
    }

    [Fact]
    public void EveryProperty_IsSent()
    {
        var sent = JsonDocument.Parse(new MarkdownOptions().ToJson()).RootElement
            .EnumerateObject().Select(p => p.Name).ToHashSet();
        var declared = typeof(MarkdownOptions).GetProperties()
            .Select(p => p.Name == "Cleanup" ? "cleanup_preset" : SnakeCase(p.Name))
            .ToHashSet();
        Assert.Equal(declared.OrderBy(n => n), sent.OrderBy(n => n));
    }

    private static string SnakeCase(string name) =>
        string.Concat(name.Select((c, i) => char.IsUpper(c) && i > 0 ? "_" + char.ToLowerInvariant(c) : char.ToLowerInvariant(c).ToString()));

    [Fact]
    public void Cleanup_IsSentAsItsPreset()
    {
        string? Preset(CleanupPreset? cleanup) =>
            JsonDocument.Parse(new MarkdownOptions { Cleanup = cleanup }.ToJson())
                .RootElement.GetProperty("cleanup_preset").GetString();

        Assert.Null(Preset(null));
        Assert.Equal("minimal", Preset(CleanupPreset.Minimal));
        Assert.Equal("standard", Preset(CleanupPreset.Standard));
        Assert.Equal("aggressive", Preset(CleanupPreset.Aggressive));
    }

    /// <summary>The two settings the document cannot show are checked as sent.</summary>
    [Fact]
    public void LineBreaksAndListMarker_AreSent()
    {
        var json = JsonDocument.Parse(
            new MarkdownOptions { PreserveLineBreaks = false, ListMarker = '*' }.ToJson()).RootElement;
        Assert.False(json.GetProperty("preserve_line_breaks").GetBoolean());
        Assert.Equal("*", json.GetProperty("list_marker").GetString());
    }

    public static IEnumerable<object?[]> OptionsThatShow() => new[]
    {
        new object?[] { new MarkdownOptions { ImagePathPrefix = "img/" }, "](img/image1)", "](assets/image1)" },
        new object?[] { new MarkdownOptions { TableFallback = TableFallback.Html }, "<table", "| North | 10 |" },
        new object?[] { new MarkdownOptions { TableFallback = TableFallback.Skip }, "plain words", "Region and sales" },
        new object?[] { new MarkdownOptions { MaxHeadingLevel = 2 }, "## ", "### " },
        new object?[] { new MarkdownOptions { IncludeFrontmatter = true }, "title: \"Quarterly report\"", null },
        new object?[] { new MarkdownOptions { IncludeEmptyParagraphs = true }, "\n\n\n\nplain words", null },
        new object?[] { new MarkdownOptions { ParagraphSpacing = false }, "plain words\n![image]", null },
        new object?[] { new MarkdownOptions { EscapeSpecialChars = false }, "see [x](y) and a*b*c", "\\[" },
        new object?[] { new MarkdownOptions { SectionMarkers = SectionMarkerStyle.Comment }, "<!-- section 1 -->", null },
        new object?[] { new MarkdownOptions { Cleanup = CleanupPreset.Minimal }, "plain words", "\uE000" },
        new object?[] { new MarkdownOptions { Cleanup = CleanupPreset.Standard }, "|North|10|", "\uE000" },
        new object?[] { new MarkdownOptions { Cleanup = CleanupPreset.Aggressive }, "|North|10|", "\uE000" },
        new object?[] { new MarkdownOptions { ImagePathPrefix = "img\\sub\\", Refine = true }, "](img/sub/image1)", "](assets/image1)" },
    };

    [Theory]
    [MemberData(nameof(OptionsThatShow))]
    public void Option_ChangesTheOutput(MarkdownOptions options, string present, string? absent)
    {
        var defaults = Render();
        var markdown = Render(options);
        Assert.NotEqual(defaults, markdown);
        Assert.Contains(present, markdown);
        if (absent is not null)
        {
            Assert.Contains(absent, defaults);
            Assert.DoesNotContain(absent, markdown);
        }
    }

    [Fact]
    public void Refine_ChangesTheOutput()
    {
        var unrefined = Render(new MarkdownOptions { ImagePathPrefix = "img\\sub\\" });
        var refined = Render(new MarkdownOptions { ImagePathPrefix = "img\\sub\\", Refine = true });
        Assert.DoesNotContain("](img/sub/image1)", unrefined);
        Assert.Contains("](img/sub/image1)", refined);
    }

    [Theory]
    [InlineData(0)]
    [InlineData(7)]
    public void MaxHeadingLevel_OutsideItsRange_IsAnInvalidArgument(int level)
    {
        var error = Assert.Throws<UnhwpException>(() => Render(new MarkdownOptions { MaxHeadingLevel = level }));
        Assert.Equal(UnhwpErrorKind.InvalidArgument, error.Kind);
        Assert.Contains("max_heading_level", error.Message);
    }
}

/// <summary>Builds HWPX documents from the committed sample.</summary>
internal static class Hwpx
{
    public static string Cell(string text, string span = "") =>
        $"<hp:tc><hp:subList><hp:p><hp:run><hp:t>{text}</hp:t></hp:run></hp:p></hp:subList>{span}</hp:tc>";

    /// <summary>
    /// The committed sample with its first section replaced by <paramref name="section0"/>, and
    /// a title in its package metadata when <paramref name="title"/> is given.
    /// </summary>
    public static byte[] Build(string section0, string? title = null)
    {
        using var source = System.IO.Compression.ZipFile.OpenRead(IntegrationTests.GetTestFile());
        using var output = new MemoryStream();
        using (var target = new System.IO.Compression.ZipArchive(output, System.IO.Compression.ZipArchiveMode.Create, leaveOpen: true))
        {
            foreach (var entry in source.Entries)
            {
                byte[] body;
                if (entry.FullName == "Contents/section0.xml")
                {
                    body = System.Text.Encoding.UTF8.GetBytes(section0);
                }
                else
                {
                    using var read = entry.Open();
                    using var buffer = new MemoryStream();
                    read.CopyTo(buffer);
                    body = buffer.ToArray();
                    if (entry.FullName == "Contents/content.hpf" && title is not null)
                    {
                        var package = System.Text.Encoding.UTF8.GetString(body).Replace(
                            "<opf:manifest>",
                            $"<opf:metadata><opf:title>{title}</opf:title></opf:metadata><opf:manifest>");
                        body = System.Text.Encoding.UTF8.GetBytes(package);
                    }
                }
                var copy = target.CreateEntry(entry.FullName, System.IO.Compression.CompressionLevel.NoCompression);
                using var write = copy.Open();
                write.Write(body, 0, body.Length);
            }
        }
        return output.ToArray();
    }
}

/// <summary>
/// Tests for UnhwpException.
/// </summary>
public class ExceptionTests
{
    [Fact]
    public void UnhwpException_StoresMessage()
    {
        var ex = new UnhwpException("test error");
        Assert.Equal("test error", ex.Message);
    }

    /// <summary>
    /// A message-only exception did not come from the native library, so it carries no
    /// classification — but it must not read as success either.
    /// </summary>
    [Fact]
    public void MessageOnlyException_IsOther_NotNone()
    {
        var ex = new UnhwpException("wrapper-side failure");

        Assert.Equal(UnhwpErrorKind.Other, ex.Kind);
        Assert.NotEqual(UnhwpErrorKind.None, ex.Kind);
    }

    [Fact]
    public void InnerExceptionConstructor_IsOther()
    {
        var ex = new UnhwpException("wrapped", new InvalidOperationException("inner"));

        Assert.Equal(UnhwpErrorKind.Other, ex.Kind);
    }

    [Fact]
    public void KindConstructor_StoresKind()
    {
        var ex = new UnhwpException("archive damaged", UnhwpErrorKind.OleContainer);

        Assert.Equal(UnhwpErrorKind.OleContainer, ex.Kind);
    }

    /// <summary>
    /// Forward compatibility: a newer native library may report a reason this build has
    /// no name for. The number has to survive rather than throw or collapse.
    /// </summary>
    [Fact]
    public void UnknownKindValue_PassesThroughAndKeepsItsNumber()
    {
        var ex = new UnhwpException("from the future", (UnhwpErrorKind)9999);

        Assert.Equal(9999, (int)ex.Kind);
        Assert.Equal("9999", ex.Kind.ToString());
    }
}

/// <summary>
/// The C# numbering is only useful if it agrees with the native ABI, so pin it here
/// too — these values are what cross the boundary.
/// </summary>
public class ErrorKindTests
{
    [Fact]
    public void Discriminants_MatchTheNativeAbi()
    {
        Assert.Equal(0, (int)UnhwpErrorKind.None);
        Assert.Equal(1, (int)UnhwpErrorKind.Other);
        Assert.Equal(2, (int)UnhwpErrorKind.Io);
        Assert.Equal(3, (int)UnhwpErrorKind.UnknownFormat);
        Assert.Equal(4, (int)UnhwpErrorKind.UnsupportedFormat);
        Assert.Equal(5, (int)UnhwpErrorKind.ZipArchive);
        Assert.Equal(6, (int)UnhwpErrorKind.XmlParse);
        Assert.Equal(7, (int)UnhwpErrorKind.InvalidData);
        Assert.Equal(8, (int)UnhwpErrorKind.MissingComponent);
        Assert.Equal(9, (int)UnhwpErrorKind.Encoding);
        Assert.Equal(10, (int)UnhwpErrorKind.StyleNotFound);
        Assert.Equal(11, (int)UnhwpErrorKind.ResourceNotFound);
        Assert.Equal(12, (int)UnhwpErrorKind.Encrypted);
        Assert.Equal(400, (int)UnhwpErrorKind.Decompression);
        Assert.Equal(401, (int)UnhwpErrorKind.OleContainer);
        Assert.Equal(402, (int)UnhwpErrorKind.RecordParse);
        Assert.Equal(403, (int)UnhwpErrorKind.DistributionRestricted);
        Assert.Equal(100, (int)UnhwpErrorKind.InvalidArgument);
        Assert.Equal(101, (int)UnhwpErrorKind.Panic);
        Assert.Equal(102, (int)UnhwpErrorKind.InvalidOutput);
    }
}

/// <summary>
/// End-to-end tests over the native library, run against the repository's own sample
/// document. They mirror what the Rust suite asserts about that same file, so a
/// regression that only shows up once content has crossed the interop boundary — a
/// section dropped, text lost in marshalling — fails here rather than in a consumer.
/// </summary>
public class IntegrationTests
{
    /// <summary>
    /// The repository's own committed sample document. It ships with the source, so a
    /// missing file is a broken checkout rather than an expected condition — these tests
    /// fail loudly instead of quietly passing over an absent fixture.
    /// </summary>
    internal static string GetTestFile()
    {
        // bin/Debug/<tfm> -> Unhwp.Tests -> csharp -> bindings -> repository root.
        var repoRoot = Path.Combine(
            Path.GetDirectoryName(typeof(IntegrationTests).Assembly.Location) ?? "",
            "..", "..", "..", "..", "..", ".."
        );

        var sample = Path.GetFullPath(
            Path.Combine(repoRoot, "tests", "fixtures", "two_sections.hwpx")
        );

        Assert.True(File.Exists(sample), $"Sample document not found at {sample}");
        return sample;
    }

    [Fact]
    public void ParseFile_ReturnsValidDocument()
    {
        var testFile = GetTestFile();

        using var doc = UnhwpDocument.ParseFile(testFile);

        // The sample has two sections in its spine. A count of 1 is the signature of a
        // section-order parser that stopped after section0.
        Assert.Equal(2, doc.SectionCount);
    }

    [Fact]
    public void ToMarkdown_ReturnsNonEmptyString()
    {
        var testFile = GetTestFile();

        using var doc = UnhwpDocument.ParseFile(testFile);
        var markdown = doc.ToMarkdown();

        // Both sections must survive the round trip, not just the first one.
        Assert.Contains("Section zero content", markdown);
        Assert.Contains("Section one content", markdown);
    }

    [Fact]
    public void ToText_ReturnsNonEmptyString()
    {
        var testFile = GetTestFile();

        using var doc = UnhwpDocument.ParseFile(testFile);
        var text = doc.ToText();

        Assert.Contains("Section zero content", text);
        Assert.Contains("Section one content", text);
    }

    [Fact]
    public void ToJson_ReturnsValidJson()
    {
        var testFile = GetTestFile();

        using var doc = UnhwpDocument.ParseFile(testFile);
        var json = doc.ToJson();
        Assert.NotNull(json);
        Assert.StartsWith("{", json);
    }

    [Fact]
    public void GetResourceIds_TextOnlyDocument_ReturnsEmptyList()
    {
        var testFile = GetTestFile();

        using var doc = UnhwpDocument.ParseFile(testFile);

        // The native side answers with a JSON array even when it is empty, so this reaches
        // the binding's deserializer — which must not depend on reflection-based JSON.
        Assert.Empty(doc.GetResourceIds());
    }

    [Fact]
    public void ParseFile_NonexistentFile_ThrowsFileNotFoundException()
    {
        Assert.Throws<FileNotFoundException>(() =>
        {
            UnhwpDocument.ParseFile("/nonexistent/path/file.hwp");
        });
    }
}

/// <summary>
/// Tables as delimited text: <see cref="UnhwpDocument.GetTables"/>.
/// </summary>
public class TableTextTests
{
    private static string Cell(string text, string span = "") => Hwpx.Cell(text, span);

    /// <summary>
    /// A section holding one table: «Region» merged down two rows, «Sales, total» across
    /// two columns over 2024 | 2025, then North | 10 | 12.
    /// </summary>
    private static readonly string TableSection =
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
        + "<hs:sec xmlns:hs=\"http://www.hancom.co.kr/hwpml/2011/section\""
        + " xmlns:hp=\"http://www.hancom.co.kr/hwpml/2011/paragraph\">"
        + "<hp:tbl>"
        + $"<hp:tr>{Cell("Region", "<hp:cellSpan colSpan=\"1\" rowSpan=\"2\"/>")}"
        + $"{Cell("Sales, total", "<hp:cellSpan colSpan=\"2\" rowSpan=\"1\"/>")}</hp:tr>"
        + $"<hp:tr>{Cell("2024")}{Cell("2025")}</hp:tr>"
        + $"<hp:tr>{Cell("North")}{Cell("10")}{Cell("12")}</hp:tr>"
        + "</hp:tbl>"
        + "</hs:sec>";

    /// <summary>The committed sample with its first section replaced by <see cref="TableSection"/>.</summary>
    private static byte[] HwpxWithATable() => Hwpx.Build(TableSection);

    [Fact]
    public void GetTables_ComeAsCsvWithTheirPlace()
    {
        using var doc = UnhwpDocument.ParseBytes(HwpxWithATable());
        var tables = doc.GetTables();
        Assert.Single(tables);
        Assert.Equal(1, tables[0].Section);
        Assert.Equal(1, tables[0].Index);
        Assert.Equal("Region,\"Sales, total\",\r\n,2024,2025\r\nNorth,10,12\r\n", tables[0].Text);
    }

    [Fact]
    public void GetTables_Tsv_SeparatesFieldsWithTabs()
    {
        using var doc = UnhwpDocument.ParseBytes(HwpxWithATable());
        Assert.Equal(
            "Region\tSales, total\t\r\n\t2024\t2025\r\nNorth\t10\t12\r\n",
            doc.GetTables(tsv: true)[0].Text);
    }

    [Fact]
    public void GetTables_NoTables_IsEmpty()
    {
        using var doc = UnhwpDocument.ParseFile(IntegrationTests.GetTestFile());
        Assert.Empty(doc.GetTables());
    }
}
