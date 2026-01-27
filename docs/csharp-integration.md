# C# Integration Guide

This guide explains how to integrate the `unhwp` library with C# and .NET applications using P/Invoke.

## Overview

The `unhwp` library provides C-compatible FFI bindings that allow seamless integration with C#, .NET Framework, and .NET Core/5+/6+ applications.

### Supported Platforms

| Platform | Library File |
|----------|-------------|
| Windows x64 | `unhwp.dll` |
| Linux x64 | `libunhwp.so` |
| macOS (Intel) | `libunhwp.dylib` |
| macOS (Apple Silicon) | `libunhwp.dylib` |

## Quick Start

### 1. Build the Native Library

```bash
# Windows
cargo build --release
# Output: target/release/unhwp.dll

# Linux
cargo build --release
# Output: target/release/libunhwp.so

# macOS
cargo build --release
# Output: target/release/libunhwp.dylib
```

### 2. Add the P/Invoke Wrapper

Create a C# class to wrap the native functions:

```csharp
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace Unhwp
{
    /// <summary>
    /// P/Invoke wrapper for the unhwp native library.
    /// Provides HWP/HWPX document parsing and conversion to Markdown.
    /// </summary>
    public static class UnhwpNative
    {
        private const string LibraryName = "unhwp";

        #region Result Codes

        public const int UNHWP_OK = 0;
        public const int UNHWP_ERR_FILE_NOT_FOUND = -1;
        public const int UNHWP_ERR_PARSE = -2;
        public const int UNHWP_ERR_RENDER = -3;
        public const int UNHWP_ERR_INVALID_ARG = -4;
        public const int UNHWP_ERR_UNSUPPORTED = -5;
        public const int UNHWP_ERR_UNKNOWN = -99;

        #endregion

        #region Format Constants

        public const int FORMAT_UNKNOWN = 0;
        public const int FORMAT_HWP5 = 1;
        public const int FORMAT_HWPX = 2;
        public const int FORMAT_HWP3 = 3;

        #endregion

        #region Structures

        [StructLayout(LayoutKind.Sequential)]
        public struct CleanupOptions
        {
            public int Enabled;
            public int Preset;         // 0=default, 1=minimal, 2=aggressive
            public int DetectMojibake;
            public int PreserveFrontmatter;

            public static CleanupOptions Default => new CleanupOptions
            {
                Enabled = 0,
                Preset = 0,
                DetectMojibake = 1,
                PreserveFrontmatter = 1
            };

            public static CleanupOptions Enabled => new CleanupOptions
            {
                Enabled = 1,
                Preset = 0,
                DetectMojibake = 1,
                PreserveFrontmatter = 1
            };

            public static CleanupOptions Aggressive => new CleanupOptions
            {
                Enabled = 1,
                Preset = 2,
                DetectMojibake = 1,
                PreserveFrontmatter = 1
            };
        }

        [StructLayout(LayoutKind.Sequential)]
        public struct RenderOptions
        {
            public int IncludeFrontmatter;
            public IntPtr ImagePathPrefix;  // UTF-8 null-terminated string
            public int TableFallback;       // 0=markdown, 1=html, 2=skip
            public int PreserveLineBreaks;
            public int EscapeSpecialChars;

            public static RenderOptions Default => new RenderOptions
            {
                IncludeFrontmatter = 1,
                ImagePathPrefix = IntPtr.Zero,
                TableFallback = 0,
                PreserveLineBreaks = 0,
                EscapeSpecialChars = 1
            };
        }

        #endregion

        #region Native Functions

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_detect_format(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_to_markdown(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
            out IntPtr outMarkdown,
            out IntPtr outError);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_to_markdown_with_cleanup(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
            out IntPtr outMarkdown,
            out IntPtr outError);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_to_markdown_ex(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
            ref RenderOptions renderOptions,
            ref CleanupOptions cleanupOptions,
            out IntPtr outMarkdown,
            out IntPtr outError);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_extract_text(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
            out IntPtr outText,
            out IntPtr outError);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_bytes_to_markdown(
            byte[] data,
            UIntPtr dataLen,
            out IntPtr outMarkdown,
            out IntPtr outError);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_bytes_to_markdown_ex(
            byte[] data,
            UIntPtr dataLen,
            ref RenderOptions renderOptions,
            ref CleanupOptions cleanupOptions,
            out IntPtr outMarkdown,
            out IntPtr outError);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void unhwp_free_string(IntPtr ptr);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_version();

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_supported_formats();

        #endregion

        #region Structured Result API

        [StructLayout(LayoutKind.Sequential)]
        public struct ImageData
        {
            public IntPtr Name;      // Null-terminated UTF-8 string
            public IntPtr Data;      // Binary data pointer
            public UIntPtr DataLen;  // Length in bytes
        }

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_parse(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
            ref RenderOptions renderOptions,
            ref CleanupOptions cleanupOptions);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_parse(
            [MarshalAs(UnmanagedType.LPUTF8Str)] string path,
            IntPtr renderOptions,  // NULL for defaults
            IntPtr cleanupOptions); // NULL for defaults

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_parse_bytes(
            byte[] data,
            UIntPtr dataLen,
            IntPtr renderOptions,
            IntPtr cleanupOptions);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_result_get_markdown(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_result_get_text(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_result_get_image_count(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_result_get_image(
            IntPtr result,
            int index,
            out ImageData outImage);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_result_get_section_count(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_result_get_paragraph_count(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_result_get_raw_content(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_result_get_error(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void unhwp_result_free(IntPtr result);

        #endregion

        #region Helper Methods

        /// <summary>
        /// Converts an IntPtr to a UTF-8 string and frees the native memory.
        /// </summary>
        public static string? PtrToStringAndFree(IntPtr ptr)
        {
            if (ptr == IntPtr.Zero)
                return null;

            try
            {
                return Marshal.PtrToStringUTF8(ptr);
            }
            finally
            {
                unhwp_free_string(ptr);
            }
        }

        /// <summary>
        /// Gets the library version.
        /// </summary>
        public static string GetVersion()
        {
            var ptr = unhwp_version();
            return Marshal.PtrToStringUTF8(ptr) ?? "unknown";
        }

        #endregion
    }
}
```

