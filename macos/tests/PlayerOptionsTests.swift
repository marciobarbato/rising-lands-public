import Foundation

// Standalone headless executable. Compile ONLY with NativePlayerOptions.swift.
@main
struct PlayerOptionsTests {
    static func expect(_ condition: Bool, _ label: String) {
        if !condition { FileHandle.standardError.write(Data("PlayerOptionsTests FAILED: \(label)\n".utf8)); exit(1) }
    }
    static func main() {
        let suite = "PlayerOptionsTests-\(ProcessInfo.processInfo.processIdentifier)"
        let defaults = UserDefaults(suiteName: suite)!
        defer { defaults.removePersistentDomain(forName: suite) }
        expect(NativeDifficulty.allCases.map(\.rawValue) == [0, 1, 2], "values are RL_DIFICULDADE_*")
        expect(NativeDifficulty.allCases.map(\.title) == ["Fácil", "Normal", "Difícil"], "PT-BR titles")
        expect(NativeDifficulty.load(defaults) == .normal, "Normal, the original game, by default")
        NativeDifficulty.hard.store(defaults)
        expect(NativeDifficulty.load(defaults) == .hard, "stored preset survives")
        defaults.set(9, forKey: NativeDifficulty.preferenceKey)
        expect(NativeDifficulty.load(defaults) == .normal, "unknown value falls back to Normal")
        defaults.set(-1, forKey: NativeDifficulty.preferenceKey)
        expect(NativeDifficulty.load(defaults) == .normal, "negative value falls back to Normal")
        expect(NativeDisplayOptions.load(defaults) == NativeDisplayOptions(), "fit and sharp pixels by default")
        let chosen = NativeDisplayOptions(integerScale: true, smoothFilter: true)
        chosen.store(defaults)
        expect(NativeDisplayOptions.load(defaults) == chosen, "display options survive")
        expect(NativeAudioSettings.load(defaults) == NativeAudioSettings(), "full volume, nothing muted by default")
        expect(NativeAudioSettings().effectsGain == 1 && NativeAudioSettings().musicGain == 1, "default gains")
        expect(NativeAudioSettings.levels.map(NativeAudioSettings.title) == ["100%", "75%", "50%", "25%"], "level titles")
        expect(NativeAudioSettings.level(tag: 50) == 0.5 && NativeAudioSettings.level(tag: 25) == 0.25 && NativeAudioSettings.level(tag: 60) == nil, "menu tags")
        let quiet = NativeAudioSettings(effectsVolume: 0.25, effectsMuted: true, musicVolume: 0.5, musicMuted: false)
        quiet.store(defaults)
        expect(NativeAudioSettings.load(defaults) == quiet, "audio levels and mutes survive")
        expect(quiet.effectsGain == 0 && quiet.musicGain == 0.5, "mute silences, volume keeps its level")
        var unmuted = quiet; unmuted.effectsMuted = false
        expect(unmuted.effectsGain == 0.25, "unmuting restores the chosen level")
        defaults.set(7.5, forKey: NativeAudioSettings.musicVolumeKey)
        defaults.set(-1, forKey: NativeAudioSettings.effectsVolumeKey)
        let repaired = NativeAudioSettings.load(defaults)
        expect(repaired.musicVolume == 1 && repaired.effectsVolume == 1 && repaired.effectsMuted, "out-of-range volume falls back to full, mute kept")
        autosave(defaults)
        print("PlayerOptionsTests passed")
    }

    /// Periodic autosave: the option and its schedule.
    static func autosave(_ defaults: UserDefaults) {
        typealias Interval = NativeAutosaveInterval
        expect(Interval.allCases.map(\.minutes) == [0, 5, 10, 15], "off, 5, 10 and 15 minutes")
        expect(Interval.allCases.map(\.title) == ["Desligado", "A cada 5 minutos", "A cada 10 minutos", "A cada 15 minutos"], "PT-BR titles")
        expect(Interval.load(defaults) == .off, "off by default, as in the original")
        Interval.ten.store(defaults)
        expect(Interval.load(defaults) == .ten, "stored interval survives")
        defaults.set(7, forKey: Interval.preferenceKey)
        expect(Interval.load(defaults) == .off, "an unknown interval falls back to off")
        expect(Interval.slot == "autosave" && !["rapido", "inicio-fase"].contains(Interval.slot) && !Interval.slot.hasPrefix("posicao-"), "own slot, never a player slot")
        expect(Interval.framesPerGameMinute == 30 * 60, "a game minute is 60 s at the 1x pace (30 frames/s)")
        expect(Interval.five.frames == 9000 && Interval.fifteen.frames == 27000 && Interval.off.frames == 0, "interval in game frames")

        var off = NativeAutosaveSchedule(.off)
        expect(!off.enabled && (0..<100_000).allSatisfy { _ in !off.frame(eligible: true) }, "off never fires")

        var schedule = NativeAutosaveSchedule(intervalFrames: 10)
        expect((0..<9).allSatisfy { _ in !schedule.frame(eligible: true) }, "not before the interval")
        expect((0..<50).allSatisfy { _ in !schedule.frame(eligible: false) }, "menus, pause, events and stops do not count")
        expect(schedule.elapsed == 9, "ineligible frames add nothing")
        expect(schedule.frame(eligible: true), "due on the interval's last eligible frame")
        expect(schedule.frame(eligible: true) && schedule.elapsed == 10, "a busy boundary stays due without counting further")
        expect(!schedule.frame(eligible: false), "never at an ineligible frame, even when due")
        schedule.saved()
        expect(!schedule.due && schedule.elapsed == 0, "a save restarts the interval")
        expect((0..<9).allSatisfy { _ in !schedule.frame(eligible: true) } && schedule.frame(eligible: true), "the next one comes one interval later")
        schedule.failed()
        expect(!schedule.due && (0..<9).allSatisfy { _ in !schedule.frame(eligible: true) }, "a failure waits a whole interval, no retry every frame")
        schedule.restart()
        expect(schedule.elapsed == 0, "a stage start or load restarts the count")

        // Speed only changes how many frames run per second: the count is in frames.
        var fast = NativeAutosaveSchedule(.five)
        var saves = [Int]()
        for frame in 1...(3 * 9000) where fast.frame(eligible: true) { saves.append(frame); fast.saved() }
        expect(saves == [9000, 18000, 27000], "every 5 game minutes = 9000 frames, at any speed")

        var change = NativeAutosaveSchedule(.fifteen)
        for _ in 0..<12000 { _ = change.frame(eligible: true) }
        change.setInterval(frames: Interval.ten.frames)
        expect(change.elapsed == 12000 && !change.due, "a longer-than-played interval keeps the frames played")
        change.setInterval(frames: Interval.five.frames)
        expect(change.elapsed == 9000 && change.frame(eligible: true), "shortening below the frames played makes it due at the next frame")
        change.setInterval(frames: 0)
        expect(!change.enabled && change.elapsed == 0 && !change.frame(eligible: true), "turning it off forgets the count")
        change.setInterval(frames: Interval.five.frames)
        expect((0..<8999).allSatisfy { _ in !change.frame(eligible: true) } && change.frame(eligible: true), "turning it on starts a full interval")
    }
}
