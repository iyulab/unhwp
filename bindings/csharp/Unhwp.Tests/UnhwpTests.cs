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
    }

    [Fact]
    public void MarkdownOptions_ToFlags_Empty()
    {
        var opts = new MarkdownOptions();
        Assert.Equal(0, opts.ToFlags());
    }

    [Fact]
    public void MarkdownOptions_ToFlags_AllSet()
    {
        var opts = new MarkdownOptions
        {
            IncludeFrontmatter = true,
            EscapeSpecialChars = true,
            ParagraphSpacing = true,
        };
        var flags = opts.ToFlags();
        Assert.Equal(NativeMethods.UNHWP_FLAG_FRONTMATTER
            | NativeMethods.UNHWP_FLAG_ESCAPE_SPECIAL
            | NativeMethods.UNHWP_FLAG_PARAGRAPH_SPACING, flags);
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
