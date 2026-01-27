using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using Unhwp.Native;

namespace Unhwp
{
    /// <summary>
    /// Document format types.
    /// </summary>
    public enum DocumentFormat
    {
        /// <summary>Unknown format.</summary>
        Unknown = 0,
        /// <summary>HWP 5.0 binary format.</summary>
        Hwp5 = 1,
        /// <summary>HWPX XML format.</summary>
        Hwpx = 2,
        /// <summary>HWP 3.x legacy format.</summary>
        Hwp3 = 3
    }

    /// <summary>
    /// Table rendering fallback mode.
    /// </summary>
    public enum TableFallback
    {
        /// <summary>Render as Markdown tables.</summary>
        Markdown = 0,
        /// <summary>Render as HTML tables.</summary>
        Html = 1,
        /// <summary>Render as plain text.</summary>
        Text = 2
    }

    /// <summary>
    /// Cleanup preset levels.
    /// </summary>
    public enum CleanupPreset
    {
        /// <summary>Minimal cleanup - only essential normalization.</summary>
        Minimal = 0,
        /// <summary>Default cleanup - balanced normalization.</summary>
        Default = 1,
        /// <summary>Aggressive cleanup - maximum normalization.</summary>
        Aggressive = 2
    }

    /// <summary>
    /// Represents an image extracted from a document.
    /// </summary>
    public class UnhwpImage
    {
        /// <summary>Gets the image filename.</summary>
        public string Name { get; }

        /// <summary>Gets the image data.</summary>
        public byte[] Data { get; }

        internal UnhwpImage(string name, byte[] data)
        {
            Name = name;
            Data = data;
        }

        /// <summary>
        /// Saves the image to a file.
        /// </summary>
        /// <param name="path">The file path to save to.</param>
        public void Save(string path)
        {
            File.WriteAllBytes(path, Data);
        }
    }

    /// <summary>
    /// Options for rendering documents to Markdown.
    /// </summary>
    public class RenderOptions
    {
        /// <summary>Include YAML frontmatter with document metadata.</summary>
        public bool IncludeFrontmatter { get; set; }

        /// <summary>Prefix for image paths in Markdown output.</summary>
        public string? ImagePathPrefix { get; set; }

        /// <summary>Fallback mode for complex tables.</summary>
        public TableFallback TableFallback { get; set; }

        /// <summary>Preserve line breaks from the original document.</summary>
        public bool PreserveLineBreaks { get; set; }

        /// <summary>Escape special Markdown characters.</summary>
        public bool EscapeSpecialChars { get; set; } = true;

        internal NativeMethods.UnhwpRenderOptions ToNative()
        {
            return new NativeMethods.UnhwpRenderOptions
            {
                IncludeFrontmatter = IncludeFrontmatter,
                ImagePathPrefix = NativeMethods.StringToCoTaskMemUtf8(ImagePathPrefix),
                TableFallback = (int)TableFallback,
                PreserveLineBreaks = PreserveLineBreaks,
                EscapeSpecialChars = EscapeSpecialChars
            };
        }
    }

    /// <summary>
    /// Options for cleaning up extracted Markdown.
    /// </summary>
    public class CleanupOptions
    {
        /// <summary>Enable cleanup processing.</summary>
        public bool Enabled { get; set; } = true;

        /// <summary>Cleanup preset level.</summary>
        public CleanupPreset Preset { get; set; } = CleanupPreset.Default;

        /// <summary>Detect and fix mojibake (garbled characters).</summary>
        public bool DetectMojibake { get; set; } = true;

        /// <summary>Preserve YAML frontmatter during cleanup.</summary>
        public bool PreserveFrontmatter { get; set; } = true;

        /// <summary>Creates minimal cleanup options.</summary>
        public static CleanupOptions Minimal => new() { Preset = CleanupPreset.Minimal };

        /// <summary>Creates default cleanup options.</summary>
        public static CleanupOptions Default => new() { Preset = CleanupPreset.Default };

        /// <summary>Creates aggressive cleanup options.</summary>
        public static CleanupOptions Aggressive => new() { Preset = CleanupPreset.Aggressive };

        /// <summary>Creates disabled cleanup options.</summary>
        public static CleanupOptions Disabled => new() { Enabled = false };

