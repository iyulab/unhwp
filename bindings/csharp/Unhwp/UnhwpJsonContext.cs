using System.Text.Json.Serialization;

namespace Unhwp;

/// <summary>
/// Source-generated serialization metadata for every type the binding exchanges with the
/// native library as JSON. Using it instead of reflection keeps the binding working in
/// trimmed, Native AOT, and other apps that disable reflection-based serialization.
/// </summary>
[JsonSerializable(typeof(string[]))]
[JsonSerializable(typeof(TableText[]))]
[JsonSerializable(typeof(RenderOptionsJson))]
internal sealed partial class UnhwpJsonContext : JsonSerializerContext
{
}
