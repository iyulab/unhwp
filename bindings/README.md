# unhwp Language Bindings

This directory contains language bindings for the unhwp Rust library.

## Available Bindings

| Language | Package | Status |
|----------|---------|--------|
| Python | [unhwp](https://pypi.org/project/unhwp/) | [![PyPI](https://img.shields.io/pypi/v/unhwp)](https://pypi.org/project/unhwp/) |
| C# / .NET | [Unhwp](https://www.nuget.org/packages/Unhwp/) | [![NuGet](https://img.shields.io/nuget/v/Unhwp)](https://www.nuget.org/packages/Unhwp/) |

## Architecture

All bindings use the native Rust library via FFI (Foreign Function Interface). The Rust library is compiled to a shared library (`.dll`, `.so`, `.dylib`) and included in the package.

```
┌─────────────────┐     ┌─────────────────┐
│  Python App     │     │  C# / .NET App  │
└────────┬────────┘     └────────┬────────┘
         │                       │
    ┌────▼────┐             ┌────▼────┐
    │ ctypes  │             │ P/Invoke│
    └────┬────┘             └────┬────┘
         │                       │
         └───────────┬───────────┘
                     │
              ┌──────▼──────┐
              │ unhwp.dll   │
              │ libunhwp.so │
              │ libunhwp.   │
              │   dylib     │
              └──────┬──────┘
                     │
              ┌──────▼──────┐
              │ Rust Core   │
              │ (unhwp)     │
              └─────────────┘
```

## Platform Support

| Platform | Architecture | Python | .NET |
|----------|--------------|--------|------|
| Windows | x64 | ✅ | ✅ |
| Linux | x64 | ✅ | ✅ |
| macOS | x64 | ✅ | ✅ |
| macOS | ARM64 | ✅ | ✅ |

## Building from Source

### Prerequisites

- Rust toolchain (1.70+)
- Python 3.9+ (for Python bindings)
- .NET SDK 6.0+ (for C# bindings)

### Build Native Library

```bash
# From repository root
cargo build --release

# Output locations:
# Windows: target/release/unhwp.dll
# Linux: target/release/libunhwp.so
# macOS: target/release/libunhwp.dylib
```

### Build Python Package

```bash
cd bindings/python

# Copy native library
mkdir -p src/unhwp/lib/win-x64
cp ../../target/release/unhwp.dll src/unhwp/lib/win-x64/

# Build wheel
pip install build
python -m build
```

### Build NuGet Package

```bash
cd bindings/csharp

# Copy native libraries
mkdir -p Unhwp/runtimes/win-x64/native
cp ../../target/release/unhwp.dll Unhwp/runtimes/win-x64/native/

# Build package
dotnet pack Unhwp/Unhwp.csproj -c Release
```

## CI/CD

The bindings are built and published automatically via GitHub Actions:

- **Trigger**: Push tags starting with `v` (e.g., `v0.1.10`)
- **Workflow**: `.github/workflows/bindings.yml`
- **Artifacts**:
  - Native libraries for all platforms
  - Python wheels
  - NuGet package

### Manual Publishing

```bash
# Trigger workflow manually
gh workflow run bindings.yml -f publish=true
```

### Required Secrets

| Secret | Description |
|--------|-------------|
| `PYPI_API_TOKEN` | PyPI API token for publishing |
| `NUGET_API_KEY` | NuGet API key for publishing |

## Directory Structure

```
bindings/
├── README.md           # This file
├── python/
│   ├── pyproject.toml  # Python package config
│   ├── README.md       # Python-specific docs
│   ├── src/
│   │   └── unhwp/
│   │       ├── __init__.py
│   │       ├── _native.py    # ctypes FFI bindings
│   │       ├── unhwp.py      # High-level API
│   │       └── lib/          # Native libraries (packaged)
│   └── tests/
│       └── test_unhwp.py
└── csharp/
    ├── Unhwp.sln       # Solution file
    ├── Unhwp/
    │   ├── Unhwp.csproj      # NuGet package config
    │   ├── README.md         # C#-specific docs
    │   ├── Unhwp.cs          # High-level API
    │   ├── NativeMethods.cs  # P/Invoke declarations
    │   └── runtimes/         # Native libraries (packaged)
    └── Unhwp.Tests/
        ├── Unhwp.Tests.csproj
        └── UnhwpTests.cs
```

## FFI Interface

The native library exports a C-compatible FFI interface defined in `src/ffi.rs`. Key functions:

| Function | Description |
|----------|-------------|
| `unhwp_version()` | Get library version |
| `unhwp_detect_format()` | Detect document format |
| `unhwp_to_markdown()` | Convert to Markdown |
| `unhwp_extract_text()` | Extract plain text |
| `unhwp_parse()` | Parse with full options |
| `unhwp_result_*()` | Access parse results |
| `unhwp_free_*()` | Free allocated memory |

See `src/ffi.rs` for complete documentation.

## Contributing

1. Make changes to the Rust library
2. Update FFI interface if needed (`src/ffi.rs`)
3. Update bindings to match FFI changes
4. Add tests for new functionality
5. Update version numbers consistently

## License

MIT License - see [LICENSE](../LICENSE) for details.
