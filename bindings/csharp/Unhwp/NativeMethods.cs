using System;
using System.Runtime.InteropServices;
using System.Text;

namespace Unhwp.Native
{
    /// <summary>
    /// P/Invoke declarations for the unhwp native library.
    /// </summary>
    internal static class NativeMethods
    {
        private const string LibraryName = "unhwp";

        /// <summary>
        /// Converts a managed string to a UTF-8 encoded byte array with null terminator.
        /// </summary>
        internal static byte[] ToUtf8(string str)
        {
            if (str == null) return new byte[] { 0 };
            var bytes = Encoding.UTF8.GetBytes(str);
            var result = new byte[bytes.Length + 1];
            Array.Copy(bytes, result, bytes.Length);
            return result;
        }

        // Error codes
        public const int UNHWP_OK = 0;
        public const int UNHWP_ERR_FILE_NOT_FOUND = 1;
        public const int UNHWP_ERR_PARSE = 2;
        public const int UNHWP_ERR_RENDER = 3;
        public const int UNHWP_ERR_INVALID_ARG = 4;
        public const int UNHWP_ERR_UNSUPPORTED = 5;
        public const int UNHWP_ERR_UNKNOWN = 99;

        // Format constants
        public const int FORMAT_UNKNOWN = 0;
        public const int FORMAT_HWP5 = 1;
        public const int FORMAT_HWPX = 2;
        public const int FORMAT_HWP3 = 3;

        /// <summary>
        /// Cleanup options structure.
        /// </summary>
        [StructLayout(LayoutKind.Sequential)]
        public struct UnhwpCleanupOptions
        {
            [MarshalAs(UnmanagedType.I1)]
            public bool Enabled;

            public int Preset; // 0=minimal, 1=default, 2=aggressive

            [MarshalAs(UnmanagedType.I1)]
            public bool DetectMojibake;

            [MarshalAs(UnmanagedType.I1)]
            public bool PreserveFrontmatter;

            public static UnhwpCleanupOptions Default => new()
            {
                Enabled = true,
                Preset = 1,
                DetectMojibake = true,
                PreserveFrontmatter = true
            };

            public static UnhwpCleanupOptions Minimal => new()
            {
                Enabled = true,
                Preset = 0,
                DetectMojibake = true,
                PreserveFrontmatter = true
            };

            public static UnhwpCleanupOptions Aggressive => new()
            {
                Enabled = true,
                Preset = 2,
                DetectMojibake = true,
                PreserveFrontmatter = true
            };

            public static UnhwpCleanupOptions Disabled => new()
            {
                Enabled = false,
                Preset = 1,
                DetectMojibake = false,
                PreserveFrontmatter = true
            };
        }

        /// <summary>
        /// Render options structure.
        /// </summary>
        [StructLayout(LayoutKind.Sequential)]
        public struct UnhwpRenderOptions
        {
            [MarshalAs(UnmanagedType.I1)]
            public bool IncludeFrontmatter;

            public IntPtr ImagePathPrefix; // char*

            public int TableFallback; // 0=markdown, 1=html, 2=text

            [MarshalAs(UnmanagedType.I1)]
            public bool PreserveLineBreaks;

            [MarshalAs(UnmanagedType.I1)]
            public bool EscapeSpecialChars;

            public static UnhwpRenderOptions Default => new()
            {
                IncludeFrontmatter = false,
                ImagePathPrefix = IntPtr.Zero,
                TableFallback = 0,
                PreserveLineBreaks = false,
                EscapeSpecialChars = true
            };
        }

        /// <summary>
        /// Image data structure.
        /// </summary>
        [StructLayout(LayoutKind.Sequential)]
        public struct UnhwpImage
        {
            public IntPtr Name;      // char*
            public IntPtr Data;      // uint8_t*
            public UIntPtr DataLen;  // size_t
        }

        // Version and info
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_version();

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_supported_formats();

        // Format detection
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_detect_format(byte[] path);

        // Options
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern UnhwpCleanupOptions unhwp_cleanup_options_default();

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern UnhwpCleanupOptions unhwp_cleanup_options_enabled(int preset);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern UnhwpRenderOptions unhwp_render_options_default();

        // Simple conversion functions
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_to_markdown(byte[] path);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_to_markdown_with_cleanup(
            byte[] path,
            UnhwpCleanupOptions cleanupOptions);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_to_markdown_ex(
            byte[] path,
            UnhwpRenderOptions renderOptions,
            UnhwpCleanupOptions cleanupOptions);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_extract_text(byte[] path);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_bytes_to_markdown(IntPtr data, UIntPtr dataLen);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_bytes_to_markdown_ex(
            IntPtr data,
            UIntPtr dataLen,
            UnhwpRenderOptions renderOptions,
            UnhwpCleanupOptions cleanupOptions);

        // Parse functions (returning result handle)
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_parse(
            byte[] path,
            UnhwpRenderOptions renderOptions);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_parse_bytes(
            IntPtr data,
            UIntPtr dataLen,
            UnhwpRenderOptions renderOptions);

        // Result accessors
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_result_get_markdown(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_result_get_text(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_result_get_raw_content(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern UIntPtr unhwp_result_get_image_count(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern int unhwp_result_get_image(IntPtr result, UIntPtr index, out UnhwpImage image);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern UIntPtr unhwp_result_get_section_count(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern UIntPtr unhwp_result_get_paragraph_count(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern IntPtr unhwp_result_get_error(IntPtr result);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void unhwp_result_free(IntPtr result);

        // Memory management
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        public static extern void unhwp_free_string(IntPtr str);

        /// <summary>
        /// Converts a native UTF-8 string pointer to a managed string.
        /// </summary>
        public static string? PtrToStringUtf8(IntPtr ptr)
        {
            if (ptr == IntPtr.Zero)
                return null;

#if NETSTANDARD2_0
            // Manual UTF-8 decoding for netstandard2.0
            int len = 0;
            while (Marshal.ReadByte(ptr, len) != 0)
                len++;

            if (len == 0)
                return string.Empty;

            var buffer = new byte[len];
            Marshal.Copy(ptr, buffer, 0, len);
            return Encoding.UTF8.GetString(buffer);
#else
            return Marshal.PtrToStringUTF8(ptr);
#endif
        }

        /// <summary>
        /// Allocates a UTF-8 string in unmanaged memory.
        /// </summary>
        public static IntPtr StringToCoTaskMemUtf8(string? str)
        {
            if (str == null)
                return IntPtr.Zero;

#if NETSTANDARD2_0
            var bytes = Encoding.UTF8.GetBytes(str);
            var ptr = Marshal.AllocCoTaskMem(bytes.Length + 1);
            Marshal.Copy(bytes, 0, ptr, bytes.Length);
            Marshal.WriteByte(ptr, bytes.Length, 0);
            return ptr;
#else
            return Marshal.StringToCoTaskMemUTF8(str);
#endif
        }
    }
}
