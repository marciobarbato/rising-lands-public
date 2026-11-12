import Foundation

/// Prepared P5 component; no event hooks, UI, implicit preferences or engine calls.
struct NativeHotkeyBindings: Equatable, Sendable {
    /// The first cases are existing input channels, not claims about their in-game
    /// meaning. The modern cases emit no channel of their own: with "Atalhos
    /// modernos" on, the host turns them into ordinary key/mouse input that the
    /// original already accepts (NativeModernHotkeys.swift).
    enum Action: String, CaseIterable, Codable, Sendable {
        case enter, escape, space, left, up, right, down, delete, x, tab
        case digit0, digit1, digit2, digit3, digit4, digit5, digit6, digit7, digit8, digit9
        case lastAlert, nextIdleWorker, nextIdleMilitary, mainStructure

        /// nil for modern actions, which have no channel of their own.
        var virtualKey: Int? {
            switch self {
            case .enter: return 13
            case .escape: return 27
            case .space: return 32
            case .left: return 37
            case .up: return 38
            case .right: return 39
            case .down: return 40
            case .delete: return 46
            case .x: return 88
            case .tab: return 9
            case .digit0: return 48
            case .digit1: return 49
            case .digit2: return 50
            case .digit3: return 51
            case .digit4: return 52
            case .digit5: return 53
            case .digit6: return 54
            case .digit7: return 55
            case .digit8: return 56
            case .digit9: return 57
            case .lastAlert, .nextIdleWorker, .nextIdleMilitary, .mainStructure: return nil
            }
        }

        var isModern: Bool { virtualKey == nil }
        static let modern: [Action] = [.lastAlert, .nextIdleWorker, .nextIdleMilitary, .mainStructure]

        /// Action labels only. Physical-key labels must come from the host's layout.
        var labelPTBR: String {
            switch self {
            case .enter: return "Enter"
            case .escape: return "Esc"
            case .space: return "Espaço"
            case .left: return "Seta para a esquerda"
            case .up: return "Seta para cima"
            case .right: return "Seta para a direita"
            case .down: return "Seta para baixo"
            case .delete: return "Delete (à frente)"
            case .x: return "X"
            case .tab: return "Tab (minimapa)"
            case .digit0: return "0"
            case .digit1: return "1"
            case .digit2: return "2"
            case .digit3: return "3"
            case .digit4: return "4"
            case .digit5: return "5"
            case .digit6: return "6"
            case .digit7: return "7"
            case .digit8: return "8"
            case .digit9: return "9"
            case .lastAlert: return "Último alerta (moderno)"
            case .nextIdleWorker: return "Próximo trabalhador ocioso (moderno)"
            case .nextIdleMilitary: return "Próximo militar ocioso (moderno)"
            case .mainStructure: return "Santuário (moderno)"
            }
        }
    }

    /// Component bits, deliberately independent of NSEvent.ModifierFlags raw values.
    struct Modifiers: OptionSet, Hashable, Codable, Sendable {
        let rawValue: UInt8
        init(rawValue: UInt8) { self.rawValue = rawValue }
        static let shift = Self(rawValue: 1)
        static let control = Self(rawValue: 2)
        static let option = Self(rawValue: 4)
        static let command = Self(rawValue: 8)
        static let gameplay: Self = [.shift, .control, .option]
        static let supported: Self = [.shift, .control, .option, .command]

        init(from decoder: Decoder) throws {
            rawValue = try decoder.singleValueContainer().decode(UInt8.self)
        }
        func encode(to encoder: Encoder) throws {
            var container = encoder.singleValueContainer()
            try container.encode(rawValue)
        }
    }

    struct Chord: Hashable, Codable, Sendable {
        let keyCode: UInt16
        let modifiers: Modifiers
        /// Legacy defaults accept held Shift/Control/Option independently, as the
        /// committed view does. New bindings are exact unless explicitly requested.
        let allowsAdditionalGameplayModifiers: Bool

        init(keyCode: UInt16, modifiers: Modifiers = [],
             allowsAdditionalGameplayModifiers: Bool = false) {
            self.keyCode = keyCode
            self.modifiers = modifiers
            self.allowsAdditionalGameplayModifiers = allowsAdditionalGameplayModifiers
        }