### 3. Create a High-Level Wrapper

```csharp
using System;
using System.IO;

namespace Unhwp
{
    /// <summary>
    /// High-level wrapper for HWP/HWPX document processing.
    /// </summary>
    public class HwpConverter : IDisposable
    {
        private bool _disposed;

        /// <summary>
        /// Gets the library version.
        /// </summary>
        public static string Version => UnhwpNative.GetVersion();

        /// <summary>
        /// Detects the format of an HWP/HWPX file.
        /// </summary>
        public static HwpFormat DetectFormat(string path)
        {
            var result = UnhwpNative.unhwp_detect_format(path);
            return result switch
            {
                UnhwpNative.FORMAT_HWP5 => HwpFormat.Hwp5,
                UnhwpNative.FORMAT_HWPX => HwpFormat.Hwpx,
                UnhwpNative.FORMAT_HWP3 => HwpFormat.Hwp3,
                _ => HwpFormat.Unknown
            };
        }

        /// <summary>
        /// Converts an HWP/HWPX file to Markdown.
        /// </summary>
        public static string ToMarkdown(string path, bool enableCleanup = false)
        {
            int result;
            IntPtr markdownPtr, errorPtr;

            if (enableCleanup)
            {
                result = UnhwpNative.unhwp_to_markdown_with_cleanup(
                    path, out markdownPtr, out errorPtr);
            }
            else
            {
                result = UnhwpNative.unhwp_to_markdown(
                    path, out markdownPtr, out errorPtr);
            }

            if (result != UnhwpNative.UNHWP_OK)
            {
                var error = UnhwpNative.PtrToStringAndFree(errorPtr);
                throw new HwpException(result, error ?? "Unknown error");
            }

            return UnhwpNative.PtrToStringAndFree(markdownPtr)
                ?? throw new HwpException(result, "Empty result");
        }

        /// <summary>
        /// Converts an HWP/HWPX file to Markdown with custom options.
        /// </summary>
        public static string ToMarkdown(string path, ConversionOptions options)
        {
            var renderOpts = new UnhwpNative.RenderOptions
            {
                IncludeFrontmatter = options.IncludeFrontmatter ? 1 : 0,
                ImagePathPrefix = IntPtr.Zero,
                TableFallback = (int)options.TableFallback,
                PreserveLineBreaks = options.PreserveLineBreaks ? 1 : 0,
                EscapeSpecialChars = options.EscapeSpecialChars ? 1 : 0
            };

            var cleanupOpts = new UnhwpNative.CleanupOptions
            {
                Enabled = options.EnableCleanup ? 1 : 0,
                Preset = (int)options.CleanupPreset,
                DetectMojibake = options.DetectMojibake ? 1 : 0,
                PreserveFrontmatter = options.PreserveFrontmatter ? 1 : 0
            };

            var result = UnhwpNative.unhwp_to_markdown_ex(
                path, ref renderOpts, ref cleanupOpts,
                out var markdownPtr, out var errorPtr);

            if (result != UnhwpNative.UNHWP_OK)
            {
                var error = UnhwpNative.PtrToStringAndFree(errorPtr);
                throw new HwpException(result, error ?? "Unknown error");
            }

            return UnhwpNative.PtrToStringAndFree(markdownPtr)
                ?? throw new HwpException(result, "Empty result");
        }

        /// <summary>
        /// Extracts plain text from an HWP/HWPX file.
        /// </summary>
        public static string ExtractText(string path)
        {
            var result = UnhwpNative.unhwp_extract_text(
                path, out var textPtr, out var errorPtr);

            if (result != UnhwpNative.UNHWP_OK)
            {
                var error = UnhwpNative.PtrToStringAndFree(errorPtr);
                throw new HwpException(result, error ?? "Unknown error");
            }

            return UnhwpNative.PtrToStringAndFree(textPtr)
                ?? throw new HwpException(result, "Empty result");
        }

        /// <summary>
        /// Converts HWP/HWPX bytes to Markdown.
        /// </summary>
        public static string BytesToMarkdown(byte[] data, bool enableCleanup = false)
        {
            var renderOpts = UnhwpNative.RenderOptions.Default;
            var cleanupOpts = enableCleanup
                ? UnhwpNative.CleanupOptions.Enabled
                : UnhwpNative.CleanupOptions.Default;

            var result = UnhwpNative.unhwp_bytes_to_markdown_ex(
                data, (UIntPtr)data.Length,
                ref renderOpts, ref cleanupOpts,
                out var markdownPtr, out var errorPtr);

            if (result != UnhwpNative.UNHWP_OK)
            {
                var error = UnhwpNative.PtrToStringAndFree(errorPtr);
                throw new HwpException(result, error ?? "Unknown error");
            }

            return UnhwpNative.PtrToStringAndFree(markdownPtr)
                ?? throw new HwpException(result, "Empty result");
        }

        /// <summary>
        /// Converts a Stream to Markdown.
        /// </summary>
        public static string StreamToMarkdown(Stream stream, bool enableCleanup = false)
        {
            using var ms = new MemoryStream();
            stream.CopyTo(ms);
            return BytesToMarkdown(ms.ToArray(), enableCleanup);
        }

        public void Dispose()
        {
            if (!_disposed)
            {
                _disposed = true;
                GC.SuppressFinalize(this);
            }
        }
    }

    /// <summary>
    /// Represents a parsed HWP/HWPX document with lazy-loaded properties.
    /// Use this class when you need access to multiple outputs (text, markdown, images).
    /// </summary>
    public sealed class HwpDocument : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;
        private string? _cachedMarkdown;
        private string? _cachedText;
        private string? _cachedRawContent;
        private HwpImage[]? _cachedImages;

        private HwpDocument(IntPtr handle)
        {
            _handle = handle;
        }

        /// <summary>
        /// Parses an HWP/HWPX file and returns a document object.
        /// </summary>
        public static HwpDocument Parse(string path, ConversionOptions? options = null)
        {
            IntPtr handle;

            if (options != null)
            {
                var renderOpts = new UnhwpNative.RenderOptions
                {
                    IncludeFrontmatter = options.IncludeFrontmatter ? 1 : 0,
                    ImagePathPrefix = IntPtr.Zero,
                    TableFallback = (int)options.TableFallback,
                    PreserveLineBreaks = options.PreserveLineBreaks ? 1 : 0,
                    EscapeSpecialChars = options.EscapeSpecialChars ? 1 : 0
                };
                var cleanupOpts = new UnhwpNative.CleanupOptions
                {
                    Enabled = options.EnableCleanup ? 1 : 0,
                    Preset = (int)options.CleanupPreset,
                    DetectMojibake = options.DetectMojibake ? 1 : 0,
                    PreserveFrontmatter = options.PreserveFrontmatter ? 1 : 0
                };
                handle = UnhwpNative.unhwp_parse(path, ref renderOpts, ref cleanupOpts);
            }
            else
            {
                handle = UnhwpNative.unhwp_parse(path, IntPtr.Zero, IntPtr.Zero);
            }

            if (handle == IntPtr.Zero)
                throw new HwpException(-1, $"Failed to parse document: {path}");

            return new HwpDocument(handle);
        }

        /// <summary>
        /// Parses HWP/HWPX bytes and returns a document object.
        /// </summary>
        public static HwpDocument ParseBytes(byte[] data, ConversionOptions? options = null)
        {
            var handle = UnhwpNative.unhwp_parse_bytes(
                data, (UIntPtr)data.Length, IntPtr.Zero, IntPtr.Zero);

            if (handle == IntPtr.Zero)
                throw new HwpException(-1, "Failed to parse document from bytes");

            return new HwpDocument(handle);
        }

        /// <summary>
        /// Gets the rendered Markdown content.
        /// </summary>
        public string Markdown
        {
            get
            {
                ThrowIfDisposed();
                if (_cachedMarkdown == null)
                {
                    var ptr = UnhwpNative.unhwp_result_get_markdown(_handle);
                    _cachedMarkdown = ptr != IntPtr.Zero
                        ? Marshal.PtrToStringUTF8(ptr) ?? string.Empty
                        : throw new HwpException(-3, GetError() ?? "Failed to render markdown");
                }
                return _cachedMarkdown;
            }
        }

        /// <summary>
        /// Gets the plain text content.
        /// </summary>
        public string RawText
        {
            get
            {
                ThrowIfDisposed();
                if (_cachedText == null)
                {
                    var ptr = UnhwpNative.unhwp_result_get_text(_handle);
                    _cachedText = ptr != IntPtr.Zero
                        ? Marshal.PtrToStringUTF8(ptr) ?? string.Empty
                        : throw new HwpException(-3, GetError() ?? "Failed to extract text");
                }
                return _cachedText;
            }
        }

        /// <summary>
        /// Gets the raw structured content as JSON.
        /// This provides access to the full document structure including:
        /// - Document metadata (title, author, dates)
        /// - Paragraph styles (heading level, alignment, list type)
        /// - Text formatting (bold, italic, underline, font, color, etc.)
        /// - Table structure (rows, cells, colspan, rowspan)
        /// - Equations, images, and links
        /// </summary>
        public string RawContent
        {
            get
            {
                ThrowIfDisposed();
                if (_cachedRawContent == null)
                {
                    var ptr = UnhwpNative.unhwp_result_get_raw_content(_handle);
                    _cachedRawContent = ptr != IntPtr.Zero
                        ? Marshal.PtrToStringUTF8(ptr) ?? "{}"
                        : throw new HwpException(-3, GetError() ?? "Failed to get raw content");
                }
                return _cachedRawContent;
            }
        }

        /// <summary>
        /// Gets the embedded images from the document.
        /// </summary>
        public IReadOnlyList<HwpImage> Images
        {
            get
            {
                ThrowIfDisposed();
                if (_cachedImages == null)
                {
                    var count = UnhwpNative.unhwp_result_get_image_count(_handle);
                    _cachedImages = new HwpImage[count];

                    for (int i = 0; i < count; i++)
                    {
                        if (UnhwpNative.unhwp_result_get_image(_handle, i, out var imageData) == 0)
                        {
                            var name = imageData.Name != IntPtr.Zero
                                ? Marshal.PtrToStringUTF8(imageData.Name) ?? $"image_{i}"
                                : $"image_{i}";

                            var dataLen = (int)imageData.DataLen;
                            var data = new byte[dataLen];
                            if (dataLen > 0 && imageData.Data != IntPtr.Zero)
                            {
                                Marshal.Copy(imageData.Data, data, 0, dataLen);
                            }

                            _cachedImages[i] = new HwpImage(name, data);
                        }
                        else
                        {
                            _cachedImages[i] = new HwpImage($"image_{i}", Array.Empty<byte>());
                        }
                    }
                }
                return _cachedImages;
            }
        }

        /// <summary>
        /// Gets the number of sections in the document.
        /// </summary>
        public int SectionCount
        {
            get
            {
                ThrowIfDisposed();
                return UnhwpNative.unhwp_result_get_section_count(_handle);
            }
        }

        /// <summary>
        /// Gets the number of paragraphs in the document.
        /// </summary>
        public int ParagraphCount
        {
            get
            {
                ThrowIfDisposed();
                return UnhwpNative.unhwp_result_get_paragraph_count(_handle);
            }
        }

        private string? GetError()
        {
            var ptr = UnhwpNative.unhwp_result_get_error(_handle);
            return ptr != IntPtr.Zero ? Marshal.PtrToStringUTF8(ptr) : null;
        }

        private void ThrowIfDisposed()
        {
            if (_disposed)
                throw new ObjectDisposedException(nameof(HwpDocument));
        }

        public void Dispose()
        {
            if (!_disposed && _handle != IntPtr.Zero)
            {
                UnhwpNative.unhwp_result_free(_handle);
                _handle = IntPtr.Zero;
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        ~HwpDocument()
        {
            Dispose();
        }
    }

    /// <summary>
    /// Represents an embedded image in an HWP document.
    /// </summary>
    public class HwpImage
    {
        public string Name { get; }
        public byte[] Data { get; }
        public int Size => Data.Length;

        internal HwpImage(string name, byte[] data)
        {
            Name = name;
            Data = data;
        }

        /// <summary>
        /// Saves the image to a file.
        /// </summary>
        public void SaveTo(string path)
        {
            File.WriteAllBytes(path, Data);
        }

        /// <summary>
        /// Gets the image data as a MemoryStream.
        /// </summary>
        public MemoryStream ToStream() => new MemoryStream(Data);
    }

    /// <summary>
    /// HWP document format types.
    /// </summary>
    public enum HwpFormat
    {
        Unknown = 0,
        Hwp5 = 1,
        Hwpx = 2,
        Hwp3 = 3
    }

    /// <summary>
    /// Table rendering fallback modes.
    /// </summary>
    public enum TableFallback
    {
        SimplifiedMarkdown = 0,
        Html = 1,
        Skip = 2
    }

    /// <summary>
    /// Cleanup preset levels.
    /// </summary>
    public enum CleanupPreset
    {
        Default = 0,
        Minimal = 1,
        Aggressive = 2
    }

    /// <summary>
    /// Conversion options for HWP to Markdown.
    /// </summary>
    public class ConversionOptions
    {
        public bool IncludeFrontmatter { get; set; } = true;
        public TableFallback TableFallback { get; set; } = TableFallback.SimplifiedMarkdown;
        public bool PreserveLineBreaks { get; set; } = false;
        public bool EscapeSpecialChars { get; set; } = true;
        public bool EnableCleanup { get; set; } = false;
        public CleanupPreset CleanupPreset { get; set; } = CleanupPreset.Default;
        public bool DetectMojibake { get; set; } = true;
        public bool PreserveFrontmatter { get; set; } = true;

        public static ConversionOptions Default => new ConversionOptions();

        public static ConversionOptions WithCleanup => new ConversionOptions
        {
            EnableCleanup = true,
            CleanupPreset = CleanupPreset.Default
        };

        public static ConversionOptions AggressiveCleanup => new ConversionOptions
        {
            EnableCleanup = true,
            CleanupPreset = CleanupPreset.Aggressive
        };
    }

    /// <summary>
    /// Exception thrown when HWP processing fails.
    /// </summary>
    public class HwpException : Exception
    {
        public int ErrorCode { get; }

        public HwpException(int errorCode, string message)
            : base($"[{errorCode}] {message}")
        {
            ErrorCode = errorCode;
        }
    }
}
```

