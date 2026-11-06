// All input is addressed to the installed process the smoke test launched.
import ApplicationServices
import AppKit
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 3,
      let pid = Int32(CommandLine.arguments[2]),
      let app = NSRunningApplication(processIdentifier: pid) else { exit(2) }
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
        event.postToPid(pid)
    }
case "dialog":
    let target = AXUIElementCreateApplication(pid)
    var raw: CFTypeRef?
    guard AXUIElementCopyAttributeValue(target, kAXWindowsAttribute as CFString, &raw) == .success,
          let windows = raw as? [AXUIElement] else { exit(4) }
    for window in windows {
        var rawChildren: CFTypeRef?
        if AXUIElementCopyAttributeValue(window, kAXChildrenAttribute as CFString, &rawChildren) == .success,
           let children = rawChildren as? [AXUIElement] {
            for child in children {
                var role: CFTypeRef?
                if AXUIElementCopyAttributeValue(child, kAXRoleAttribute as CFString, &role) == .success,
                   role as? String == "AXSheet" { exit(0) }
            }
        }
        var subrole: CFTypeRef?
        if AXUIElementCopyAttributeValue(window, kAXSubroleAttribute as CFString, &subrole) == .success,
           subrole as? String == "AXDialog" { exit(0) }
    }
    exit(4)
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
