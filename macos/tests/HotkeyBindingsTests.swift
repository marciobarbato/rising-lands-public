import Foundation

/// Standalone executable: compile only this file and NativeHotkeyBindings.swift.
@main
struct HotkeyBindingsTests {
    typealias Bindings = NativeHotkeyBindings
    typealias Action = Bindings.Action
    typealias Chord = Bindings.Chord
    typealias Modifiers = Bindings.Modifiers
    typealias BindingError = Bindings.BindingError

    struct Failure: Error { let message: String }

    static func check(_ condition: @autoclosure () throws -> Bool, _ message: String,
                      line: UInt = #line) throws {
        guard try condition() else { throw Failure(message: "Line \(line): \(message)") }
    }

    static func rejects(_ expected: BindingError, _ operation: () throws -> Void,
                        line: UInt = #line) throws {
        do { try operation() }
        catch let error as BindingError {
            try check(error == expected, "Expected \(expected), got \(error)", line: line)
            try check(!(error.errorDescription ?? "").isEmpty, "Missing PT-BR error", line: line)
            return
        }
        throw Failure(message: "Line \(line): expected \(expected)")
    }

    static func defaults() throws {
        let bindings = Bindings()
        // Behavioral expectations from the committed input table, not file-byte pins.
        let expected: [(Action, Int, [UInt16])] = [
            (.enter, 13, [36, 76]), (.escape, 27, [53]), (.space, 32, [49]),
            (.left, 37, [123]), (.up, 38, [126]), (.right, 39, [124]), (.down, 40, [125]),
            (.delete, 46, [117]), (.x, 88, [7]), (.tab, 9, [48]), (.digit0, 48, [29]), (.digit1, 49, [18]),
            (.digit2, 50, [19]), (.digit3, 51, [20]), (.digit4, 52, [21]), (.digit5, 53, [23]),
            (.digit6, 54, [22]), (.digit7, 55, [26]), (.digit8, 56, [28]), (.digit9, 57, [25])
        ]
        try check(expected.count + Action.modern.count == Action.allCases.count, "Unexpected default actions")
        // "Atalhos modernos": no channel of their own, exact AoE2-style defaults on keys the original never reads.
        for (action, code): (Action, UInt16) in [(.lastAlert, 115), (.nextIdleWorker, 47), (.nextIdleMilitary, 43), (.mainStructure, 4)] {
            try check(action.isModern && action.virtualKey == nil && Action.modern.contains(action), "Modern action emits a channel: \(action)")
            try check(bindings.chords(for: action).map(\.keyCode) == [code], "Modern default missing: \(action)")
            try check(action.labelPTBR.contains("moderno"), "Modern label not marked in PT-BR: \(action)")
            try check(bindings.action(keyCode: code) == action, "Modern default unresolved: \(action)")
            for bits: UInt8 in 1...7 {
                try check(bindings.action(keyCode: code, modifiers: Modifiers(rawValue: bits)) == nil, "Modern default not exact: \(action)/\(bits)")
            }
            try check(bindings.action(keyCode: code, modifiers: .command) == nil, "Command leaked to a modern action")
        }
        for (action, virtualKey, codes) in expected {
            try check(action.virtualKey == virtualKey, "Incorrect virtual key for \(action)")
            try check(bindings.chords(for: action).map(\.keyCode) == codes, "Missing aliases for \(action)")
            try check(!action.labelPTBR.isEmpty, "Missing action label")
            for code in codes {
                for bits: UInt8 in 0...7 {
                    try check(bindings.action(keyCode: code, modifiers: Modifiers(rawValue: bits)) == action,
                              "Default lost held modifier semantics: \(action)/\(bits)")
                }
                try check(bindings.action(keyCode: code, modifiers: .command) == nil, "Command leaked to game")
            }
        }
        // F11 (VK 0x7a) is the original's 800x600 <-> 640x480 switch, WndProc 0x418488. The renderer composes only
        // 800x600, so the host contract (docs/HANDOVER_20261109_CLASSES.md) is that the app never delivers it: no
        // action emits VK 0x7a, and the physical F11 key (103) is unbound. The live driver rejects VK 0x7a as well.
        try check(!Action.allCases.contains { $0.virtualKey == 0x7a }, "An action emits F11 (VK 0x7a)")
        try check(bindings.action(keyCode: 103) == nil, "Physical F11 bound by default")
        try check(bindings.action(keyCode: 0) == nil, "Invented A default")
        try check(bindings.action(keyCode: 51) == nil, "Backspace substituted for forward Delete")
        try check(bindings.action(keyCode: 7, modifiers: Modifiers(rawValue: 16)) == nil, "Unknown modifier matched")
        try check(bindings.action(keyCode: 65535) == nil, "Invalid event matched")
    }