        fileprivate func matches(keyCode: UInt16, modifiers: Modifiers) -> Bool {
            guard self.keyCode == keyCode,
                  modifiers.subtracting(.gameplay).isEmpty else { return false }
            return allowsAdditionalGameplayModifiers
                ? modifiers.isSuperset(of: self.modifiers) : modifiers == self.modifiers
        }

        fileprivate func overlaps(_ other: Self) -> Bool {
            // Only eight gameplay modifier combinations; compare actual accepted sets.
            (UInt8(0)...7).contains { bits in
                let flags = Modifiers(rawValue: bits)
                return matches(keyCode: other.keyCode, modifiers: flags)
                    && other.matches(keyCode: keyCode, modifiers: flags)
            }
        }
    }

    enum BindingError: Error, Equatable, LocalizedError {
        case invalidFile, unsupportedVersion(Int), incompleteActions, duplicateAction(Action)
        case invalidKeyCode(UInt16), invalidModifiers, reservedForMenu
        case conflict(Action, Action), nonFileURL, readFailed, writeFailed

        var errorDescription: String? {
            switch self {
            case .invalidFile: return "O arquivo de atalhos é inválido ou está incompleto."
            case .unsupportedVersion(let version): return "Versão de atalhos não compatível: \(version)."
            case .incompleteActions: return "O arquivo não contém todas as ações de teclado."
            case .duplicateAction(let action): return "A ação \(action.labelPTBR) está repetida."
            case .invalidKeyCode(let code): return "Código de tecla inválido: \(code)."
            case .invalidModifiers: return "A combinação contém modificadores inválidos."
            case .reservedForMenu: return "Combinações com Command são reservadas aos menus do app."
            case .conflict(let first, let second):
                return "Conflito de atalhos entre \(first.labelPTBR) e \(second.labelPTBR)."
            case .nonFileURL: return "Escolha um arquivo local para os atalhos."
            case .readFailed: return "Não foi possível ler o arquivo de atalhos."
            case .writeFailed: return "Não foi possível salvar o arquivo de atalhos."
            }
        }
    }

    static let fileVersion = 1
    /// Actions added after the first version-1 files; absent entries are migrated.
    static let addedActions: [Action] = [.tab] + Action.modern
    private var bindings: [Action: [Chord]]

    init() { bindings = Self.defaults }

    func chords(for action: Action) -> [Chord] { bindings[action] ?? [] }

    /// Command events never resolve to gameplay, on any keyboard layout.
    func action(keyCode: UInt16, modifiers: Modifiers = []) -> Action? {
        Action.allCases.first { action in
            chords(for: action).contains { $0.matches(keyCode: keyCode, modifiers: modifiers) }
        }
    }

    /// Includes overlaps with legacy permissive defaults. Menu reservation is
    /// reported by rebind/replace, independently of action-to-action conflicts.
    func conflicts(for chord: Chord, excluding action: Action? = nil) -> [Action] {
        Action.allCases.filter { candidate in
            candidate != action && chords(for: candidate).contains { $0.overlaps(chord) }
        }
    }

    /// Replaces every alias of an action. Empty arrays explicitly unbind it.
    /// Memory changes only after the entire candidate passes validation.
    mutating func rebind(_ action: Action, to chords: [Chord]) throws {
        var candidate = bindings
        candidate[action] = chords
        try replaceBindings(candidate)
    }

    /// Complete transaction, including swaps that would conflict if applied singly.
    mutating func replaceBindings(_ candidate: [Action: [Chord]]) throws {
        try Self.validate(candidate)
        bindings = candidate
    }

    /// In-memory only. Persistence always requires an explicit save(to:).
    mutating func resetToDefaults() { bindings = Self.defaults }

