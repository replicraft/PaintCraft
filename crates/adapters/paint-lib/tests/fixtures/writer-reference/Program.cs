// Qualification only: unchanged pinned reader sources are compiled alongside this
// entrypoint under the installed net10 runtime. No Windows Paint behavior is implied.
using System.Security.Cryptography;
using System.Text.Json;
using System.Runtime.InteropServices;
using MsPaintFile;
using MsPaintFile.Container;
using MsPaintFile.Items;

var bytes = File.ReadAllBytes(args[0]);
using var source = new MemoryStream(bytes, false);
var document = PaintDocument.Load(source);
using var raw = new MemoryStream(bytes, false);
var reader = new BoxReader(raw);
IovlPayload? overlay = null;
while (reader.TryReadBoxHeader(reader.Length, out var box))
{
    if (box.Type.ToString() == "meta")
    {
        reader.Position = box.PayloadStart + 4;
        while (reader.TryReadBoxHeader(box.PayloadEnd, out var child))
        {
            if (child.Type.ToString() == "idat")
            {
                var payload = new byte[checked((int)child.PayloadLength)];
                raw.ReadExactly(payload);
                overlay = IovlItem.Parse(payload, document.Layers.Count);
            }
            reader.Position = child.PayloadEnd;
        }
    }
    reader.Position = box.PayloadEnd;
}
if (overlay is null)
{
    throw new Exception("No original-reader iovl payload");
}
Console.WriteLine(JsonSerializer.Serialize(new
{
    runtime = Environment.Version.ToString(),
    framework = RuntimeInformation.FrameworkDescription,
    architecture = RuntimeInformation.ProcessArchitecture.ToString(),
    width = document.Width,
    height = document.Height,
    canvas_fill8 = new int[]
    {
        document.CanvasFill.R, document.CanvasFill.G,
        document.CanvasFill.B, document.CanvasFill.A
    },
    canvas_fill16 = new int[]
    {
        overlay.FillR, overlay.FillG, overlay.FillB, overlay.FillA
    },
    overlay_flags = overlay.Flags,
    layers = document.Layers.Select(layer => new
    {
        name = layer.Name,
        width = layer.Width,
        height = layer.Height,
        origin = new[] { layer.OffsetX, layer.OffsetY },
        rgba_sha256 = Convert.ToHexStringLower(SHA256.HashData(layer.PixelsRgba))
    })
}));