        internal NativeMethods.UnhwpCleanupOptions ToNative()
        {
            return new NativeMethods.UnhwpCleanupOptions
            {
                Enabled = Enabled,
                Preset = (int)Preset,
                DetectMojibake = DetectMojibake,
                PreserveFrontmatter = PreserveFrontmatter
            };
        }
    }

    /// <summary>
    /// Result of parsing an HWP/HWPX document.
    /// </summary>
    public class ParseResult : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;

        internal ParseResult(IntPtr handle)
        {
            _handle = handle;

            // Check for errors
            var errorPtr = NativeMethods.unhwp_result_get_error(handle);
            if (errorPtr != IntPtr.Zero)
            {
                var error = NativeMethods.PtrToStringUtf8(errorPtr);
                Dispose();
                throw new UnhwpException(error ?? "Unknown parse error");
            }
        }

        /// <summary>Gets the rendered Markdown content.</summary>
        public string Markdown
        {
            get
            {
                ThrowIfDisposed();
                var ptr = NativeMethods.unhwp_result_get_markdown(_handle);
                return NativeMethods.PtrToStringUtf8(ptr) ?? string.Empty;
            }
        }

        /// <summary>Gets the plain text content.</summary>
        public string Text
        {
            get
            {
                ThrowIfDisposed();
                var ptr = NativeMethods.unhwp_result_get_text(_handle);
                return NativeMethods.PtrToStringUtf8(ptr) ?? string.Empty;
            }
        }

        /// <summary>Gets the raw content (without cleanup).</summary>
        public string RawContent
        {
            get
            {
                ThrowIfDisposed();
                var ptr = NativeMethods.unhwp_result_get_raw_content(_handle);
                return NativeMethods.PtrToStringUtf8(ptr) ?? string.Empty;
            }
        }

        /// <summary>Gets the number of sections in the document.</summary>
        public int SectionCount
        {
            get
            {
                ThrowIfDisposed();
                return (int)NativeMethods.unhwp_result_get_section_count(_handle);
            }
        }

        /// <summary>Gets the number of paragraphs in the document.</summary>
        public int ParagraphCount
        {
            get
            {
                ThrowIfDisposed();
                return (int)NativeMethods.unhwp_result_get_paragraph_count(_handle);
            }
        }

        /// <summary>Gets the number of images in the document.</summary>
        public int ImageCount
        {
            get
            {
                ThrowIfDisposed();
                return (int)NativeMethods.unhwp_result_get_image_count(_handle);
            }
        }

        /// <summary>Gets all images from the document.</summary>
        public IReadOnlyList<UnhwpImage> Images
        {
            get
            {
                ThrowIfDisposed();
                var images = new List<UnhwpImage>();
                var count = ImageCount;

                for (int i = 0; i < count; i++)
                {
                    if (NativeMethods.unhwp_result_get_image(_handle, (UIntPtr)i, out var img) == 0)
                    {
                        var name = NativeMethods.PtrToStringUtf8(img.Name) ?? $"image_{i}";
                        var dataLen = (int)img.DataLen;
                        var data = new byte[dataLen];
                        if (dataLen > 0)
                        {
                            Marshal.Copy(img.Data, data, 0, dataLen);
                        }
                        images.Add(new UnhwpImage(name, data));
                    }
                }

                return images;
            }
        }

        private void ThrowIfDisposed()
        {
            if (_disposed)
                throw new ObjectDisposedException(nameof(ParseResult));
        }

        /// <summary>Releases native resources.</summary>
        public void Dispose()
        {
            if (!_disposed)
            {
                if (_handle != IntPtr.Zero)
                {
                    NativeMethods.unhwp_result_free(_handle);
                    _handle = IntPtr.Zero;
                }
                _disposed = true;
            }
            GC.SuppressFinalize(this);
        }