    /// Read, decode, version-check and validate before assigning anything.
    /// Missing/corrupt files throw; neither memory nor the file is reset implicitly.
    mutating func load(from url: URL) throws {
        guard url.isFileURL else { throw BindingError.nonFileURL }
        let data: Data
        do { data = try Data(contentsOf: url) }
        catch { throw BindingError.readFailed }
        let document: Document
        do {
            let decoder = JSONDecoder()
            let header = try decoder.decode(Header.self, from: data)
            guard header.version == Self.fileVersion else {
                throw BindingError.unsupportedVersion(header.version)
            }
            document = try decoder.decode(Document.self, from: data)
        } catch let error as BindingError { throw error }
        catch { throw BindingError.invalidFile }
        var candidate: [Action: [Chord]] = [:]
        for entry in document.bindings {
            guard candidate[entry.action] == nil else { throw BindingError.duplicateAction(entry.action) }
            candidate[entry.action] = entry.chords
        }
        // Files saved before an action existed keep every saved choice; the new
        // action takes its default unless that physical key is already in use.
        for action in Self.addedActions where candidate[action] == nil {
            let used = candidate.values.flatMap { $0 }
            candidate[action] = Self.defaults[action]!.filter { chord in !used.contains { $0.overlaps(chord) } }
        }
        try replaceBindings(candidate)
    }

    /// Atomic replacement at the supplied path. Does not create parent directories,
    /// touch UserDefaults or select an Application Support location.
    func save(to url: URL) throws {
        guard url.isFileURL else { throw BindingError.nonFileURL }
        let document = Document(version: Self.fileVersion, bindings: Action.allCases.map {
            Entry(action: $0, chords: chords(for: $0))
        })
        do {
            let encoder = JSONEncoder()
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
            try encoder.encode(document).write(to: url, options: .atomic)
        } catch { throw BindingError.writeFailed }
    }

    private struct Header: Decodable { let version: Int }
    private struct Entry: Codable { let action: Action; let chords: [Chord] }
    private struct Document: Codable { let version: Int; let bindings: [Entry] }

    private static func validate(_ candidate: [Action: [Chord]]) throws {
        guard Set(candidate.keys) == Set(Action.allCases) else { throw BindingError.incompleteActions }
        var seen: [(Action, Chord)] = []
        for action in Action.allCases {
            for chord in candidate[action] ?? [] {
                // 54...63 are modifier/lock/function keys, not keyDown channels.
                guard chord.keyCode < 128, !(54...63).contains(chord.keyCode) else {
                    throw BindingError.invalidKeyCode(chord.keyCode)
                }
                guard chord.modifiers.subtracting(.supported).isEmpty else { throw BindingError.invalidModifiers }
                guard !chord.modifiers.contains(.command) else { throw BindingError.reservedForMenu }
                for (otherAction, otherChord) in seen where chord.overlaps(otherChord) {
                    throw BindingError.conflict(otherAction, action)
                }
                seen.append((action, chord))
            }
        }
    }

    /// NativeLiveWindow.swift keyDown/keyUp at 2c73209db1a896a856ca1b1f67f42602bcfabc63.
    /// No speed/pause actions are introduced, and Space keeps virtual key 32.
    private static var defaults: [Action: [Chord]] {
        let codes: [(Action, [UInt16])] = [
            (.enter, [36, 76]), (.escape, [53]), (.space, [49]),
            (.left, [123]), (.up, [126]), (.right, [124]), (.down, [125]),
            (.delete, [117]), (.x, [7]), (.tab, [48]), (.digit0, [29]), (.digit1, [18]),
            (.digit2, [19]), (.digit3, [20]), (.digit4, [21]), (.digit5, [23]),
            (.digit6, [22]), (.digit7, [26]), (.digit8, [28]), (.digit9, [25])
        ]
        var result = Dictionary(uniqueKeysWithValues: codes.map { action, keys in
            (action, keys.map { Chord(keyCode: $0, allowsAdditionalGameplayModifiers: true) })
        })
        // Modern defaults (AoE2 style on keys the original does not read): Home, ".", ",", H.
        // Exact chords, so Shift/Control/Option variants stay free.
        for (action, key): (Action, UInt16) in [(.lastAlert, 115), (.nextIdleWorker, 47),
                                                (.nextIdleMilitary, 43), (.mainStructure, 4)] {
            result[action] = [Chord(keyCode: key)]
        }
        return result
    }
}