    static func rebindingAndConflicts() throws {
        var bindings = Bindings()
        let original = bindings
        try check(bindings.conflicts(for: Chord(keyCode: 123, modifiers: .shift)) == [.left],
                  "Permissive default overlap not reported")
        try check(bindings.conflicts(for: Chord(keyCode: 123), excluding: .left).isEmpty,
                  "Exclusion failed")
        try rejects(.conflict(.left, .x)) { try bindings.rebind(.x, to: [Chord(keyCode: 123, modifiers: .shift)]) }
        try check(bindings == original, "Conflict partially changed memory")
        let controlR = Chord(keyCode: 15, modifiers: .control)
        try bindings.rebind(.x, to: [controlR])
        try check(bindings.action(keyCode: 7) == nil, "Old binding retained")
        try check(bindings.action(keyCode: 15, modifiers: .control) == .x, "Rebound action missing")
        try check(bindings.action(keyCode: 15) == nil, "Required modifier ignored")
        try check(bindings.action(keyCode: 15, modifiers: [.shift, .control]) == nil, "Exact chord accepted extra flags")
        try check(bindings.conflicts(for: controlR) == [.x], "Exact conflict missing")
        try bindings.rebind(.enter, to: [])
        try check(bindings.action(keyCode: 36) == nil && bindings.action(keyCode: 76) == nil, "Alias not unbound")
        try bindings.rebind(.enter, to: [Chord(keyCode: 15, modifiers: .shift)])
        try check(bindings.action(keyCode: 15, modifiers: .shift) == .enter, "Disjoint modifier chord rejected")
        let before = bindings
        try rejects(.conflict(.enter, .enter)) {
            try bindings.rebind(.enter, to: [Chord(keyCode: 15), Chord(keyCode: 15)])
        }
        try check(bindings == before, "Duplicate alias partially changed memory")

        var swap = Dictionary(uniqueKeysWithValues: Action.allCases.map { ($0, bindings.chords(for: $0)) })
        swap[.left] = bindings.chords(for: .right)
        swap[.right] = bindings.chords(for: .left)
        try bindings.replaceBindings(swap)
        try check(bindings.action(keyCode: 124) == .left && bindings.action(keyCode: 123) == .right,
                  "Transactional swap failed")
        let swapped = bindings
        swap.removeValue(forKey: .escape)
        try rejects(.incompleteActions) { try bindings.replaceBindings(swap) }
        try check(bindings == swapped, "Incomplete transaction changed memory")

        var permissive = Bindings()
        try permissive.rebind(.x, to: [Chord(keyCode: 15, modifiers: .control,
                                          allowsAdditionalGameplayModifiers: true)])
        try check(permissive.action(keyCode: 15, modifiers: [.control, .option]) == .x,
                  "Permissive chord lost additional gameplay modifier")
        try check(permissive.action(keyCode: 15, modifiers: .option) == nil,
                  "Permissive chord ignored required modifier")
        try check(permissive.conflicts(for: Chord(keyCode: 15, modifiers: [.shift, .control])) == [.x],
                  "Exact candidate missed permissive existing binding")
        try check(permissive.conflicts(for: Chord(keyCode: 15, modifiers: .option)).isEmpty,
                  "Disjoint exact candidate falsely conflicted")
        try permissive.rebind(.x, to: [Chord(keyCode: 15, modifiers: [.shift, .control])])
        try check(permissive.conflicts(for: Chord(keyCode: 15, modifiers: .control,
                                                allowsAdditionalGameplayModifiers: true)) == [.x],
                  "Permissive candidate missed exact existing binding")
    }

