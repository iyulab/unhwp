using System;
using System.Reflection;
using System.Runtime.InteropServices;

namespace Unhwp;

/// <summary>
/// P/Invoke declarations for the unhwp native library.
/// </summary>
internal static class NativeMethods
{
    // On Windows, we use unhwp_native.dll to avoid conflict with managed Unhwp.dll
    // On Unix, libunhwp.so/dylib doesn't conflict with Unhwp.dll
    private const string LibraryName = "unhwp";

    static NativeMethods()
    {
        NativeLibrary.SetDllImportResolver(typeof(NativeMethods).Assembly, ResolveDllImport);
    }

    private static IntPtr ResolveDllImport(string libraryName, Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (libraryName != LibraryName)
            return IntPtr.Zero;

        // Try platform-specific names
        string[] namesToTry;
        if (RuntimeInformation.IsOSPlatform(OSPlatform.Windows))
        {
            // On Windows, try unhwp_native.dll first (for test scenarios),
            // then fall back to unhwp.dll (for NuGet package scenarios)
            namesToTry = new[] { "unhwp_native", "unhwp" };
        }
        else
        {
            namesToTry = new[] { "unhwp" };
        }

        foreach (var name in namesToTry)
        {
            if (NativeLibrary.TryLoad(name, assembly, searchPath, out var handle))
                return handle;
        }

        return IntPtr.Zero;
    }

    // Flags for markdown rendering
    public const int UNHWP_FLAG_FRONTMATTER = 1;
    public const int UNHWP_FLAG_ESCAPE_SPECIAL = 2;
    public const int UNHWP_FLAG_PARAGRAPH_SPACING = 4;

    // JSON format options
    public const int UNHWP_JSON_PRETTY = 0;
    public const int UNHWP_JSON_COMPACT = 1;

    /// <summary>
    /// Get the library version.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_version();

    /// <summary>
    /// Get the last error message.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_last_error();

    /// <summary>
    /// Parse a document from a file path.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr unhwp_parse_file([MarshalAs(UnmanagedType.LPUTF8Str)] string path);

    /// <summary>
    /// Parse a document from a byte buffer.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_parse_bytes(IntPtr data, UIntPtr len);

    /// <summary>
    /// Free a document handle.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void unhwp_free_document(IntPtr doc);

    /// <summary>
    /// Convert a document to Markdown.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_to_markdown(IntPtr doc, int flags);

    /// <summary>
    /// Convert a document to plain text.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_to_text(IntPtr doc);

    /// <summary>
    /// Convert a document to JSON.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_to_json(IntPtr doc, int format);

    /// <summary>
    /// Get the plain text content of a document.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_plain_text(IntPtr doc);

    /// <summary>
    /// Get the number of sections in a document.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int unhwp_section_count(IntPtr doc);

    /// <summary>
    /// Get the number of resources in a document.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern int unhwp_resource_count(IntPtr doc);

    /// <summary>
    /// Get the document title.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_get_title(IntPtr doc);

    /// <summary>
    /// Get the document author.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_get_author(IntPtr doc);

    /// <summary>
    /// Free a string allocated by the library.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void unhwp_free_string(IntPtr str);

    /// <summary>
    /// Get all resource IDs as a JSON array.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern IntPtr unhwp_get_resource_ids(IntPtr doc);

    /// <summary>
    /// Get resource metadata as JSON.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr unhwp_get_resource_info(
        IntPtr doc,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string resourceId);

    /// <summary>
    /// Get resource binary data.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl, CharSet = CharSet.Ansi)]
    public static extern IntPtr unhwp_get_resource_data(
        IntPtr doc,
        [MarshalAs(UnmanagedType.LPUTF8Str)] string resourceId,
        out UIntPtr outLen);

    /// <summary>
    /// Free binary data allocated by the library.
    /// </summary>
    [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
    public static extern void unhwp_free_bytes(IntPtr data, UIntPtr len);
}
