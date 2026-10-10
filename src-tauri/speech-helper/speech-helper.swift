// speech-helper: the dictation sidecar for Nesa.
//
// Records from the microphone while the app asks, streams the audio through Apple's on-device
// speech engine (SpeechAnalyzer with DictationTranscriber) and reports results as JSON lines.
// It runs as a separate process so a speech failure can never take down the terminal sessions.
//
//   stdin   {"cmd":"start","words":["Kubernetes"],"builtin":true} | {"cmd":"stop"} | {"cmd":"cancel"}
//           words are the user's vocabulary for this hold. builtin adds contextualWords when true.
//   stdout  {"event":"ready"}
//           {"event":"state","state":"downloading"}
//           {"event":"listening"}
//           {"event":"level","value":0...1}
//           {"event":"partial","text":"..."}
//           {"event":"final","text":"..."}     once per stop; empty when nothing was heard
//           {"event":"cancelled"}
//           {"event":"error","message":"..."}

import AVFAudio
import AVFoundation
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

/// One line from the app: a command, and for `start`, the vocabulary to expect during that hold.
struct Request {
    let cmd: String
    let words: [String]
    let builtin: Bool
}

let (requests, requestInput) = AsyncStream<Request>.makeStream()

// stdin is read on its own thread. When the app closes the pipe, the helper exits with it.
Thread.detachNewThread {
    while let line = readLine() {
        guard let data = line.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let cmd = object["cmd"] as? String else { continue }
        let words = object["words"] as? [String] ?? []
        let builtin = object["builtin"] as? Bool ?? true
        requestInput.yield(Request(cmd: cmd, words: words, builtin: builtin))
    }
    requestInput.finish()
    exit(0)
}

Task {
    if #available(macOS 26.0, *) {
        let dictation = Dictation()
        NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange, object: nil, queue: nil) { _ in
            Task { await dictation.forgetEngine() }
        }
        await dictation.prewarm()
        Out.send(["event": "ready"])
        // Requests run one at a time, so a stop that arrives during start waits for it.
        for await request in requests {
            switch request.cmd {
            case "start": await dictation.start(hints: contextualHints(request.words, builtin: request.builtin))
            case "stop": await dictation.stop()
            case "cancel": await dictation.cancel()
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
