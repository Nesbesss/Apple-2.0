// speech-helper: the dictation sidecar for Nesa.
//
// Records from the microphone while the app asks, streams the audio through Apple's on-device
// speech engine (SpeechAnalyzer with DictationTranscriber) and reports results as JSON lines.
// It runs as a separate process so a speech failure can never take down the terminal sessions.
//
//   stdin   {"cmd":"start","words":["Kubernetes"],"builtin":true} | {"cmd":"stop"} | {"cmd":"cancel"}
//           words are the user's vocabulary for this hold. builtin adds contextualWords when true.
//           {"cmd":"watch"}            starts the key watch if needed and reports the state
//           {"cmd":"target"}           remembers the app the user is in, for a paste
//           {"cmd":"paste","text":"...","return":false,"learn":false}
//           {"cmd":"request_access"}   asks for Input Monitoring and Accessibility
//   stdout  {"event":"ready"}
//           {"event":"state","state":"downloading"}
//           {"event":"listening"}
//           {"event":"level","value":0...1}
//           {"event":"partial","text":"..."}
//           {"event":"final","text":"..."}     once per stop; empty when nothing was heard
//           {"event":"cancelled"}
//           {"event":"error","message":"..."}
//           {"event":"tap","state":"on"|"off"|"unavailable"}   the key watch
//           {"event":"flags","alt":true,"ctrl":false,"meta":false,"shift":false}
//           {"event":"input"}                  a key or a click came in (never which key)
//           {"event":"target","app":"Notes"}   the app the words will go to, when a hold begins
//           {"event":"edit","inserted":"...","edited":"..."}   a fix in the field the words went into
//           {"event":"access","input":true,"accessibility":true}
//           {"event":"paste","ok":true,"app":"Notes"} or {"event":"paste","ok":false,"reason":"..."}

import AVFAudio
import FluidAudio
import AVFoundation
import AppKit
import ApplicationServices
import Carbon.HIToolbox
import Foundation
import Speech

/// Developer vocabulary that general speech models tend to miss. Biases recognition toward it.
let contextualWords = [
    "Claude", "Claude Code", "Codex", "Anthropic", "Kubernetes", "gitignore", "pull request",
    "commit", "rebase", "Tauri", "Rust", "Cargo", "npm", "Xcode", "xterm", "localhost", "stdout",
    "JSON", "TypeScript", "SwiftUI", "GitHub", "Nesa", "pty", "merge", "diff",
]

/// The words the engine should expect for one hold: the built-in developer words when `builtin` is on,
/// then the user's own. A word that repeats ignoring case is listed once, so "Claude" and "claude" count
/// as one.
func contextualHints(_ words: [String], builtin: Bool) -> [String] {
    var seen = Set<String>()
    return ((builtin ? contextualWords : []) + words).filter { seen.insert($0.lowercased()).inserted }
}

enum Out {
    private static let lock = NSLock()

    static func send(_ event: [String: Any]) {
        guard let data = try? JSONSerialization.data(withJSONObject: event) else { return }
        var line = data
        line.append(0x0A)
        lock.lock()
        defer { lock.unlock() }
        FileHandle.standardOutput.write(line)
    }
}

enum DictationError: LocalizedError {
    case microphoneDenied
    case noMicrophone
    case noAudioFormat

    var errorDescription: String? {
        switch self {
        case .microphoneDenied: "Allow Nesa to use the microphone in System Settings, Privacy & Security."
        case .noMicrophone: "No microphone input is available."
        case .noAudioFormat: "The speech engine has no audio format on this Mac."
        }
    }
}

func message(_ error: Error) -> String {
    (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
}

/// Joins recognizer segments. Segments usually carry their own leading space, so a space is added
/// only when neither side already has one and the next segment does not start with punctuation.
func joinSegments(_ segments: [(start: Double, text: String)]) -> String {
    segments.reduce(into: "") { joined, segment in
        guard let last = joined.last, let first = segment.text.first else {
            joined += segment.text
            return
        }
        let needsSpace = !last.isWhitespace && !first.isWhitespace && !first.isPunctuation
        joined += needsSpace ? " " + segment.text : segment.text
    }
}

/// Converts microphone buffers to the format the analyzer asked for. The converter is reused so
/// its resampling state carries across buffers.
enum Audio {
    static func convert(_ buffer: AVAudioPCMBuffer, with converter: AVAudioConverter, to format: AVAudioFormat) -> AVAudioPCMBuffer? {
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * format.sampleRate / buffer.format.sampleRate) + 64
        guard let out = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: capacity) else { return nil }
        var supplied = false
        var error: NSError?
        let status = converter.convert(to: out, error: &error) { _, inputStatus in
            if supplied {
                inputStatus.pointee = .noDataNow
                return nil
            }
            supplied = true
            inputStatus.pointee = .haveData
            return buffer
        }
        guard status != .error, error == nil, out.frameLength > 0 else { return nil }
        return out
    }
}

