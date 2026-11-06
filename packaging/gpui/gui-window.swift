// The installer smoke test addresses only the process it launched. It does not
// require Accessibility automation or send keys to another application.
import AppKit
import CoreGraphics
import Foundation

guard CommandLine.arguments.count == 3,
      let pid = Int32(CommandLine.arguments[2]),
      let app = NSRunningApplication(processIdentifier: pid) else { exit(2) }
switch CommandLine.arguments[1] {
case "activate":
    _ = app.activate(options: [.activateIgnoringOtherApps])
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
