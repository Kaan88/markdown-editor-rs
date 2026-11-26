// Native UI test driver. No global input, activation, or screen/region capture.
import AppKit
import ApplicationServices
import Foundation
import ScreenCaptureKit
import Vision

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data(("mac UI: " + message + "\n").utf8))
    exit(1)
}

func attr(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}

func title(_ element: AXUIElement) -> String? { attr(element, kAXTitleAttribute) as? String }
func children(_ element: AXUIElement) -> [AXUIElement] {
    attr(element, kAXChildrenAttribute) as? [AXUIElement] ?? []
}

func hasOpenButton(_ element: AXUIElement, _ depth: Int = 4) -> Bool {
    if title(element) == "Open", attr(element, kAXRoleAttribute) as? String == kAXButtonRole { return true }
    return depth > 0 && children(element).contains { hasOpenButton($0, depth - 1) }
}

func frame(_ element: AXUIElement) -> CGRect {
    guard let p = attr(element, kAXPositionAttribute), let s = attr(element, kAXSizeAttribute),
          CFGetTypeID(p) == AXValueGetTypeID(), CFGetTypeID(s) == AXValueGetTypeID() else {
        fail("scratch window has no accessible geometry")
    }
    var origin = CGPoint.zero
    var size = CGSize.zero
    guard AXValueGetValue(p as! AXValue, .cgPoint, &origin),
          AXValueGetValue(s as! AXValue, .cgSize, &size), size.width > 0, size.height > 0 else {
        fail("invalid scratch window geometry")
    }
    return CGRect(origin: origin, size: size)
}

func containsBookmark(_ item: Any, _ document: URL) -> Bool {
    if let data = item as? Data {
        var stale = false
        guard let url = try? URL(resolvingBookmarkData: data, options: [.withoutUI, .withoutMounting],
                                 relativeTo: nil, bookmarkDataIsStale: &stale) else { return false }
        return url.isFileURL && url.resolvingSymlinksInPath().path == document.path
    }
    if let list = item as? [Any] { return list.contains { containsBookmark($0, document) } }
    if let dict = item as? [String: Any] { return dict.values.contains { containsBookmark($0, document) } }
    return false
}

func hasQuitTitle(_ path: String) throws -> Bool {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.recognitionLanguages = ["en-US"]
    try VNImageRequestHandler(url: URL(fileURLWithPath: path), options: [:]).perform([request])
    return (request.results ?? []).contains {
        $0.topCandidates(1).first?.string.trimmingCharacters(in: .whitespacesAndNewlines) == "Save changes?"
    }
}