## Usage Examples

### Recommended: Object-Oriented API (HwpDocument)

```csharp
using Unhwp;

// Parse document once, access multiple properties
using var doc = HwpDocument.Parse("document.hwp");

// Access properties like result.Markdown, result.RawText, result.RawContent, result.Images
Console.WriteLine($"Markdown length: {doc.Markdown.Length}");
Console.WriteLine($"Raw text preview: {doc.RawText[..100]}...");
Console.WriteLine($"Image count: {doc.Images.Count}");
Console.WriteLine($"Sections: {doc.SectionCount}, Paragraphs: {doc.ParagraphCount}");

// Access structured JSON with full metadata (formatting, styles, etc.)
string jsonContent = doc.RawContent;
Console.WriteLine($"JSON content length: {jsonContent.Length}");

// Save all images
foreach (var image in doc.Images)
{
    image.SaveTo($"./images/{image.Name}");
    Console.WriteLine($"Saved: {image.Name} ({image.Size} bytes)");
}
```

### Access Raw Structured Content (JSON)

```csharp
using Unhwp;
using System.Text.Json;

using var doc = HwpDocument.Parse("document.hwp");

// Get full structured content as JSON
// Includes: metadata, sections, paragraphs, text runs with formatting,
// tables, images, equations, hyperlinks, styles, and more
string jsonContent = doc.RawContent;

// Parse JSON for programmatic access
using var jsonDoc = JsonDocument.Parse(jsonContent);
var root = jsonDoc.RootElement;

// Access metadata
if (root.TryGetProperty("metadata", out var metadata))
{
    var title = metadata.GetProperty("title").GetString();
    var author = metadata.GetProperty("author").GetString();
    Console.WriteLine($"Title: {title}, Author: {author}");
}

// Iterate through sections and paragraphs
if (root.TryGetProperty("sections", out var sections))
{
    foreach (var section in sections.EnumerateArray())
    {
        var content = section.GetProperty("content");
        foreach (var block in content.EnumerateArray())
        {
            if (block.TryGetProperty("Paragraph", out var para))
            {
                // Access paragraph style (heading level, alignment, etc.)
                var style = para.GetProperty("style");
                var headingLevel = style.GetProperty("heading_level").GetInt32();

                // Access text runs with formatting
                var runs = para.GetProperty("content");
                foreach (var run in runs.EnumerateArray())
                {
                    if (run.TryGetProperty("Text", out var textRun))
                    {
                        var text = textRun.GetProperty("text").GetString();
                        var runStyle = textRun.GetProperty("style");
                        var isBold = runStyle.GetProperty("bold").GetBoolean();
                        var isItalic = runStyle.GetProperty("italic").GetBoolean();
                        Console.WriteLine($"Text: {text} (bold={isBold}, italic={isItalic})");
                    }
                }
            }
        }
    }
}

// Save JSON to file for external processing
File.WriteAllText("document.json", jsonContent);
```

