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
/// Tests for MarkdownOptions.
/// </summary>
public class MarkdownOptionsTests
{
    [Fact]
    public void MarkdownOptions_HasSensibleDefaults()
    {
        var opts = new MarkdownOptions();
        Assert.False(opts.IncludeFrontmatter);
        Assert.False(opts.EscapeSpecialChars);
        Assert.False(opts.ParagraphSpacing);
        Assert.False(opts.Refine);
    }

    [Fact]
    public void MarkdownOptions_CanSetAllProperties()
    {
        var opts = new MarkdownOptions
        {
            IncludeFrontmatter = true,
            EscapeSpecialChars = true,
            ParagraphSpacing = true,
            Refine = true,
        };
        Assert.True(opts.IncludeFrontmatter);
        Assert.True(opts.EscapeSpecialChars);
        Assert.True(opts.ParagraphSpacing);
        Assert.True(opts.Refine);
    }

    [Fact]
    public void MarkdownOptions_CanSetIndividualProperties()
    {
        var opts = new MarkdownOptions { IncludeFrontmatter = true };
        Assert.True(opts.IncludeFrontmatter);
        Assert.False(opts.EscapeSpecialChars);
        Assert.False(opts.ParagraphSpacing);
        Assert.False(opts.Refine);
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
    private static string GetTestFile()
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
    public void ParseFile_NonexistentFile_ThrowsFileNotFoundException()
    {
        Assert.Throws<FileNotFoundException>(() =>
        {
            UnhwpDocument.ParseFile("/nonexistent/path/file.hwp");
        });
    }
}