/// Reports the microphone level about 30 times a second, which drives the HUD animation.
final class LevelMeter: @unchecked Sendable {
    private let lock = NSLock()
    private var lastSent: UInt64 = 0

    func observe(_ buffer: AVAudioPCMBuffer) {
        guard let samples = buffer.floatChannelData?[0], buffer.frameLength > 0 else { return }
        let count = Int(buffer.frameLength)
        var sum: Float = 0
        for i in 0..<count { sum += samples[i] * samples[i] }
        let rms = (sum / Float(count)).squareRoot()

        let now = DispatchTime.now().uptimeNanoseconds
        lock.lock()
        let due = now - lastSent >= 33_000_000
        if due { lastSent = now }
        lock.unlock()
        guard due else { return }

        // Speech is quiet, so scale it up; normal talking should fill most of the HUD.
        Out.send(["event": "level", "value": min(1, Double(rms) * 8)])
    }
}

@available(macOS 26.0, *)
actor Dictation {
    private struct Session {
        let engine: AVAudioEngine
        let input: AVAudioInputNode
        let analyzer: SpeechAnalyzer
        let inputs: AsyncStream<AnalyzerInput>.Continuation
        let results: Task<String, Error>
        let analysis: Task<Void, Error>
    }

    private var session: Session?

    // Prepared once at launch, so a hold only has to open the microphone.
    private var locale: Locale?
    private var format: AVAudioFormat?
    private var engine: AVAudioEngine?

    /// Work that does not depend on the user: the language, the audio format, the model, and the
    /// audio engine. Runs before the first command, so the first hold is already fast.
    func prewarm() async {
        let locale = await DictationTranscriber.supportedLocale(equivalentTo: Locale.current)
            ?? Locale(identifier: "en-US")
        self.locale = locale
        let transcriber = DictationTranscriber(locale: locale, preset: .progressiveShortDictation)
        do {
            // The first use on a Mac downloads the language model. Later launches skip this.
            if let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) {
                Out.send(["event": "state", "state": "downloading"])
                try await request.downloadAndInstall()
            }
        } catch {
            Out.send(["event": "error", "message": message(error)])
        }
        format = await SpeechAnalyzer.bestAvailableAudioFormat(compatibleWith: [transcriber])

        // Creating the engine and reading its input is the slow part of setup. Starting it is not.
        let engine = AVAudioEngine()
        _ = engine.inputNode.outputFormat(forBus: 0)
        self.engine = engine
        Out.send(["event": "state", "state": "ready"])
    }

    /// The audio hardware changed (AirPods connected, a different microphone chosen). The cached
    /// engine may now point at the old input, so the next hold builds a fresh one.
    func forgetEngine() {
        engine = nil
    }

    func start(hints: [String]) async {
        guard session == nil else { return }
        do {
            session = try await openSession(hints: hints)
        } catch {
            Out.send(["event": "error", "message": message(error)])
        }
    }

    func stop() async {
        guard let session else {
            Out.send(["event": "final", "text": ""])
            return
        }
        self.session = nil
        session.engine.stop()
        session.input.removeTap(onBus: 0)
        session.inputs.finish()
        do {
            try await session.analyzer.finalizeAndFinishThroughEndOfInput()
            let text = try await session.results.value
            Out.send(["event": "final", "text": text])
        } catch {
            Out.send(["event": "error", "message": message(error)])
        }
        _ = try? await session.analysis.value
    }

    func cancel() async {
        guard let session else {
            Out.send(["event": "cancelled"])
            return
        }
        self.session = nil
        session.engine.stop()
        session.input.removeTap(onBus: 0)
        session.inputs.finish()
        await session.analyzer.cancelAndFinishNow()
        session.results.cancel()
        Out.send(["event": "cancelled"])
    }

    private func openSession(hints: [String]) async throws -> Session {
        if format == nil || engine == nil { await prewarm() }
        guard let format, let engine else { throw DictationError.noAudioFormat }
        guard let locale else { throw DictationError.noAudioFormat }
        let transcriber = DictationTranscriber(locale: locale, preset: .progressiveShortDictation)

        if AVCaptureDevice.authorizationStatus(for: .audio) == .notDetermined {
            _ = await AVCaptureDevice.requestAccess(for: .audio)
        }
        guard AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else {
            throw DictationError.microphoneDenied
        }

        // Open the microphone before the analyzer is ready. Audio waits in the stream until the
        // analyzer starts, so the first word is not lost to setup time.
        let input = engine.inputNode
        let hardware = input.outputFormat(forBus: 0)
        guard hardware.sampleRate > 0, hardware.channelCount > 0,
              let converter = AVAudioConverter(from: hardware, to: format) else {
            throw DictationError.noMicrophone
        }

        let (stream, inputs) = AsyncStream<AnalyzerInput>.makeStream()
        let meter = LevelMeter()
        input.installTap(onBus: 0, bufferSize: 2048, format: hardware) { buffer, _ in
            meter.observe(buffer)
            guard let converted = Audio.convert(buffer, with: converter, to: format) else { return }
            inputs.yield(AnalyzerInput(buffer: converted))
        }
        engine.prepare()
        do {
            try engine.start()
        } catch {
            input.removeTap(onBus: 0)
            inputs.finish()
            throw error
        }
        Out.send(["event": "listening"])

        let analyzer = SpeechAnalyzer(modules: [transcriber])
        let context = AnalysisContext()
        context.contextualStrings[.general] = hints
        let results = Task<String, Error> {
            var segments: [(start: Double, text: String)] = []
            for try await result in transcriber.results {
                let text = String(result.text.characters)
                let start = result.range.start.seconds
                if result.isFinal {
                    // A final that starts where the previous one did repeats it, so it replaces it.
                    if let last = segments.last, start <= last.start {
                        segments[segments.count - 1] = (start, text)
                    } else {
                        segments.append((start, text))
                    }
                    Out.send(["event": "partial", "text": joinSegments(segments)])
                } else {
                    Out.send(["event": "partial", "text": joinSegments(segments + [(start, text)])])
                }
            }
            return joinSegments(segments)
        }
        let analysis: Task<Void, Error>
        do {
            try await analyzer.setContext(context)
            try await analyzer.prepareToAnalyze(in: format)
            analysis = Task { try await analyzer.start(inputSequence: stream) }
        } catch {
            engine.stop()
            input.removeTap(onBus: 0)
            inputs.finish()
            results.cancel()
            throw error
        }
        return Session(engine: engine, input: input, analyzer: analyzer, inputs: inputs, results: results, analysis: analysis)
    }
}

