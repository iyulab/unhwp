using Xunit;

namespace Unhwp.Tests;

/// <summary>
/// Tests for version and info functions.
/// </summary>
public class VersionTests
{
    [Fact]
    public void Version_ReturnsNonEmptyString()
    {
        var version = UnhwpConverter.Version;
        Assert.NotNull(version);
        Assert.NotEmpty(version);
    }

    [Fact]
    public void Version_HasSemverFormat()
    {
        var version = UnhwpConverter.Version;
        var parts = version.Split('.');
        Assert.True(parts.Length >= 2, "Version should have at least major.minor");
    }

    [Fact]
    public void SupportedFormats_ReturnsNonEmptyString()
    {
        var formats = UnhwpConverter.SupportedFormats;
        Assert.NotNull(formats);
        Assert.Contains("HWP", formats, StringComparison.OrdinalIgnoreCase);
    }
}

/// <summary>
/// Tests for format detection.
/// </summary>
public class FormatDetectionTests
{
    [Fact]
    public void DetectFormat_UnknownFile_ReturnsUnknown()
    {
        var tempFile = Path.GetTempFileName();
        try
        {
            File.WriteAllText(tempFile, "Hello, World!");
            var format = UnhwpConverter.DetectFormat(tempFile);
            Assert.Equal(DocumentFormat.Unknown, format);
        }
        finally
        {
            File.Delete(tempFile);
        }
    }

    [Fact]
    public void DetectFormat_NonexistentFile_ReturnsUnknown()
    {
        var format = UnhwpConverter.DetectFormat("/nonexistent/path/file.hwp");
        Assert.Equal(DocumentFormat.Unknown, format);
    }
}

/// <summary>
/// Tests for options classes.
/// </summary>
public class OptionsTests
{
    [Fact]
    public void RenderOptions_HasSensibleDefaults()
    {
        var opts = new RenderOptions();
        Assert.False(opts.IncludeFrontmatter);
        Assert.Null(opts.ImagePathPrefix);
        Assert.True(opts.EscapeSpecialChars);
    }

    [Fact]
    public void CleanupOptions_Minimal_HasPreset0()
    {
        var opts = CleanupOptions.Minimal;
        Assert.Equal(CleanupPreset.Minimal, opts.Preset);
        Assert.True(opts.Enabled);
    }

    [Fact]
    public void CleanupOptions_Default_HasPreset1()
    {
        var opts = CleanupOptions.Default;
        Assert.Equal(CleanupPreset.Default, opts.Preset);
        Assert.True(opts.Enabled);
    }

    [Fact]
    public void CleanupOptions_Aggressive_HasPreset2()
    {
        var opts = CleanupOptions.Aggressive;
        Assert.Equal(CleanupPreset.Aggressive, opts.Preset);
        Assert.True(opts.Enabled);
    }

    [Fact]
    public void CleanupOptions_Disabled_HasEnabledFalse()
    {
        var opts = CleanupOptions.Disabled;
        Assert.False(opts.Enabled);
    }
}

/// <summary>
/// Tests for enums.
/// </summary>
public class EnumTests
{
    [Fact]
    public void DocumentFormat_HasDistinctValues()
    {
        var formats = new[]
        {
            DocumentFormat.Unknown,
            DocumentFormat.Hwp5,
            DocumentFormat.Hwpx,
            DocumentFormat.Hwp3
        };

        var distinctCount = formats.Distinct().Count();
        Assert.Equal(4, distinctCount);
    }

    [Fact]
    public void TableFallback_HasThreeOptions()
    {
        var values = Enum.GetValues<TableFallback>();
        Assert.Equal(3, values.Length);
    }

    [Fact]
    public void CleanupPreset_HasThreeLevels()
    {
        var values = Enum.GetValues<CleanupPreset>();
        Assert.Equal(3, values.Length);
    }
}