    static func menuAndInvalidBindings() throws {
        var bindings = Bindings()
        let original = bindings
        // Physical US positions for the committed character menu equivalents;
        // the component reserves *all* Command chords, covering other layouts too.
        for code: UInt16 in [12, 8, 9, 0, 18, 19, 46, 13, 15] {
            for modifiers: Modifiers in [.command, [.shift, .command], [.control, .option, .command]] {
                try rejects(.reservedForMenu) { try bindings.rebind(.x, to: [Chord(keyCode: code, modifiers: modifiers)]) }
                try check(bindings.action(keyCode: code, modifiers: modifiers) == nil, "Menu chord resolved")
            }
        }
        try rejects(.invalidModifiers) {
            try bindings.rebind(.x, to: [Chord(keyCode: 15, modifiers: Modifiers(rawValue: 16))])
        }
        for code: UInt16 in [54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 128, 65535] {
            try rejects(.invalidKeyCode(code)) { try bindings.rebind(.x, to: [Chord(keyCode: code)]) }
        }
        try check(bindings == original, "Rejected input changed memory")
    }

    static func roundTripAndReset(in directory: URL) throws {
        let url = directory.appendingPathComponent("round-trip.json")
        var bindings = Bindings()
        try bindings.rebind(.enter, to: [Chord(keyCode: 15, modifiers: .option), Chord(keyCode: 17, modifiers: .control)])
        try bindings.rebind(.escape, to: [])
        try bindings.save(to: url)
        let saved = try Data(contentsOf: url)
        var loaded = Bindings()
        try loaded.load(from: url)
        try check(loaded == bindings, "Round trip changed bindings")
        try check(loaded.chords(for: .escape).isEmpty, "Explicit unbind was lost")
        try loaded.save(to: url)
        try check(Data(contentsOf: url) == saved, "Encoding is not deterministic")
        loaded.resetToDefaults()
        try check(loaded == Bindings(), "Reset did not restore defaults")
        try check(Data(contentsOf: url) == saved, "Reset wrote to disk implicitly")
        try loaded.save(to: url)
        try check(Data(contentsOf: url) != saved, "Atomic overwrite did not replace file")
        try bindings.load(from: url)
        try check(bindings == Bindings(), "Persisted reset did not round trip")
    }

    static func invalidFiles(in directory: URL) throws {
        let url = directory.appendingPathComponent("invalid.json")
        var bindings = Bindings()
        try bindings.rebind(.x, to: [Chord(keyCode: 15, modifiers: .control)])
        let before = bindings
        let cases: [(String, BindingError)] = [
            ("", .invalidFile), ("{", .invalidFile), ("[]", .invalidFile),
            ("{\"version\":1,\"bindings\":[", .invalidFile),
            ("{\"version\":1}", .invalidFile), ("{\"version\":\"1\",\"bindings\":[]}", .invalidFile),
            ("{\"bindings\":[]}", .invalidFile), ("{\"version\":1,\"bindings\":[]}", .incompleteActions),
            ("{\"version\":0}", .unsupportedVersion(0)), ("{\"version\":2}", .unsupportedVersion(2)),
            ("{\"version\":-1}", .unsupportedVersion(-1))
        ]
        for (contents, expected) in cases {
            let data = Data(contents.utf8)
            try data.write(to: url)
            try rejects(expected) { try bindings.load(from: url) }
            try check(bindings == before, "Bad file partially changed memory: \(contents)")
            try check(Data(contentsOf: url) == data, "Bad file silently rewritten")
        }
        try Data([0xFF, 0xFE, 0x00]).write(to: url)
        try rejects(.invalidFile) { try bindings.load(from: url) }
        try check(bindings == before, "Invalid bytes changed memory")
    }