// MARK: - Keys, permissions and paste
//
// The key watch is a listen-only event tap. It reports which modifier keys are down and whether a key
// or a click came in. It never reads which key was pressed and never changes an event. Paste writes the
// words to the clipboard, presses Command-V in the app the user is in, and puts the clipboard back.

/// Tags the keystrokes this helper posts, so the key watch ignores its own paste.
let syntheticMarker: Int64 = 0x4E65_7361

/// The two permissions that dictation outside Nesa needs. The user grants them in System Settings.
enum Access {
    static var inputMonitoring: Bool { CGPreflightListenEventAccess() }
    static var accessibility: Bool { AXIsProcessTrusted() }

    static func report() {
        Out.send(["event": "access", "input": inputMonitoring, "accessibility": accessibility])
    }

    /// Asks for both. macOS shows its prompts, and each permission gains a row in System Settings.
    static func request() {
        _ = CGRequestListenEventAccess()
        let prompt = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        _ = AXIsProcessTrustedWithOptions([prompt: true] as CFDictionary)
        report()
    }
}

/// Watches the modifier keys and input in every app. One tap, created on demand.
enum KeyWatch {
    private static let lock = NSLock()
    private static var tap: CFMachPort?
    private static var starting = false
    private static let kinds: [CGEventType] = [.keyDown, .flagsChanged, .leftMouseDown, .rightMouseDown, .otherMouseDown]