### Parse with Options

```csharp
using Unhwp;

var options = new ConversionOptions
{
    EnableCleanup = true,
    CleanupPreset = CleanupPreset.Aggressive,
    DetectMojibake = true,
    IncludeFrontmatter = true
};

using var doc = HwpDocument.Parse("document.hwp", options);

// Write markdown to file
File.WriteAllText("output.md", doc.Markdown);

// Get plain text for processing
string text = doc.RawText;
Console.WriteLine($"Word count: ~{text.Split(' ').Length}");
```

### Basic Usage (Legacy API)

```csharp
using Unhwp;

// Check library version
Console.WriteLine($"unhwp version: {HwpConverter.Version}");

// Detect file format
var format = HwpConverter.DetectFormat("document.hwp");
Console.WriteLine($"Format: {format}");

// Convert to Markdown (simple)
string markdown = HwpConverter.ToMarkdown("document.hwp");
File.WriteAllText("output.md", markdown);

// Convert with cleanup enabled (for LLM training data)
string cleanMarkdown = HwpConverter.ToMarkdown("document.hwp", enableCleanup: true);
```

### Custom Options

```csharp
using Unhwp;

var options = new ConversionOptions
{
    IncludeFrontmatter = true,
    TableFallback = TableFallback.Html,
    EnableCleanup = true,
    CleanupPreset = CleanupPreset.Aggressive,
    DetectMojibake = true
};

string markdown = HwpConverter.ToMarkdown("document.hwp", options);
```