@main
struct MacUI {
    @MainActor static func main() async throws {
        let a = CommandLine.arguments
        guard a.count >= 4 else { fail("expected action, executable and bundle identity") }
        let action = a[1], id = a[3]
        // The hand-drawn confirmation has no AX element. Read its heading from the
        // already verified window capture, not from a region of the desktop.
        if action == "quit-title" {
            guard a.count == 4, ["open", "closed"].contains(id),
                  try hasQuitTitle(a[2]) == (id == "open") else {
                fail("Quit confirmation did not reach the expected state")
            }
            return
        }
        let expectedExe = URL(fileURLWithPath: a[2]).resolvingSymlinksInPath().path
        func executable(_ app: NSRunningApplication) -> Bool {
            app.executableURL?.resolvingSymlinksInPath().path == expectedExe
        }
        let matches = NSWorkspace.shared.runningApplications.filter {
            !$0.isTerminated && ($0.bundleIdentifier == id || executable($0))
        }
        if action == "absent" {
            guard matches.isEmpty else { fail("scratch identity or executable already running") }
            return
        }
        if action == "discover" {
            guard matches.count == 1, matches[0].bundleIdentifier == id, executable(matches[0]) else {
                fail("no unique scratch process")
            }
            print(matches[0].processIdentifier)
            return
        }
        guard a.count >= 5, let pid = Int32(a[4]), pid > 0 else { fail("invalid scratch PID") }
        if action == "terminate", matches.isEmpty { return }
        guard matches.count == 1, let app = matches.first, app.processIdentifier == pid,
              app.bundleIdentifier == id, executable(app) else { fail("scratch process identity changed") }
        if action == "terminate" {
            // A dirty test document has already exercised Quit; cleanup must not ask again.
            guard app.forceTerminate() else { fail("scratch process refused termination") }
            for _ in 0..<20 {
                if app.isTerminated { return }
                try await Task.sleep(nanoseconds: 100_000_000)
            }
            guard app.isTerminated else { fail("scratch process did not exit") }
            return
        }
        guard a.count >= 8 else { fail("expected document, editor state and window kind") }
        let doc = URL(fileURLWithPath: a[5]).resolvingSymlinksInPath()
        guard ["clean", "dirty"].contains(a[6]), ["main", "open"].contains(a[7]) else { fail("bad window state") }
        let expectedTitle = (a[6] == "dirty" ? "• " : "") + doc.lastPathComponent + " — mdedit"
        let ax = AXUIElementCreateApplication(pid)
        guard let windows = attr(ax, kAXWindowsAttribute) as? [AXUIElement] else { fail("no accessible scratch windows") }
        let editors = windows.filter { title($0) == expectedTitle }
        guard editors.count == 1, let editor = editors.first else { fail("expected scratch document is not open") }
        // The scratch document's unique basename is also the window title. If AppKit
        // publishes AXDocument, require the exact path as well.
        if let value = attr(editor, kAXDocumentAttribute) as? String {
            let parsed = URL(string: value)
            let url = parsed.flatMap { $0.isFileURL ? $0 : nil } ?? URL(fileURLWithPath: value)
            guard url.resolvingSymlinksInPath().path == doc.path else { fail("scratch document path mismatch") }
        }
        let panel = a[7] == "open"
        let others = windows.filter { !CFEqual($0, editor) }
        let sheets = children(editor).filter { attr($0, kAXRoleAttribute) as? String == kAXSheetRole }
        let target: AXUIElement
        if panel {
            let dialogs = others + sheets.filter { sheet in !others.contains { CFEqual($0, sheet) } }
            guard dialogs.count == 1, let dialog = dialogs.first,
                  (title(dialog) == "Open" || hasOpenButton(dialog)),
                  let role = attr(dialog, kAXRoleAttribute) as? String,
                  role == kAXSheetRole || role == kAXWindowRole else { fail("no unique owned Open panel") }
            target = dialog
        } else {
            guard others.isEmpty && sheets.isEmpty else { fail("unexpected window or sheet") }
            target = editor
        }
        let bounds = frame(target)
        let info = CGWindowListCopyWindowInfo(.optionAll, kCGNullWindowID) as? [[String: Any]] ?? []
        let ids = info.compactMap { row -> UInt32? in
            guard row[kCGWindowOwnerPID as String] as? Int32 == pid,
                  let dict = row[kCGWindowBounds as String] as? [String: Any],
                  let rect = CGRect(dictionaryRepresentation: dict as CFDictionary),
                  abs(bounds.minX - rect.minX) <= 2, abs(bounds.minY - rect.minY) <= 2,
                  abs(bounds.width - rect.width) <= 2, abs(bounds.height - rect.height) <= 2 else { return nil }
            return row[kCGWindowNumber as String] as? UInt32
        }
        guard ids.count == 1, let window = ids.first else { fail("no unique scratch window owned by PID") }
        func stillOwned() -> Bool {
            guard !app.isTerminated, executable(app),
                  let rows = CGWindowListCopyWindowInfo(.optionIncludingWindow, window) as? [[String: Any]] else {
                return false
            }
            return rows.contains {
                $0[kCGWindowNumber as String] as? UInt32 == window && $0[kCGWindowOwnerPID as String] as? Int32 == pid
            }
        }
        if action == "window" { print(window); return }
        if action == "fullscreen" {
            guard !panel, stillOwned(), a.count == 9, ["true", "false"].contains(a[8]),
                  let full = attr(editor, "AXFullScreen") as? Bool,
                  full == (a[8] == "true") else { fail("scratch window did not reach expected full-screen state") }
            print(window)
            return
        }
        guard a.count >= 9, UInt32(a[8]) == window else { fail("scratch window changed") }
        switch action {
        case "capture":
            guard a.count == 10 else { fail("expected capture destination") }
            let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: false)
            guard let owned = content.windows.first(where: {
                $0.windowID == window && $0.owningApplication?.processID == pid &&
                $0.owningApplication?.bundleIdentifier == id
            }) else { fail("capture could not identify the owned window") }
            let config = SCStreamConfiguration()
            config.width = Int(owned.frame.width * 2)
            config.height = Int(owned.frame.height * 2)
            config.showsCursor = false
            let image = try await SCScreenshotManager.captureImage(
                contentFilter: SCContentFilter(desktopIndependentWindow: owned), configuration: config)
            guard stillOwned(),
                  let png = NSBitmapImageRep(cgImage: image).representation(using: .png, properties: [:]) else {
                fail("could not encode scratch window")
            }
            try png.write(to: URL(fileURLWithPath: a[9]), options: .atomic)
        case "keys":
            guard a.count > 9 else { fail("expected PID-targeted key sequence") }
            for key in a.dropFirst(9) {
                let parts = key.split(separator: ":")
                guard let code = parts.first.flatMap({ UInt16($0) }), code < 128 else { fail("invalid virtual key") }
                var flags = CGEventFlags()
                for mod in parts.dropFirst() {
                    switch mod {
                    case "cmd": flags.insert(.maskCommand)
                    case "opt": flags.insert(.maskAlternate)
                    case "shift": flags.insert(.maskShift)
                    case "ctrl": flags.insert(.maskControl)
                    default: fail("invalid modifier")
                    }
                }
                let cleanTitle = doc.lastPathComponent + " — mdedit"
                guard stillOwned(), let currentTitle = title(editor),
                      currentTitle == cleanTitle || currentTitle == "• " + cleanTitle,
                      let down = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: true),
                      let up = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: false) else {
                    fail("scratch input target disappeared")
                }
                down.flags = flags
                up.flags = flags
                down.postToPid(pid)
                up.postToPid(pid)
                try await Task.sleep(nanoseconds: 60_000_000)
            }
        case "edit-menu":
            guard stillOwned(), !panel, let bar = attr(ax, kAXMenuBarAttribute), CFGetTypeID(bar) == AXUIElementGetTypeID(),
                  let edit = children(bar as! AXUIElement).first(where: { title($0) == "Edit" }),
                  AXUIElementPerformAction(edit, kAXPressAction as CFString) == .success else {
                fail("could not open the scratch Edit menu")
            }
        case "recent":
            let folder = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent(
                "Library/Application Support/com.apple.sharedfilelist/com.apple.LSSharedFileList.ApplicationRecentDocuments")
            let found = (try? FileManager.default.contentsOfDirectory(at: folder, includingPropertiesForKeys: nil)) ?? []
            guard found.contains(where: { url in
                guard url.deletingPathExtension().lastPathComponent.lowercased() == id.lowercased(),
                      let data = try? Data(contentsOf: url),
                      let plist = try? PropertyListSerialization.propertyList(from: data, format: nil) else { return false }
                return containsBookmark(plist, doc)
            }) else { fail("the scratch identity's recent list does not contain the exact document") }
        default: fail("unknown scratch UI action")
        }
    }
}