    /// Starts the tap if it is not running. Tries again on each call, so a permission granted later works.
    static func ensure() {
        lock.lock()
        defer { lock.unlock() }
        guard tap == nil, !starting else { return }
        starting = true
        let thread = Thread {
            let mask = kinds.reduce(CGEventMask(0)) { $0 | (CGEventMask(1) << $1.rawValue) }
            let created = CGEvent.tapCreate(
                tap: .cgSessionEventTap, place: .headInsertEventTap, options: .listenOnly,
                eventsOfInterest: mask, callback: keyWatchCallback, userInfo: nil)
            guard let created else {
                lock.lock()
                starting = false
                lock.unlock()
                Out.send(["event": "tap", "state": "unavailable"])
                return
            }
            lock.lock()
            tap = created
            starting = false
            lock.unlock()
            CFRunLoopAddSource(CFRunLoopGetCurrent(), CFMachPortCreateRunLoopSource(nil, created, 0), .commonModes)
            CGEvent.tapEnable(tap: created, enable: true)
            Out.send(["event": "tap", "state": "on"])
            CFRunLoopRun()
        }
        thread.start()
    }

    /// Tells the app whether the tap is running, so it can choose between the tap and the window.
    static func report() {
        lock.lock()
        let running = tap != nil
        lock.unlock()
        Out.send(["event": "tap", "state": running ? "on" : "off"])
    }

    /// Called on the tap's thread for each event it was asked for.
    static func handle(_ type: CGEventType, _ event: CGEvent) {
        switch type {
        case .tapDisabledByTimeout, .tapDisabledByUserInput:
            // The system switches off a slow tap. Turn it back on rather than losing the keys.
            lock.lock()
            let current = tap
            lock.unlock()
            if let current { CGEvent.tapEnable(tap: current, enable: true) }
        case .flagsChanged:
            guard event.getIntegerValueField(.eventSourceUserData) != syntheticMarker else { return }
            let flags = event.flags
            Out.send([
                "event": "flags",
                "alt": flags.contains(.maskAlternate),
                "ctrl": flags.contains(.maskControl),
                "meta": flags.contains(.maskCommand),
                "shift": flags.contains(.maskShift),
            ])
        case .keyDown, .leftMouseDown, .rightMouseDown, .otherMouseDown:
            guard event.getIntegerValueField(.eventSourceUserData) != syntheticMarker else { return }
            Out.send(["event": "input"])
        default:
            return
        }
    }
}

/// The tap's callback. It cannot capture anything, so it hands each event to `KeyWatch`.
let keyWatchCallback: CGEventTapCallBack = { _, type, event, _ in
    KeyWatch.handle(type, event)
    return Unmanaged.passUnretained(event)
}

/// Pastes dictated words into the app the user was in when the hold started.
enum Typing {
    private static let lock = NSLock()
    private static var target: NSRunningApplication?
    private static let queue = DispatchQueue(label: "nesa.typing")
    private static let transient = NSPasteboard.PasteboardType("org.nspasteboard.TransientType")

    /// Remembers the frontmost app, unless it is Nesa itself, and tells the app its name for the pill.
    static func rememberTarget() {
        let front = NSWorkspace.shared.frontmostApplication
        let own = ProcessInfo.processInfo.processIdentifier
        let chosen = front?.processIdentifier == own ? nil : front
        lock.lock()
        target = chosen
        lock.unlock()
        Out.send(["event": "target", "app": chosen?.localizedName ?? ""])
    }

    /// Pastes `text` into the remembered app. Runs off the request loop, so a paste never holds up a hold.
    static func paste(_ text: String, pressReturn: Bool, learn: Bool) {
        queue.async {
            lock.lock()
            let app = target
            lock.unlock()
            guard let app else {
                report(false, "target")
                return
            }
            guard !IsSecureEventInputEnabled() else {
                report(false, "secure")
                return
            }
            guard Access.accessibility else {
                report(false, "accessibility")
                return
            }
            // A pasted Command-V must land in the app the user chose, never in Nesa.
            guard NSWorkspace.shared.frontmostApplication?.processIdentifier == app.processIdentifier else {
                report(false, "moved")
                return
            }
            // The field is read before the paste, so the text the paste adds can be found afterwards.
            let field = learn ? Corrections.snapshot(pid: app.processIdentifier) : nil
            let board = NSPasteboard.general
            let saved = copy(board.pasteboardItems ?? [])
            board.clearContents()
            board.setString(text, forType: .string)
            board.setData(Data(), forType: transient)
            let written = board.changeCount
            keystroke(9, flags: .maskCommand) // V
            report(true, app.localizedName ?? "")
            if let field { Corrections.watch(field, inserted: text) }
            if pressReturn {
                DispatchQueue.global().asyncAfter(deadline: .now() + 0.15) { keystroke(36, flags: []) }
            }
            // Put the clipboard back, unless something was copied since. Some apps read it slowly.
            DispatchQueue.global().asyncAfter(deadline: .now() + 0.9) {
                guard board.changeCount == written else { return }
                board.clearContents()
                board.writeObjects(saved)
            }
        }
    }