### Batch Processing

```csharp
using Unhwp;
using System.IO;
using System.Threading.Tasks;

async Task ProcessDirectory(string inputDir, string outputDir)
{
    var files = Directory.GetFiles(inputDir, "*.hwp")
        .Concat(Directory.GetFiles(inputDir, "*.hwpx"));

    await Parallel.ForEachAsync(files, async (file, ct) =>
    {
        try
        {
            var markdown = HwpConverter.ToMarkdown(file, enableCleanup: true);
            var outputPath = Path.Combine(
                outputDir,
                Path.GetFileNameWithoutExtension(file) + ".md");
            await File.WriteAllTextAsync(outputPath, markdown, ct);
            Console.WriteLine($"Converted: {file}");
        }
        catch (HwpException ex)
        {
            Console.WriteLine($"Error processing {file}: {ex.Message}");
        }
    });
}
```

### In-Memory Processing

```csharp
using Unhwp;

// From byte array
byte[] hwpData = File.ReadAllBytes("document.hwp");
string markdown = HwpConverter.BytesToMarkdown(hwpData, enableCleanup: true);

// From stream
using var stream = File.OpenRead("document.hwp");
string markdownFromStream = HwpConverter.StreamToMarkdown(stream);
```

### ASP.NET Core Integration

```csharp
// Controller example
[ApiController]
[Route("api/[controller]")]
public class DocumentController : ControllerBase
{
    [HttpPost("convert")]
    public async Task<IActionResult> ConvertHwp(IFormFile file)
    {
        if (file == null || file.Length == 0)
            return BadRequest("No file uploaded");

        using var stream = file.OpenReadStream();
        using var ms = new MemoryStream();
        await stream.CopyToAsync(ms);

        try
        {
            var options = new ConversionOptions
            {
                EnableCleanup = true,
                TableFallback = TableFallback.Html
            };

            var markdown = HwpConverter.BytesToMarkdown(ms.ToArray(), true);
            return Ok(new { markdown });
        }
        catch (HwpException ex)
        {
            return BadRequest(new { error = ex.Message });
        }
    }
}
```