    static func invalidDocuments(in directory: URL) throws {
        let url = directory.appendingPathComponent("document.json")
        var bindings = Bindings()
        try bindings.save(to: url)
        let data = try Data(contentsOf: url)
        guard let document = try JSONSerialization.jsonObject(with: data) as? [String: Any],
              let entries = document["bindings"] as? [[String: Any]] else {
            throw Failure(message: "Fixture encoding failed")
        }
        try bindings.rebind(.x, to: [Chord(keyCode: 15)])
        let before = bindings
        func rejectEntries(_ entries: [[String: Any]], _ expected: BindingError) throws {
            let encoded = try JSONSerialization.data(withJSONObject: ["version": 1, "bindings": entries])
            try encoded.write(to: url)
            try rejects(expected) { try bindings.load(from: url) }
            try check(bindings == before, "Invalid document partially changed memory")
            try check(Data(contentsOf: url) == encoded, "Invalid document overwritten")
        }
        try rejectEntries(entries.filter { $0["action"] as? String != "enter" }, .incompleteActions)
        // A file saved before the modern actions existed migrates them to their defaults,
        // except a default whose key the file already uses.
        let withoutModern = entries.filter { entry in !Action.modern.map(\.rawValue).contains(entry["action"] as? String ?? "") }
        var beforeModern = withoutModern
        beforeModern[beforeModern.firstIndex { $0["action"] as? String == "x" }!]["chords"] = [["keyCode": 47, "modifiers": 0, "allowsAdditionalGameplayModifiers": false]]
        try JSONSerialization.data(withJSONObject: ["version": 1, "bindings": beforeModern]).write(to: url)
        var upgraded = Bindings()
        try upgraded.load(from: url)
        try check(upgraded.chords(for: .nextIdleWorker).isEmpty && upgraded.action(keyCode: 47) == .x
                  && upgraded.chords(for: .lastAlert).map(\.keyCode) == [115] && upgraded.chords(for: .mainStructure).map(\.keyCode) == [4],
                  "Pre-modern file migrates without stealing a saved key")
        // A version-1 file saved before Tab existed migrates: saved chords stay,
        // Tab gets its default key, or none when that key is already taken.
        let withoutTab = entries.filter { $0["action"] as? String != "tab" }
        try JSONSerialization.data(withJSONObject: ["version": 1, "bindings": withoutTab]).write(to: url)
        var migrated = Bindings()
        try migrated.rebind(.x, to: [Chord(keyCode: 15)])
        try migrated.load(from: url)
        try check(migrated.chords(for: .tab).map(\.keyCode) == [48] && migrated.chords(for: .x).map(\.keyCode) == [7], "Pre-Tab file migrates")
        var taken = withoutTab
        taken[taken.firstIndex { $0["action"] as? String == "x" }!]["chords"] = [["keyCode": 48, "modifiers": 0, "allowsAdditionalGameplayModifiers": true]]
        try JSONSerialization.data(withJSONObject: ["version": 1, "bindings": taken]).write(to: url)
        try migrated.load(from: url)
        try check(migrated.chords(for: .tab).isEmpty && migrated.action(keyCode: 48) == .x, "Migration never steals a saved key")
        try rejectEntries(entries + [entries[0]], .duplicateAction(.enter))
        var changed = entries
        changed[0]["action"] = "pause"
        try rejectEntries(changed, .invalidFile)
        for (chord, error): ([String: Any], BindingError) in [
            (["keyCode": 128, "modifiers": 0, "allowsAdditionalGameplayModifiers": false], .invalidKeyCode(128)),
            (["keyCode": 15, "modifiers": 16, "allowsAdditionalGameplayModifiers": false], .invalidModifiers),
            (["keyCode": 15, "modifiers": 8, "allowsAdditionalGameplayModifiers": false], .reservedForMenu),
            (["keyCode": 15, "modifiers": 256, "allowsAdditionalGameplayModifiers": false], .invalidFile),
            (["keyCode": -1, "modifiers": 0, "allowsAdditionalGameplayModifiers": false], .invalidFile),
            (["keyCode": 65536, "modifiers": 0, "allowsAdditionalGameplayModifiers": false], .invalidFile),
            (["keyCode": 15, "modifiers": 0], .invalidFile),
            (["keyCode": 53, "modifiers": 1, "allowsAdditionalGameplayModifiers": false], .conflict(.enter, .escape))
        ] {
            changed = entries
            changed[0]["chords"] = [chord]
            try rejectEntries(changed, error)
        }
        // A valid early entry followed by a bad last entry must never commit early.
        changed = entries
        changed[0]["chords"] = [["keyCode": 15, "modifiers": 2, "allowsAdditionalGameplayModifiers": false]]
        changed[changed.count - 1]["chords"] = [["keyCode": 65535, "modifiers": 0, "allowsAdditionalGameplayModifiers": false]]
        try rejectEntries(changed, .invalidKeyCode(65535))
        try data.write(to: url)
        try bindings.load(from: url)
        try check(bindings == Bindings(), "Valid file failed after rejected loads")
    }