        /// <summary>Finalizer.</summary>
        ~ParseResult()
        {
            Dispose();
        }
    }

    /// <summary>
    /// Exception thrown by unhwp operations.
    /// </summary>
    public class UnhwpException : Exception
    {
        /// <summary>Creates a new UnhwpException.</summary>
        public UnhwpException(string message) : base(message) { }
    }

    /// <summary>
    /// Main API for unhwp document processing.
    /// </summary>
    public static class UnhwpConverter
    {
        /// <summary>Gets the library version.</summary>
        public static string Version
        {
            get
            {
                var ptr = NativeMethods.unhwp_version();
                return NativeMethods.PtrToStringUtf8(ptr) ?? "unknown";
            }
        }

        /// <summary>Gets the supported formats description.</summary>
        public static string SupportedFormats
        {
            get
            {
                var ptr = NativeMethods.unhwp_supported_formats();
                return NativeMethods.PtrToStringUtf8(ptr) ?? string.Empty;
            }
        }

        /// <summary>
        /// Detects the format of a document file.
        /// </summary>
        /// <param name="path">Path to the document file.</param>
        /// <returns>The detected document format.</returns>
        public static DocumentFormat DetectFormat(string path)
        {
            var result = NativeMethods.unhwp_detect_format(NativeMethods.ToUtf8(path));
            return (DocumentFormat)result;
        }

        /// <summary>
        /// Parses an HWP/HWPX document file.
        /// </summary>
        /// <param name="path">Path to the document file.</param>
        /// <param name="options">Optional render options.</param>
        /// <returns>ParseResult containing the extracted content.</returns>
        public static ParseResult Parse(string path, RenderOptions? options = null)
        {
            var nativeOpts = (options ?? new RenderOptions()).ToNative();
            var handle = NativeMethods.unhwp_parse(NativeMethods.ToUtf8(path), nativeOpts);

            if (handle == IntPtr.Zero)
                throw new UnhwpException($"Failed to parse document: {path}");

            return new ParseResult(handle);
        }

        /// <summary>
        /// Parses an HWP/HWPX document from bytes.
        /// </summary>
        /// <param name="data">Document content as bytes.</param>
        /// <param name="options">Optional render options.</param>
        /// <returns>ParseResult containing the extracted content.</returns>
        public static ParseResult ParseBytes(byte[] data, RenderOptions? options = null)
        {
            var nativeOpts = (options ?? new RenderOptions()).ToNative();

            var pinnedData = GCHandle.Alloc(data, GCHandleType.Pinned);
            try
            {
                var handle = NativeMethods.unhwp_parse_bytes(
                    pinnedData.AddrOfPinnedObject(),
                    (UIntPtr)data.Length,
                    nativeOpts);

                if (handle == IntPtr.Zero)
                    throw new UnhwpException("Failed to parse document from bytes");

                return new ParseResult(handle);
            }
            finally
            {
                pinnedData.Free();
            }
        }

        /// <summary>
        /// Converts an HWP/HWPX document to Markdown.
        /// </summary>
        /// <param name="path">Path to the document file.</param>
        /// <returns>Markdown content as a string.</returns>
        public static string ToMarkdown(string path)
        {
            var ptr = NativeMethods.unhwp_to_markdown(NativeMethods.ToUtf8(path));
            if (ptr == IntPtr.Zero)
                throw new UnhwpException($"Failed to convert document: {path}");

            return NativeMethods.PtrToStringUtf8(ptr) ?? string.Empty;
        }

        /// <summary>
        /// Converts an HWP/HWPX document to Markdown with cleanup.
        /// </summary>
        /// <param name="path">Path to the document file.</param>
        /// <param name="cleanupOptions">Optional cleanup options.</param>
        /// <returns>Cleaned Markdown content as a string.</returns>
        public static string ToMarkdownWithCleanup(string path, CleanupOptions? cleanupOptions = null)
        {
            var nativeOpts = (cleanupOptions ?? CleanupOptions.Default).ToNative();
            var ptr = NativeMethods.unhwp_to_markdown_with_cleanup(NativeMethods.ToUtf8(path), nativeOpts);

            if (ptr == IntPtr.Zero)
                throw new UnhwpException($"Failed to convert document: {path}");

            return NativeMethods.PtrToStringUtf8(ptr) ?? string.Empty;
        }

        /// <summary>
        /// Extracts plain text from an HWP/HWPX document.
        /// </summary>
        /// <param name="path">Path to the document file.</param>
        /// <returns>Plain text content as a string.</returns>
        public static string ExtractText(string path)
        {
            var ptr = NativeMethods.unhwp_extract_text(NativeMethods.ToUtf8(path));
            if (ptr == IntPtr.Zero)
                throw new UnhwpException($"Failed to extract text: {path}");

            return NativeMethods.PtrToStringUtf8(ptr) ?? string.Empty;
        }
    }
}
