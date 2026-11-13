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
