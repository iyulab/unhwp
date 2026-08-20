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
/// Integration tests requiring actual HWP files and native library.
/// </summary>
public class IntegrationTests
{
    private static string? GetTestFile()
    {
        var testFilesDir = Path.Combine(
            Path.GetDirectoryName(typeof(IntegrationTests).Assembly.Location) ?? "",
            "..", "..", "..", "..", "..", "..", "test-files"
        );

        var sample = Path.Combine(testFilesDir, "Sample.hwp");
        if (File.Exists(sample))
            return sample;

        var altPath = Path.Combine(Environment.CurrentDirectory, "..", "..", "..", "..", "test-files", "Sample.hwp");
        if (File.Exists(altPath))
            return altPath;

        return null;
    }

    [Fact(Skip = "Requires native library and test files")]
    public void ParseFile_ReturnsValidDocument()
    {
        var testFile = GetTestFile();
        if (testFile == null) return;

        using var doc = UnhwpDocument.ParseFile(testFile);
        Assert.True(doc.SectionCount >= 0);
        Assert.True(doc.ResourceCount >= 0);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void ToMarkdown_ReturnsNonEmptyString()
    {
        var testFile = GetTestFile();
        if (testFile == null) return;

        using var doc = UnhwpDocument.ParseFile(testFile);
        var markdown = doc.ToMarkdown();
        Assert.NotNull(markdown);
        Assert.NotEmpty(markdown);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void ToText_ReturnsNonEmptyString()
    {
        var testFile = GetTestFile();
        if (testFile == null) return;

        using var doc = UnhwpDocument.ParseFile(testFile);
        var text = doc.ToText();
        Assert.NotNull(text);
        Assert.NotEmpty(text);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void ToJson_ReturnsValidJson()
    {
        var testFile = GetTestFile();
        if (testFile == null) return;

        using var doc = UnhwpDocument.ParseFile(testFile);
        var json = doc.ToJson();
        Assert.NotNull(json);
        Assert.StartsWith("{", json);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void ParseFile_NonexistentFile_ThrowsFileNotFoundException()
    {
        Assert.Throws<FileNotFoundException>(() =>
        {
            UnhwpDocument.ParseFile("/nonexistent/path/file.hwp");
        });
    }
}
