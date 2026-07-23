import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)

    let channel = FlutterMethodChannel(
      name: "lynqo/window_style",
      binaryMessenger: flutterViewController.engine.binaryMessenger
    )
    channel.setMethodCallHandler { [weak self] (call, result) in
      guard let self = self else {
        result(FlutterError(code: "UNAVAILABLE", message: "Window not available", details: nil))
        return
      }
      if call.method == "setPopupStyle" {
        self.standardWindowButton(.closeButton)?.isHidden = true
        self.standardWindowButton(.miniaturizeButton)?.isHidden = true
        self.standardWindowButton(.zoomButton)?.isHidden = true
        self.titlebarAppearsTransparent = true
        self.titleVisibility = .hidden
        result(nil)
      } else if call.method == "setNormalStyle" {
        self.standardWindowButton(.closeButton)?.isHidden = false
        self.standardWindowButton(.miniaturizeButton)?.isHidden = false
        self.standardWindowButton(.zoomButton)?.isHidden = false
        self.titlebarAppearsTransparent = false
        self.titleVisibility = .visible
        result(nil)
      } else if call.method == "simulatePaste" {
        self.simulatePaste()
        result(nil)
      } else {
        result(FlutterMethodNotImplemented)
      }
    }

    super.awakeFromNib()
  }

  private func writeDebugLog(_ message: String) {
    let homeDir = FileManager.default.homeDirectoryForCurrentUser
    let logFile = homeDir.appendingPathComponent(".lynqo/paste_debug.log")
    let formatter = DateFormatter()
    formatter.dateFormat = "yyyy-MM-dd HH:mm:ss"
    let timestamp = formatter.string(from: Date())
    let logLine = "[\(timestamp)] \(message)\n"
    
    let folder = logFile.deletingLastPathComponent()
    try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    
    if let data = logLine.data(using: .utf8) {
      if let fileHandle = try? FileHandle(forWritingTo: logFile) {
        fileHandle.seekToEndOfFile()
        fileHandle.write(data)
        fileHandle.closeFile()
      } else {
        try? logLine.write(to: logFile, atomically: true, encoding: .utf8)
      }
    }
  }

  private func simulatePaste() {
    let isTrusted = AXIsProcessTrusted()
    writeDebugLog("simulatePaste called. AXIsProcessTrusted = \(isTrusted)")
    
    if !isTrusted {
      writeDebugLog("Process not trusted. Prompting user...")
      let options = [kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String: true] as CFDictionary
      AXIsProcessTrustedWithOptions(options)
      return
    }
    
    writeDebugLog("Hiding and deactivating lynqo application...")
    NSApp.hide(nil)
    NSApp.deactivate()
    
    writeDebugLog("Scheduling simulated paste after 350ms delay...")
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.35) {
      self.writeDebugLog("Posting key events...")
      let source = CGEventSource(stateID: .combinedSessionState)
      
      // Key code 9: 'v'
      let vDown = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: true)
      vDown?.flags = .maskCommand
      
      let vUp = CGEvent(keyboardEventSource: source, virtualKey: 9, keyDown: false)
      vUp?.flags = .maskCommand
      
      // Post events in sequence to session event tap
      vDown?.post(tap: .cgSessionEventTap)
      vUp?.post(tap: .cgSessionEventTap)
      
      self.writeDebugLog("Key events posted successfully.")
    }
  }
}
