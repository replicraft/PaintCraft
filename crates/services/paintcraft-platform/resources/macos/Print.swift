import AppKit
import Foundation

// Capturing the native panel response distinguishes cancellation from spool errors.
final class PaintCraftPrintPanel: NSPrintPanel {
    var cancelled = false
    override func runModal(with printInfo: NSPrintInfo) -> Int {
        let response = super.runModal(with: printInfo)
        cancelled = response == NSApplication.ModalResponse.cancel.rawValue
        return response
    }
}

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}
let args = CommandLine.arguments
let validateOnly = args.count == 6 && args[5] == "--validate-only"
guard (args.count == 5 || validateOnly), args[1] == "--png", args[3] == "--title" else {
    fail("Expected --png PATH --title TITLE")
}
let url = URL(fileURLWithPath: args[2])
guard let attributes = try? FileManager.default.attributesOfItem(atPath: url.path),
      let count = attributes[.size] as? NSNumber, count.uint64Value <= 67_108_864,
      let data = try? Data(contentsOf: url), data.count >= 33 else {
    fail("Print image cannot be read or exceeds the encoded limit")
}
let bytes = [UInt8](data.prefix(24))
guard Array(bytes.prefix(8)) == [137,80,78,71,13,10,26,10],
      Array(bytes[8..<16]) == [0,0,0,13,73,72,68,82] else { fail("Invalid PNG header") }
func dimension(_ start: Int) -> UInt64 {
    bytes[start..<start+4].reduce(0) { ($0 << 8) | UInt64($1) }
}
let width = dimension(16), height = dimension(20)
guard width > 0, height > 0, width * height <= 16_777_216,
      let bitmap = NSBitmapImageRep(data: data),
      bitmap.pixelsWide == Int(width), bitmap.pixelsHigh == Int(height) else {
    fail("Print image cannot be decoded or exceeds the pixel limit")
}
// No application, window, print operation or print job is created in validation mode.
if validateOnly { exit(0) }
ProcessInfo.processInfo.processName = "PaintCraft"
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let image = NSImage(size: NSSize(width: Int(width), height: Int(height)))
image.addRepresentation(bitmap)
let view = NSImageView(frame: NSRect(x: 0, y: 0, width: Double(width)*0.75, height: Double(height)*0.75))
view.image = image
view.imageScaling = .scaleProportionallyUpOrDown
let info = NSPrintInfo.shared.copy() as! NSPrintInfo
info.horizontalPagination = .fit
info.verticalPagination = .fit
info.isHorizontallyCentered = true
info.isVerticallyCentered = true
let operation = NSPrintOperation(view: view, printInfo: info)
operation.jobTitle = args[4]
let panel = PaintCraftPrintPanel()
panel.options.formUnion([.showsPaperSize, .showsOrientation, .showsScaling, .showsPreview])
operation.printPanel = panel
operation.showsPrintPanel = true
operation.showsProgressPanel = true
app.activate(ignoringOtherApps: true)
let success = operation.run()
exit(success ? 0 : (panel.cancelled ? 2 : 1))