    static func ioFailures(in directory: URL) throws {
        let url = directory.appendingPathComponent("missing.json")
        var bindings = Bindings()
        try bindings.rebind(.x, to: [Chord(keyCode: 15)])
        let before = bindings
        try rejects(.readFailed) { try bindings.load(from: url) }
        try check(!FileManager.default.fileExists(atPath: url.path), "Missing file created")
        let remote = URL(string: "https://example.invalid/hotkeys.json")!
        try rejects(.nonFileURL) { try bindings.load(from: remote) }
        try rejects(.nonFileURL) { try bindings.save(to: remote) }
        // ENOTDIR is deterministic, including when tests run with elevated rights.
        let blocker = directory.appendingPathComponent("regular-file")
        let sentinel = Data("keep existing file".utf8)
        try sentinel.write(to: blocker)
        try rejects(.writeFailed) { try bindings.save(to: blocker.appendingPathComponent("bindings.json")) }
        try check(Data(contentsOf: blocker) == sentinel, "Failed save damaged existing file")
        try rejects(.writeFailed) { try bindings.save(to: directory) }
        try check(bindings == before, "IO failure changed memory")
    }

    static func main() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("RisingLands-HotkeyBindings-\(UUID().uuidString)", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
        do {
            try defaults()
            print("PASS defaults: 20 channel actions, 21 physical keys, all gameplay modifiers; 4 exact modern actions")
            try rebindingAndConflicts()
            print("PASS rebinding, alias conflicts, exact modifiers, unbind and transactional swap")
            try menuAndInvalidBindings()
            print("PASS Command menu reservation and invalid key/modifier rejection")
            try roundTripAndReset(in: directory)
            print("PASS round trip, atomic replacement and explicit reset persistence")
            try invalidFiles(in: directory)
            print("PASS malformed, truncated, invalid-byte and unsupported-version files")
            try invalidDocuments(in: directory)
            print("PASS schema validation and atomic rejection of late document errors")
            try ioFailures(in: directory)
            print("PASS explicit file URLs and nondestructive IO failures")
        } catch {
            try FileManager.default.removeItem(at: directory)
            throw error
        }
        try FileManager.default.removeItem(at: directory)
        print("HotkeyBindingsTests: 7 groups passed; temporary files only; no UI")
    }
}
