import Foundation

/// Host side of lane B's in-game FLC events (event-flc.md, steps 2-5): after each
/// play frame, poll the engine's event state; while an event runs, overlay the
/// decoded FLC frame centred on the 800×600 indexed view with the FLC palette,
/// advance FLC frames by the app clock at the file's speed, call terminar once
/// after the last frame has been shown for its delay, and give the view back to
/// the engine (and its game palette) when the event ends. Worker queue only.
final class NativeEventFLCPlayback {
    let folder: URL
    private(set) var flc: NativeFLC?
    private(set) var index: UInt32 = .max
    private(set) var finished = false
    private(set) var failure: String?
    private(set) var started = 0.0
    private(set) var terminations = 0
    var surface = [UInt8](repeating: 0, count: 480_000)

    init(folder: URL) { self.folder = folder }

    /// Returns true when `surface`/`flc.palette` replace the engine's buffers this frame.
    /// `tick` runs on each display step after the opening one (the original latch
    /// 0x4e1b14 is clear) while the FLC counter, once advanced, is still below its
    /// frame count, before the frame is drawn: EvenementAffiche's loop sound post.
    func update(state: MacOSEventFlcState, now: Double, name: (UInt32) -> String?, enginePixels: UnsafePointer<UInt8>,
                finish: () -> Int32, tick: () -> Void = {}) -> Bool {
        guard state.running != 0 else { flc = nil; index = .max; finished = false; return false }
        if finished { return false }
        let opening = flc == nil || index != state.index
        if opening {
            index = state.index; started = now; failure = nil
            guard let stem = name(state.index) else { failure = "Evento FLC \(state.index) sem nome"; finished = true; return false }
            do {
                let opened = try NativeFLC(contentsOf: folder.appendingPathComponent(stem + ".FLC"))
                guard opened.frames == Int(state.frames), opened.width == Int(state.width), opened.height == Int(state.height) else {
                    failure = "Evento \(stem): cabeçalho diferente do declarado"; finished = true; return false
                }
                try opened.next()
                flc = opened
            } catch { failure = "Evento \(stem): \(error)"; finished = true; return false }
        }
        guard let flc = flc else { return false }
        let delay = max(1, flc.delayMS)
        let target = min(flc.frames, 1 + Int((now - started) * 1000 / delay))
        do { while flc.shown < target { try flc.next() } }
        catch { failure = "Evento FLC: \(error)"; finished = true; return false }
        if !opening && flc.shown < flc.frames { tick() }
        if flc.shown >= flc.frames && state.modal != 0 && (now - started) * 1000 >= Double(flc.frames) * delay {
            // The last frame has been shown for its delay: the original player's end.
            terminations += 1
            if finish() != 0 { failure = "rl_evento_flc_terminar_v1 recusado" }
            finished = true
            return false
        }
        surface.withUnsafeMutableBufferPointer { $0.baseAddress!.update(from: enginePixels, count: 480_000) }
        flc.composite(into: &surface)
        return true
    }
}