## NuGet Package Setup

### Project File (.csproj)

```xml
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFramework>net10.0</TargetFramework>
    <Nullable>enable</Nullable>
  </PropertyGroup>

  <ItemGroup>
    <!-- Include native library based on runtime -->
    <None Include="runtimes\win-x64\native\unhwp.dll">
      <Pack>true</Pack>
      <PackagePath>runtimes\win-x64\native</PackagePath>
      <CopyToOutputDirectory>PreserveNewest</CopyToOutputDirectory>
    </None>
    <None Include="runtimes\linux-x64\native\libunhwp.so">
      <Pack>true</Pack>
      <PackagePath>runtimes\linux-x64\native</PackagePath>
      <CopyToOutputDirectory>PreserveNewest</CopyToOutputDirectory>
    </None>
    <None Include="runtimes\osx-x64\native\libunhwp.dylib">
      <Pack>true</Pack>
      <PackagePath>runtimes\osx-x64\native</PackagePath>
      <CopyToOutputDirectory>PreserveNewest</CopyToOutputDirectory>
    </None>
  </ItemGroup>
</Project>
```

## Error Handling

### Error Codes

| Code | Constant | Description |
|------|----------|-------------|
| 0 | `UNHWP_OK` | Success |
| -1 | `UNHWP_ERR_FILE_NOT_FOUND` | File not found |
| -2 | `UNHWP_ERR_PARSE` | Parse error (invalid/corrupted file) |
| -3 | `UNHWP_ERR_RENDER` | Render error |
| -4 | `UNHWP_ERR_INVALID_ARG` | Invalid argument (null pointer) |
| -5 | `UNHWP_ERR_UNSUPPORTED` | Unsupported format |
| -99 | `UNHWP_ERR_UNKNOWN` | Unknown error |