    private static func report(_ ok: Bool, _ detail: String) {
        if ok {
            Out.send(["event": "paste", "ok": true, "app": detail])
        } else {
            Out.send(["event": "paste", "ok": false, "reason": detail])
        }
    }

    /// A copy of each clipboard item, so the clipboard can be written back after the paste.
    private static func copy(_ items: [NSPasteboardItem]) -> [NSPasteboardItem] {
        items.map { item in
            let copy = NSPasteboardItem()
            for type in item.types {
                if let data = item.data(forType: type) { copy.setData(data, forType: type) }
            }
            return copy
        }
    }

    /// Presses one key with exactly these modifiers. The events are tagged so the key watch ignores them.
    private static func keystroke(_ key: CGKeyCode, flags: CGEventFlags) {
        let source = CGEventSource(stateID: .privateState)
        for down in [true, false] {
            guard let event = CGEvent(keyboardEventSource: source, virtualKey: key, keyDown: down) else { continue }
            event.flags = flags
            event.setIntegerValueField(.eventSourceUserData, value: syntheticMarker)
            event.post(tap: .cgSessionEventTap)
        }
    }
}

// MARK: - Learning from corrections
//
// After a paste, the text field the words went into is read back. A fix the user makes is reported as the
// words between the saved context either side of the dictation, before and after. Only that stretch is
// read, only while the same field has focus, for at most two minutes. Secure fields are never read.

/// The text fields of other apps, read through Accessibility, for learning from corrections.
enum Corrections {
    static let queue = DispatchQueue(label: "nesa.corrections")
    /// Words of context kept on each side of the dictated text, in UTF-16 units.
    static let contextUnits = 24
    static let trackSeconds: Double = 120
    /// How long the text must stay the same before an edit counts as finished.
    static let settleSeconds: Double = 2.5
    /// A longer stretch is not a correction, so it is not read.
    static let maxRegionUnits = 20_000

    /// A text field before a paste: the element, its text and where the caret was, in UTF-16 units.
    struct Field {
        let element: AXUIElement
        let pid: pid_t
        let text: String
        let caret: Int
        let selected: Int
    }

    /// One dictated text being watched. Its state is changed only on `queue`.
    final class Tracker {
        let element: AXUIElement
        let pid: pid_t
        let left: String
        let right: String
        var baseline: String
        var latest: String
        var changedAt: Date
        let started = Date()
        var misses = 0
        var timer: DispatchSourceTimer?

        init(element: AXUIElement, pid: pid_t, left: String, right: String, inserted: String) {
            self.element = element
            self.pid = pid
            self.left = left
            self.right = right
            baseline = inserted
            latest = inserted
            changedAt = started
        }
    }

    private static var active: Tracker?

