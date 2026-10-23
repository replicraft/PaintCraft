import AppKit
import ApplicationServices
let pid = Int32(CommandLine.arguments[1])!
func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}
func find(_ node: AXUIElement, _ title: String, _ depth: Int) -> AXUIElement? {
    if attribute(node,kAXTitleAttribute) as? String == title {return node}
    if depth < 4, let children = attribute(node,kAXChildrenAttribute) as? [AXUIElement] {
        for child in children {if let result = find(child,title,depth+1) {return result}}
    }
    return nil
}
let app = AXUIElementCreateApplication(pid)
if let menu = attribute(app,kAXMenuBarAttribute) {
    let root = menu as! AXUIElement
    if let item = find(root,"PaintCraft",0) {
        print("open_app_menu=\(AXUIElementPerformAction(item,kAXPressAction as CFString).rawValue)")
    }
    if let item = find(root,"Quit PaintCraft",0) {
        print("quit=\(AXUIElementPerformAction(item,kAXPressAction as CFString).rawValue)")
    }
}