### Example Error Handling

```csharp
try
{
    var markdown = HwpConverter.ToMarkdown("document.hwp");
}
catch (HwpException ex) when (ex.ErrorCode == UnhwpNative.UNHWP_ERR_FILE_NOT_FOUND)
{
    Console.WriteLine("File not found");
}
catch (HwpException ex) when (ex.ErrorCode == UnhwpNative.UNHWP_ERR_PARSE)
{
    Console.WriteLine($"Failed to parse document: {ex.Message}");
}
catch (HwpException ex)
{
    Console.WriteLine($"Error: {ex.Message}");
}
```

## Memory Management

The native library allocates strings that must be freed. The C# wrapper handles this automatically through `PtrToStringAndFree()`. If you use the low-level `UnhwpNative` methods directly, ensure you call `unhwp_free_string()` for any returned string pointers.

```csharp
// Low-level usage (not recommended unless necessary)
IntPtr markdownPtr, errorPtr;
var result = UnhwpNative.unhwp_to_markdown(path, out markdownPtr, out errorPtr);

if (result == UnhwpNative.UNHWP_OK)
{
    var markdown = Marshal.PtrToStringUTF8(markdownPtr);
    UnhwpNative.unhwp_free_string(markdownPtr);  // MUST free!
}
else
{
    var error = Marshal.PtrToStringUTF8(errorPtr);
    UnhwpNative.unhwp_free_string(errorPtr);  // MUST free!
}
```

## Thread Safety

The unhwp library is thread-safe. Multiple threads can call conversion functions simultaneously without synchronization. Each call operates on independent data.

## Performance Tips

1. **Use byte arrays for repeated processing**: If processing the same file multiple times, read it once into a byte array.

2. **Batch processing**: Use `Parallel.ForEachAsync` for processing multiple files.

3. **Disable unnecessary options**: If you don't need frontmatter or cleanup, disable them for faster processing.

4. **Reuse options structs**: Create `ConversionOptions` once and reuse for multiple conversions.

## Troubleshooting

### DLL Not Found

1. Ensure `unhwp.dll` is in the same directory as your executable or in the system PATH.
2. For NuGet packages, ensure the native libraries are in the correct `runtimes/` subdirectories.
3. On Linux, you may need to set `LD_LIBRARY_PATH`.

### Encoding Issues

The library uses UTF-8 encoding. Ensure your file paths are valid UTF-8 strings.

### Korean Text Display Issues

Korean text should display correctly as the library handles HWP's internal encoding. If you see garbled text:
1. Ensure your terminal/application supports UTF-8.
2. Enable cleanup mode to remove mojibake characters.
