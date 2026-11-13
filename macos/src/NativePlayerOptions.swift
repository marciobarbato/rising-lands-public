import Foundation

/// Stage-start difficulty preset (RL_DIFICULDADE_*). Normal is the original
/// game; Easy and Hard are presets of this port built from the original's own
/// knobs. The engine applies it inside the stage load, so a change made during
/// a stage takes effect at the next New game, Restart or Next stage.
enum NativeDifficulty: UInt32, CaseIterable, Sendable {
    case easy, normal, hard

    static let preferenceKey = "RisingLandsDifficulty"
    var title: String { ["Fácil", "Normal", "Difícil"][Int(rawValue)] }

    static func load(_ defaults: UserDefaults = .standard) -> NativeDifficulty {
        guard defaults.object(forKey: preferenceKey) != nil,
              let value = UInt32(exactly: defaults.integer(forKey: preferenceKey)) else { return .normal }
        return NativeDifficulty(rawValue: value) ?? .normal
    }
    func store(_ defaults: UserDefaults = .standard) { defaults.set(Int(rawValue), forKey: Self.preferenceKey) }
}

/// Window presentation choices. Host only: never reach Rust, saves or replay.
struct NativeDisplayOptions: Equatable, Sendable {
    static let integerKey = "RisingLandsIntegerScale"
    static let smoothKey = "RisingLandsSmoothFilter"
    var integerScale = false
    var smoothFilter = false

    static func load(_ defaults: UserDefaults = .standard) -> NativeDisplayOptions {
        NativeDisplayOptions(integerScale: defaults.bool(forKey: integerKey), smoothFilter: defaults.bool(forKey: smoothKey))
    }
    func store(_ defaults: UserDefaults = .standard) {
        defaults.set(integerScale, forKey: Self.integerKey)
        defaults.set(smoothFilter, forKey: Self.smoothKey)
    }
}

/// Sound effects and CD music levels (menu Áudio). Host only: applied to the output
/// devices (the effects mixer and the music stream), never to the engine, saves or
/// replay. A stored value outside 0...1 falls back to full volume.
struct NativeAudioSettings: Equatable, Sendable {
    static let levels: [Float] = [1, 0.75, 0.5, 0.25]
    static let effectsVolumeKey = "RisingLandsEffectsVolume"
    static let effectsMutedKey = "RisingLandsEffectsMuted"
    static let musicVolumeKey = "RisingLandsMusicVolume"
    static let musicMutedKey = "RisingLandsMusicMuted"
    var effectsVolume: Float = 1
    var effectsMuted = false
    var musicVolume: Float = 1
    var musicMuted = false
    var effectsGain: Float { effectsMuted ? 0 : effectsVolume }
    var musicGain: Float { musicMuted ? 0 : musicVolume }

    static func title(_ level: Float) -> String { "\(Int((level * 100).rounded()))%" }
    /// Menu tags are whole percentages.
    static func level(tag: Int) -> Float? { levels.first { Int(($0 * 100).rounded()) == tag } }

    static func load(_ defaults: UserDefaults = .standard) -> NativeAudioSettings {
        func volume(_ key: String) -> Float {
            guard defaults.object(forKey: key) != nil else { return 1 }
            let value = defaults.float(forKey: key)
            return value.isFinite && (0...1).contains(value) ? value : 1
        }
        return NativeAudioSettings(effectsVolume: volume(effectsVolumeKey), effectsMuted: defaults.bool(forKey: effectsMutedKey),
                                   musicVolume: volume(musicVolumeKey), musicMuted: defaults.bool(forKey: musicMutedKey))
    }
    func store(_ defaults: UserDefaults = .standard) {
        defaults.set(effectsVolume, forKey: Self.effectsVolumeKey)
        defaults.set(effectsMuted, forKey: Self.effectsMutedKey)
        defaults.set(musicVolume, forKey: Self.musicVolumeKey)
        defaults.set(musicMuted, forKey: Self.musicMutedKey)
    }
}

/// Periodic autosave (menu Jogo → Salvamento automático). Host only: it writes the
/// ordinary save of the game in progress to its own slot (`autosave.rls` + `.view`),
/// never to the player's slots or the stage-start autosave. Off by default, as in
/// the original game, which has no periodic autosave.
enum NativeAutosaveInterval: Int, CaseIterable, Sendable {
    case off = 0, five = 5, ten = 10, fifteen = 15
    static let preferenceKey = "RisingLandsAutosaveMinutes"
    /// The slot name; the file is `<saveDirectory>/autosave.rls` beside its `.view`.
    static let slot = "autosave"
    /// Game frames in one minute of game time: 60 s at the 1x pace of 30 frames/s
    /// (NativeGameSpeed.normalFrameRate). Counting frames makes the interval follow
    /// the chosen speed: at 2x a 10-minute interval passes in 5 wall-clock minutes.
    static let framesPerGameMinute: UInt64 = 1800

    var minutes: Int { rawValue }
    var frames: UInt64 { UInt64(rawValue) * Self.framesPerGameMinute }
    var title: String { self == .off ? "Desligado" : "A cada \(rawValue) minutos" }

    static func load(_ defaults: UserDefaults = .standard) -> NativeAutosaveInterval {
        guard defaults.object(forKey: preferenceKey) != nil,
              let value = NativeAutosaveInterval(rawValue: defaults.integer(forKey: preferenceKey)) else { return .off }
        return value
    }
    func store(_ defaults: UserDefaults = .standard) { defaults.set(rawValue, forKey: Self.preferenceKey) }
}

/// When the periodic autosave is due. Pure bookkeeping, owned by the game worker:
/// `frame(eligible:)` is called once per completed game frame and counts only the
/// frames of the game in progress (not menus, briefing, pause, an event FLC or a
/// stopped engine; paused games run no frames at all). Once `intervalFrames`
/// eligible frames have passed the save is due; it stays due, without counting
/// further, until `saved()` (success) or `failed()` (an error: wait one more full
/// interval instead of retrying every frame). A busy boundary (a click still queued
/// for the next frame) simply retries at the next eligible frame. `restart()` runs
/// at every stage start and load, so a fresh game never autosaves at once.
struct NativeAutosaveSchedule: Equatable, Sendable {
    private(set) var intervalFrames: UInt64
    private(set) var elapsed: UInt64 = 0

    init(intervalFrames: UInt64) { self.intervalFrames = intervalFrames }
    init(_ interval: NativeAutosaveInterval) { self.init(intervalFrames: interval.frames) }

    var enabled: Bool { intervalFrames > 0 }
    var due: Bool { enabled && elapsed >= intervalFrames }

    /// One completed game frame. Returns true when the autosave should be attempted
    /// at this frame boundary.
    mutating func frame(eligible: Bool) -> Bool {
        guard enabled, eligible else { return false }
        if elapsed < intervalFrames { elapsed += 1 }
        return due
    }
    /// A new interval keeps the frames already played, so shortening it may make
    /// the save due at the next eligible frame; turning it off forgets them.
    mutating func setInterval(frames: UInt64) {
        intervalFrames = frames
        if frames == 0 { elapsed = 0 } else { elapsed = min(elapsed, frames) }
    }
    mutating func saved() { elapsed = 0 }
    mutating func failed() { elapsed = 0 }
    mutating func restart() { elapsed = 0 }
}