    static func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success else { return nil }
        return value as AnyObject?
    }

    /// The focused text field of an app, if it has one that Accessibility can reach.
    static func focusedField(pid: pid_t) -> AXUIElement? {
        let app = AXUIElementCreateApplication(pid)
        AXUIElementSetMessagingTimeout(app, 0.25)
        // Chromium and Electron build their accessibility tree only when asked. Other apps ignore this.
        _ = AXUIElementSetAttributeValue(app, "AXManualAccessibility" as CFString, kCFBooleanTrue)
        guard let focused = attribute(app, kAXFocusedUIElementAttribute as String) else { return nil }
        let element = focused as! AXUIElement
        AXUIElementSetMessagingTimeout(element, 0.25)
        return element
    }

    static func isSecure(_ element: AXUIElement) -> Bool {
        let role = attribute(element, kAXRoleAttribute as String) as? String
        let subrole = attribute(element, kAXSubroleAttribute as String) as? String
        return role == "AXSecureTextField" || subrole == "AXSecureTextField"
    }

    /// The text of a field and its selection, in UTF-16 units.
    static func textState(_ element: AXUIElement) -> (String, CFRange)? {
        guard let value = attribute(element, kAXValueAttribute as String) as? String,
              let rangeValue = attribute(element, kAXSelectedTextRangeAttribute as String),
              CFGetTypeID(rangeValue) == AXValueGetTypeID() else { return nil }
        var range = CFRange()
        guard AXValueGetValue(rangeValue as! AXValue, .cfRange, &range) else { return nil }
        return (value, range)
    }

    /// The field a paste is about to go into, or nil when it is secure or cannot be read.
    static func snapshot(pid: pid_t) -> Field? {
        guard let element = focusedField(pid: pid), !isSecure(element) else { return nil }
        guard let (text, range) = textState(element) else { return nil }
        return Field(element: element, pid: pid, text: text, caret: range.location, selected: range.length)
    }

    /// The text between two UTF-16 offsets, moved inward so that no surrogate pair is cut in half.
    static func slice(_ text: NSString, _ from: Int, _ to: Int) -> String {
        var start = max(0, from)
        var end = min(text.length, to)
        if start < end, (0xDC00...0xDFFF).contains(text.character(at: start)) { start += 1 }
        if start < end, (0xD800...0xDBFF).contains(text.character(at: end - 1)) { end -= 1 }
        guard end > start else { return "" }
        return text.substring(with: NSRange(location: start, length: end - start))
    }

    /// Starts tracking once the paste has landed. Some apps update their field late, so the check is retried.
    static func watch(_ field: Field, inserted: String) {
        let text = field.text as NSString
        let left = slice(text, field.caret - contextUnits, field.caret)
        let rightStart = field.caret + field.selected
        let right = slice(text, rightStart, rightStart + contextUnits)
        DispatchQueue.global().async {
            for _ in 0..<6 {
                Thread.sleep(forTimeInterval: 0.3)
                guard let state = textState(field.element) else { continue }
                let after = state.0 as NSString
                let count = (inserted as NSString).length
                guard field.caret + count <= after.length,
                      after.substring(with: NSRange(location: field.caret, length: count)) == inserted else { continue }
                let tracker = Tracker(element: field.element, pid: field.pid, left: left, right: right, inserted: inserted)
                queue.async { start(tracker) }
                return
            }
        }
    }

    /// The text that sits between the saved context, or nil when the context is no longer there.
    static func region(of value: String, left: String, right: String) -> String? {
        let text = value as NSString
        let length = text.length
        var starts: [Int] = []
        if left.isEmpty {
            starts = [0]
        } else {
            var from = 0
            while from < length {
                let found = text.range(of: left, options: .literal, range: NSRange(location: from, length: length - from))
                if found.location == NSNotFound { break }
                starts.append(found.location + found.length)
                from = found.location + 1
            }
        }
        for start in starts {
            var stop = length
            if !right.isEmpty {
                let found = text.range(of: right, options: .literal, range: NSRange(location: start, length: length - start))
                if found.location == NSNotFound { continue }
                stop = found.location
            }
            guard stop >= start, stop - start <= maxRegionUnits else { continue }
            return text.substring(with: NSRange(location: start, length: stop - start))
        }
        return nil
    }

    private static func start(_ tracker: Tracker) {
        if let previous = active { finish(previous) }
        active = tracker
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + 1, repeating: 1)
        timer.setEventHandler { tick(tracker) }
        tracker.timer = timer
        timer.resume()
    }

    /// Reports an edit that is still pending, then stops watching the field.
    private static func finish(_ tracker: Tracker) {
        if tracker.latest != tracker.baseline {
            Out.send(["event": "edit", "inserted": tracker.baseline, "edited": tracker.latest])
        }
        tracker.timer?.cancel()
        tracker.timer = nil
        if active === tracker { active = nil }
    }

    private static func tick(_ tracker: Tracker) {
        // A secure field, or secure input, is never read, and watching stops there.
        if isSecure(tracker.element) || IsSecureEventInputEnabled() {
            finish(tracker)
            return
        }
        let now = Date()
        let stillFocused = focusedField(pid: tracker.pid).map { CFEqual($0, tracker.element) } ?? false
        let current: String? = textState(tracker.element).flatMap { state in
            Corrections.region(of: state.0, left: tracker.left, right: tracker.right)
        }
        if let current {
            tracker.misses = 0
            if current != tracker.latest {
                tracker.latest = current
                tracker.changedAt = now
            }
        } else {
            tracker.misses += 1
        }
        // An edit counts once the text has been still for a moment, or as soon as the user leaves the field.
        let settled = now.timeIntervalSince(tracker.changedAt) >= settleSeconds
        if (settled || !stillFocused) && tracker.latest != tracker.baseline {
            Out.send(["event": "edit", "inserted": tracker.baseline, "edited": tracker.latest])
            tracker.baseline = tracker.latest
        }
        if !stillFocused || tracker.misses >= 3 || now.timeIntervalSince(tracker.started) > trackSeconds {
            finish(tracker)
        }
    }
}

