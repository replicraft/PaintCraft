import AppKit
import Foundation

// A safe native event receiver. Each document gets its own Rust editor process;
// Finder never replaces a document that already has unsaved work.
let executable = Bundle.main.bundleURL.appendingPathComponent("Contents/Helpers/PaintCraft Editor.app/Contents/MacOS/paintcraft")
if CommandLine.arguments.dropFirst().first == "--validate-only" {
    guard FileManager.default.isExecutableFile(atPath: executable.path) else { exit(1) }
    let report: [String: Any] = ["executable": executable.path,
                              "arguments": Array(CommandLine.arguments.dropFirst(2))]
    guard let data = try? JSONSerialization.data(withJSONObject: report, options: [.sortedKeys]) else { exit(1) }
    FileHandle.standardOutput.write(data)
    exit(0)
}
final class Launcher: NSObject, NSApplicationDelegate {
    var children: [Int32: Process] = [:]
    var receivedOpen = false
    var finishedLaunching = false
    var initial: DispatchWorkItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        finishedLaunching = true
        if !receivedOpen {
            let launch = DispatchWorkItem { [self] in
                initial = nil
                if !receivedOpen { _ = start(nil) }
                stopIfIdle()
            }
            initial = launch
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15, execute: launch)
        } else { stopIfIdle() }
    }
    func application(_ sender: NSApplication, openFiles filenames: [String]) {
        beginOpen()
        var succeeded = true
        for path in filenames { if !start(path) { succeeded = false } }
        sender.reply(toOpenOrPrint: succeeded ? .success : .failure)
        stopIfIdle()
    }
    func application(_ application: NSApplication, open urls: [URL]) {
        beginOpen()
        for url in urls where url.isFileURL { _ = start(url.path) }
        stopIfIdle()
    }
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        beginOpen()
        _ = start(nil)
        return false
    }
    func beginOpen() {
        receivedOpen = true
        initial?.cancel()
        initial = nil
    }
    func start(_ path: String?) -> Bool {
        let process = Process()
        process.executableURL = executable
        process.arguments = path.map { [$0] } ?? []
        process.standardInput = FileHandle.nullDevice
        process.terminationHandler = { [weak self] child in
            DispatchQueue.main.async {
                self?.children.removeValue(forKey: child.processIdentifier)
                self?.stopIfIdle()
            }
        }
        do {
            try process.run()
            children[process.processIdentifier] = process
            return true
        } catch {
            NSAlert(error: error).runModal()
            return false
        }
    }
    func stopIfIdle() {
        if finishedLaunching && initial == nil && children.isEmpty { NSApplication.shared.terminate(nil) }
    }
}
ProcessInfo.processInfo.processName = "PaintCraft"
let app = NSApplication.shared
let launcher = Launcher()
app.delegate = launcher
app.setActivationPolicy(.accessory)
app.run()