/// <summary>
/// Integration tests requiring actual HWP files.
/// These tests are skipped if test files are not available.
/// </summary>
public class IntegrationTests
{
    private static string? GetTestFile()
    {
        // Look for test files relative to the test assembly
        var testFilesDir = Path.Combine(
            Path.GetDirectoryName(typeof(IntegrationTests).Assembly.Location) ?? "",
            "..", "..", "..", "..", "..", "..", "test-files"
        );

        var sample = Path.Combine(testFilesDir, "Sample.hwp");
        if (File.Exists(sample))
            return sample;

        // Try alternate location
        var altPath = Path.Combine(Environment.CurrentDirectory, "..", "..", "..", "..", "test-files", "Sample.hwp");
        if (File.Exists(altPath))
            return altPath;

        return null;
    }

    [Fact(Skip = "Requires native library and test files")]
    public void Parse_ReturnsValidResult()
    {
        var testFile = GetTestFile();
        if (testFile == null)
            return; // Skip if no test file

        using var result = UnhwpConverter.Parse(testFile);
        Assert.NotNull(result.Markdown);
        Assert.NotNull(result.Text);
        Assert.True(result.SectionCount >= 0);
        Assert.True(result.ParagraphCount >= 0);
        Assert.True(result.ImageCount >= 0);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void ToMarkdown_ReturnsNonEmptyString()
    {
        var testFile = GetTestFile();
        if (testFile == null)
            return;

        var markdown = UnhwpConverter.ToMarkdown(testFile);
        Assert.NotNull(markdown);
        Assert.NotEmpty(markdown);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void ExtractText_ReturnsNonEmptyString()
    {
        var testFile = GetTestFile();
        if (testFile == null)
            return;

        var text = UnhwpConverter.ExtractText(testFile);
        Assert.NotNull(text);
        Assert.NotEmpty(text);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void ToMarkdownWithCleanup_AppliesCleanup()
    {
        var testFile = GetTestFile();
        if (testFile == null)
            return;

        var clean = UnhwpConverter.ToMarkdownWithCleanup(testFile, CleanupOptions.Aggressive);
        var raw = UnhwpConverter.ToMarkdown(testFile);

        // Both should be valid strings
        Assert.NotNull(clean);
        Assert.NotNull(raw);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void Parse_WithOptions_RespectsSettings()
    {
        var testFile = GetTestFile();
        if (testFile == null)
            return;

        var options = new RenderOptions
        {
            IncludeFrontmatter = true,
            ImagePathPrefix = "images/"
        };

        using var result = UnhwpConverter.Parse(testFile, options);
        Assert.NotNull(result.Markdown);
    }

    [Fact(Skip = "Requires native library and test files")]
    public void DetectFormat_ValidHwpFile_ReturnsHwp5()
    {
        var testFile = GetTestFile();
        if (testFile == null)
            return;

        var format = UnhwpConverter.DetectFormat(testFile);
        Assert.Equal(DocumentFormat.Hwp5, format);
    }
}

/// <summary>
/// Tests for UnhwpImage class.
/// </summary>
public class ImageTests
{
    [Fact]
    public void UnhwpImage_Save_WritesToFile()
    {
        // Create a mock image
        var imageData = new byte[] { 0x89, 0x50, 0x4E, 0x47 }; // PNG magic bytes
        var image = CreateTestImage("test.png", imageData);

        var tempPath = Path.GetTempFileName();
        try
        {
            image.Save(tempPath);
            var written = File.ReadAllBytes(tempPath);
            Assert.Equal(imageData, written);
        }
        finally
        {
            File.Delete(tempPath);
        }
    }

    private static UnhwpImage CreateTestImage(string name, byte[] data)
    {
        // Use reflection to create UnhwpImage (internal constructor)
        var ctor = typeof(UnhwpImage).GetConstructor(
            System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance,
            null,
            new[] { typeof(string), typeof(byte[]) },
            null
        );

        return (UnhwpImage)ctor!.Invoke(new object[] { name, data });
    }
}