// MARK: - Parakeet
//
// Parakeet TDT 0.6B (FluidAudio), in batch mode. The audio of a hold is kept. While the user speaks it is
// transcribed again about once a second, so the pill shows the words as they come. On release the whole hold
// is transcribed once more, and that text is typed. Apple's engine serves holds while Parakeet is downloading.
//
// Vocabulary boosting (FluidAudio's CTC pass) is not used: on these models it inserted vocabulary words into
// unrelated speech, so the words list does not bias Parakeet. "Often heard as" fixes and learned corrections
// still apply, because they change the text after recognition.

/// Microphone audio at 16 kHz mono, gathered while a hold is open. The tap appends; the ticker and release read.
final class SampleBuffer: @unchecked Sendable {
    private let lock = NSLock()
    private var samples: [Float] = []

    func append(_ more: [Float]) {
        lock.lock()
        samples += more
        lock.unlock()
    }

    func snapshot() -> [Float] {
        lock.lock()
        defer { lock.unlock() }
        return samples
    }
}

@available(macOS 26.0, *)
actor Parakeet {
    private var asr: AsrManager?
    private(set) var isReady = false
    private var session: Session?

    private struct Session {
        let engine: AVAudioEngine
        let input: AVAudioInputNode
        let samples: SampleBuffer
        let ticker: Task<Void, Never>
    }

    /// How often the words on the pill are refreshed while the user speaks.
    static let partialInterval: UInt64 = 1_000_000_000
    /// Half a second at 16 kHz: less audio than this has no words worth transcribing.
    static let minimumSamples = 8_000

    /// Downloads and loads the model once, at launch. Until it is ready, holds use Apple's engine.
    func prewarm() async {
        do {
            let directory = AsrModels.defaultCacheDirectory(for: .ultra)
            if !AsrModels.modelsExist(at: directory, version: .ultra) {
                Out.send(["event": "state", "state": "downloading"])
            }
            let models = try await AsrModels.downloadAndLoad(version: .ultra, encoderPrecision: .int8)
            let manager = AsrManager(config: .default)
            try await manager.loadModels(models)
            asr = manager
            isReady = true
            Out.send(["event": "engine", "name": "parakeet", "state": "ready"])
        } catch {
            Out.send(["event": "error", "message": "Parakeet could not load: \(message(error))"])
        }
    }

    /// Opens the microphone and starts refreshing the words on the pill.
    func start() async throws {
        guard session == nil else { return }
        guard let asr else { throw DictationError.noAudioFormat }
        if AVCaptureDevice.authorizationStatus(for: .audio) == .notDetermined {
            _ = await AVCaptureDevice.requestAccess(for: .audio)
        }
        guard AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else {
            throw DictationError.microphoneDenied
        }

        // A new engine per hold, so a change of microphone is picked up without a special case.
        let engine = AVAudioEngine()
        let input = engine.inputNode
        let hardware = input.outputFormat(forBus: 0)
        guard hardware.sampleRate > 0, hardware.channelCount > 0 else { throw DictationError.noMicrophone }

        let converter = AudioConverter()
        let samples = SampleBuffer()
        let meter = LevelMeter()
        input.installTap(onBus: 0, bufferSize: 2048, format: hardware) { buffer, _ in
            meter.observe(buffer)
            if let chunk = try? converter.resampleBuffer(buffer) {
                samples.append(chunk)
            }
        }
        engine.prepare()
        do {
            try engine.start()
        } catch {
            input.removeTap(onBus: 0)
            throw error
        }

        let ticker = Task {
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: Parakeet.partialInterval)
                guard !Task.isCancelled else { break }
                let audio = samples.snapshot()
                guard audio.count >= Parakeet.minimumSamples else { continue }
                var state = TdtDecoderState.make()
                let result = try? await asr.transcribe(audio, decoderState: &state)
                // A result that lands after release must not overwrite the final text on the pill.
                guard !Task.isCancelled, let result else { continue }
                Out.send(["event": "partial", "text": result.text])
            }
        }
        Out.send(["event": "listening"])
        session = Session(engine: engine, input: input, samples: samples, ticker: ticker)
    }

    /// Stops the microphone and transcribes the whole hold. The returned text is what gets typed.
    func stop() async throws -> String {
        guard let session else { return "" }
        self.session = nil
        session.engine.stop()
        session.input.removeTap(onBus: 0)
        session.ticker.cancel()
        let audio = session.samples.snapshot()
        guard audio.count >= Parakeet.minimumSamples, let asr else { return "" }
        var state = TdtDecoderState.make()
        return try await asr.transcribe(audio, decoderState: &state).text
    }

    func cancel() {
        guard let session else { return }
        self.session = nil
        session.engine.stop()
        session.input.removeTap(onBus: 0)
        session.ticker.cancel()
    }
}

