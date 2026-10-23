import Foundation
import CoreGraphics
import ApplicationServices
let pid = Int32(CommandLine.arguments[1])!
var report: [String: Any] = ["pid": pid, "accessibility_trusted": AXIsProcessTrusted()]
if let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] {
    report["windows"] = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid }
}
func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
    var result: CFTypeRef?
    let error = AXUIElementCopyAttributeValue(element, name as CFString, &result)
    return error == .success ? result : nil
}
if AXIsProcessTrusted() {
    let app = AXUIElementCreateApplication(pid)
    if let menu = attribute(app, kAXMenuBarAttribute) {
        let element = menu as! AXUIElement
        if let items = attribute(element, kAXChildrenAttribute) as? [AXUIElement] {
            report["menu_titles"] = items.map { attribute($0, kAXTitleAttribute) as? String ?? "" }
        }
    }
}
let data = try JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
print(String(data: data, encoding: .utf8)!)
