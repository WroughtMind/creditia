// All input is addressed to the installed process the smoke test launched.
import ApplicationServices
import AppKit
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 3,
      let pid = Int32(CommandLine.arguments[2]),
      let app = NSRunningApplication(processIdentifier: pid) else { exit(2) }
// AppKit hosts native file panels in an XPC service. Its windows and key
// focus can belong to that service instead of the Creditia process.
func visibleWindows() -> [[String: Any]] {
    let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
    return CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] ?? []
}
func panelService(_ row: [String: Any]) -> Bool {
    guard let owner = row[kCGWindowOwnerPID as String] as? Int32 else { return false }
    let id = NSRunningApplication(processIdentifier: owner)?.bundleIdentifier ?? ""
    let name = row[kCGWindowOwnerName as String] as? String ?? ""
    return (id + name).lowercased().contains("openandsavepanel")
}
func axString(_ element: AXUIElement, _ name: String) -> String {
    var result: CFTypeRef?
    AXUIElementCopyAttributeValue(element, name as CFString, &result)
    return result as? String ?? ""
}
func containsDialog(_ element: AXUIElement, _ depth: Int) -> Bool {
    if ["AXSheet", "AXDialog"].contains(axString(element, kAXRoleAttribute)) ||
       ["AXDialog", "AXSystemDialog"].contains(axString(element, kAXSubroleAttribute)) { return true }
    guard depth > 0 else { return false }
    var children: CFTypeRef?
    AXUIElementCopyAttributeValue(element, kAXChildrenAttribute as CFString, &children)
    return (children as? [AXUIElement] ?? []).prefix(32).contains { containsDialog($0, depth - 1) }
}
switch CommandLine.arguments[1] {
case "activate":
    _ = app.activate(options: [.activateIgnoringOtherApps])
case "input-access":
    // Never prompt or change runner permissions. A tester can record this check
    // separately on machines which do not permit targeted input automation.
    if !AXIsProcessTrusted() || !CGPreflightPostEventAccess() { exit(5) }
case "copy", "open", "escape":
    let action = CommandLine.arguments[1]
    let key: CGKeyCode = action == "copy" ? 8 : (action == "open" ? 31 : 53)
    let flags: CGEventFlags = action == "copy" ? [.maskCommand, .maskShift] :
        (action == "open" ? [.maskCommand] : [])
    for down in [true, false] {
        guard let event = CGEvent(keyboardEventSource: nil, virtualKey: key, keyDown: down) else { exit(5) }
        event.flags = flags
        if action == "escape", let panel = visibleWindows().first(where: panelService),
           let owner = panel[kCGWindowOwnerPID as String] as? Int32 {
            event.postToPid(owner)
        } else {
            event.postToPid(pid)
        }
    }
case "dialog":
    let openTitles = ["Open", "打开", "開啟"]
    if visibleWindows().contains(where: { row in
        panelService(row) || (row[kCGWindowOwnerPID as String] as? Int32 == pid &&
            openTitles.contains(row[kCGWindowName as String] as? String ?? ""))
    }) || containsDialog(AXUIElementCreateApplication(pid), 4) { exit(0) }
    exit(4)
case "inspect":
    let rows = visibleWindows().map { row -> [String: Any] in
        let owner = row[kCGWindowOwnerPID as String] as? Int32 ?? 0
        return ["owner": row[kCGWindowOwnerName as String] ?? "", "pid": owner,
                "id": row[kCGWindowNumber as String] ?? 0, "title": row[kCGWindowName as String] ?? "",
                "bundle": NSRunningApplication(processIdentifier: owner)?.bundleIdentifier ?? ""]
    }
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: rows))
case "quit":
    if !app.terminate() { exit(3) }
case "window":
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    guard let window = windows.first(where: {
        ($0[kCGWindowOwnerPID as String] as? Int32) == pid &&
        ($0[kCGWindowLayer as String] as? Int) == 0 &&
        (($0[kCGWindowBounds as String] as? [String: Any])?["Width"] as? Double ?? 0) >= 800
    }), let number = window[kCGWindowNumber as String] as? Int else { exit(4) }
    let output: [String: Any] = ["id": number, "bounds": window[kCGWindowBounds as String] ?? [:]]
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: output))
default:
    exit(2)
}