/// One line from the app: a command, and for `start`, the vocabulary to expect during that hold.
struct Request {
    let cmd: String
    let words: [String]
    let builtin: Bool
    /// "parakeet" or "apple". Only `start` uses it.
    let engine: String
}

let (requests, requestInput) = AsyncStream<Request>.makeStream()

// stdin is read on its own thread. When the app closes the pipe, the helper exits with it.
Thread.detachNewThread {
    while let line = readLine() {
        guard let data = line.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let cmd = object["cmd"] as? String else { continue }
        // Keys and paste are handled here, not in the speech queue, so a slow speech start cannot delay them.
        switch cmd {
        case "watch":
            KeyWatch.ensure()
            KeyWatch.report()
            Access.report()
        case "request_access":
            Access.request()
        case "target":
            Typing.rememberTarget()
        case "paste":
            Typing.paste(object["text"] as? String ?? "", pressReturn: object["return"] as? Bool ?? false,
                learn: object["learn"] as? Bool ?? false)
        default:
            let words = object["words"] as? [String] ?? []
            let builtin = object["builtin"] as? Bool ?? true
            let engine = object["engine"] as? String ?? "parakeet"
            requestInput.yield(Request(cmd: cmd, words: words, builtin: builtin, engine: engine))
        }
    }
    requestInput.finish()
    exit(0)
}

Task {
    if #available(macOS 26.0, *) {
        let dictation = Dictation()
        let parakeet = Parakeet()
        NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange, object: nil, queue: nil) { _ in
            Task { await dictation.forgetEngine() }
        }
        // Parakeet loads in the background, so Apple's engine can serve holds until it is ready.
        Task { await parakeet.prewarm() }
        await dictation.prewarm()
        Out.send(["event": "ready"])
        // Which engine the current hold runs on. Stop and cancel follow it, so a hold always ends where it began.
        var holdOnParakeet = false
        // Requests run one at a time, so a stop that arrives during start waits for it.
        for await request in requests {
            switch request.cmd {
            case "start":
                let hints = contextualHints(request.words, builtin: request.builtin)
                if request.engine == "parakeet", await parakeet.isReady {
                    holdOnParakeet = true
                    do { try await parakeet.start() } catch { Out.send(["event": "error", "message": message(error)]) }
                } else {
                    holdOnParakeet = false
                    if request.engine == "parakeet" { Out.send(["event": "engine", "used": "apple"]) }
                    await dictation.start(hints: hints)
                }
            case "stop":
                if holdOnParakeet {
                    do { Out.send(["event": "final", "text": try await parakeet.stop()]) } catch { Out.send(["event": "error", "message": message(error)]) }
                } else {
                    await dictation.stop()
                }
            case "cancel":
                if holdOnParakeet {
                    await parakeet.cancel()
                    Out.send(["event": "cancelled"])
                } else {
                    await dictation.cancel()
                }
            default: Out.send(["event": "error", "message": "Unknown command \(request.cmd)"])
            }
        }
    } else {
        Out.send(["event": "ready"])
        for await _ in requests {
            Out.send(["event": "error", "message": "Dictation needs macOS 26 or later."])
        }
    }
    exit(0)
}

dispatchMain()
