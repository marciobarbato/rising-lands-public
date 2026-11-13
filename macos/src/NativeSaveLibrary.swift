import Foundation

/// The player's saved games as files in the save folder (host only: the engine
/// reads a save file only when it loads it).
///
/// A slot is `<slot>.rls` (the engine's save bytes) plus its presentation
/// companion `<slot>.rls.<sha256 of the .rls>.view`. The named slots
/// posicao-1..posicao-10 are the original lists' save1.sav..save9.sav,save0.sav;
/// the name the original lists show lives in the host file `nomes.plist`
/// (key "1".."10"), never in the save itself, so renaming needs no format or
/// engine change. `rapido` (Salvar rápido), `inicio-fase` (the stage-start
/// autosave) and `autosave` (the periodic autosave, NativeAutosaveInterval.slot)
/// are not in the original lists and have fixed names.
struct NativeSavedGame: Equatable {
    enum Kind: Equatable {
        case named(Int)
        case quick
        case stageStart
        case periodic
    }
    let kind: Kind
    let file: URL
    /// Every presentation companion of this slot (normally one).
    let views: [URL]
    let date: Date
    let name: String
    /// "MONDE07B.ETA" and 0..2, read from the companion; nil when it cannot be read.
    let mission: String?
    let difficulty: UInt32?

    var slot: String { file.deletingPathExtension().lastPathComponent }
    var canRename: Bool { if case .named = kind { return true }; return false }
    var isStageStart: Bool { kind == .stageStart }
    /// "Fase 7 · Normal", "Fase 7B · Difícil" or "Fase desconhecida".
    var stageTitle: String {
        guard let mission, mission.count >= 11, let number = Int(mission.dropFirst(5).prefix(2)) else { return "Fase desconhecida" }
        let variant = mission.dropFirst(7).prefix(while: { $0 != "." })
        let difficultyTitle = difficulty.flatMap { $0 <= 2 ? ["Fácil", "Normal", "Difícil"][Int($0)] : nil }
        return "Fase \(number)\(variant)" + (difficultyTitle.map { " · \($0)" } ?? "")
    }
}

enum NativeSaveLibraryError: LocalizedError {
    case emptyName
    case notRenamable
    case missing(String)
    case trash(String, String)
    var errorDescription: String? {
        switch self {
        case .emptyName: return "Digite um nome para a partida."
        case .notRenamable: return "Só as partidas das posições 1 a 10 têm nome."
        case .missing(let name): return "A partida “\(name)” não está mais na pasta de partidas salvas."
        case .trash(let file, let reason): return "Não foi possível mover “\(file)” para a Lixeira: \(reason)"
        }
    }
}

struct NativeSaveLibrary {
    let directory: URL
    /// Moves one file to the Trash and returns where it went. Tests inject a fake.
    var trashItem: (URL) throws -> URL? = { url in
        var result: NSURL?
        try FileManager.default.trashItem(at: url, resultingItemURL: &result)
        return result as URL?
    }

    static let namesFile = "nomes.plist"
    static let namedSlots = 1...10
    static let maximumNameLength = 80
    /// The original lists show 31 CP1252 bytes: the name, then " · dd/MM HH:mm".
    static let captionBytes = 31
    static let visibleNameCharacters = 17

    init(directory: URL) { self.directory = directory }

    var namesURL: URL { directory.appendingPathComponent(Self.namesFile) }
    func url(_ slot: String) -> URL { directory.appendingPathComponent(slot).appendingPathExtension("rls") }

    func names() -> [String: String] {
        guard let data = try? Data(contentsOf: namesURL),
              let names = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: String] else { return [:] }
        return names
    }
    func writeNames(_ names: [String: String]) throws {
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try PropertyListSerialization.data(fromPropertyList: names, format: .xml, options: 0).write(to: namesURL, options: .atomic)
    }

    static func defaultName(_ kind: NativeSavedGame.Kind) -> String {
        switch kind {
        case .named(let number): return "Posição \(number)"
        case .quick: return "Salvamento rápido"
        case .stageStart: return "Início da fase · salvamento automático"
        case .periodic: return "Salvamento automático"
        }
    }
    static func slot(_ kind: NativeSavedGame.Kind) -> String {
        switch kind {
        case .named(let number): return "posicao-\(number)"
        case .quick: return "rapido"
        case .stageStart: return "inicio-fase"
        case .periodic: return "autosave"
        }
    }

    /// Existing slots: posicao-1..10 in order, then rapido, inicio-fase and autosave
    /// (the order of Jogo ▸ Carregar partida).
    func games() -> [NativeSavedGame] {
        let names = self.names()
        let files = (try? FileManager.default.contentsOfDirectory(atPath: directory.path)) ?? []
        let kinds: [NativeSavedGame.Kind] = Self.namedSlots.map { .named($0) } + [.quick, .stageStart, .periodic]
        return kinds.compactMap { kind in
            let file = url(Self.slot(kind))
            guard let date = (try? FileManager.default.attributesOfItem(atPath: file.path))?[.modificationDate] as? Date else { return nil }
            let prefix = file.lastPathComponent + "."
            let views = files.filter { $0.hasPrefix(prefix) && $0.hasSuffix(".view") && !$0.contains(".tmp.") }.sorted()
                .map { directory.appendingPathComponent($0) }
            var name = Self.defaultName(kind)
            if case .named(let number) = kind, let stored = names[String(number)] { name = stored }
            let stage = views.lazy.compactMap(Self.stage).first
            return NativeSavedGame(kind: kind, file: file, views: views, date: date, name: name, mission: stage?.0, difficulty: stage?.1)
        }
    }

    /// Mission and difficulty of an RLVIEW3/RLVIEW4 companion (72-byte header,
    /// then the presentation, then 16 bytes of mission name and a u32 difficulty).
    static let presentationSize = 480000 + 1024 + 270336 + 4
    static func stage(_ view: URL) -> (String, UInt32)? {
        guard let handle = try? FileHandle(forReadingFrom: view) else { return nil }
        defer { try? handle.close() }
        guard let magic = try? handle.read(upToCount: 8), magic == Data("RLVIEW3\n".utf8) || magic == Data("RLVIEW4\n".utf8),
              (try? handle.seek(toOffset: UInt64(72 + presentationSize))) != nil,
              let tail = try? handle.read(upToCount: 20), tail.count == 20 else { return nil }
        let nameBytes = tail.prefix(16).prefix(while: { $0 != 0 })
        guard let name = String(data: nameBytes, encoding: .ascii), name.hasPrefix("MONDE"), name.hasSuffix(".ETA"), (11...12).contains(name.count) else { return nil }
        let difficulty = tail.suffix(4).enumerated().reduce(UInt32(0)) { $0 | UInt32($1.element) << (8 * UInt32($1.offset)) }
        return (name, difficulty)
    }

    static func validName(_ text: String) throws -> String {
        let name = String(text.trimmingCharacters(in: .whitespacesAndNewlines).prefix(maximumNameLength))
        guard !name.isEmpty else { throw NativeSaveLibraryError.emptyName }
        return name
    }

    /// The name shown by the original save and load lists for posicao-number.
    @discardableResult
    func rename(_ game: NativeSavedGame, to text: String) throws -> String {
        guard case .named(let number) = game.kind else { throw NativeSaveLibraryError.notRenamable }
        guard FileManager.default.fileExists(atPath: game.file.path) else { throw NativeSaveLibraryError.missing(game.name) }
        let name = try Self.validName(text)
        var names = self.names(); names[String(number)] = name
        try writeNames(names)
        return name
    }

    /// Moves the .rls first (the slot disappears from every list), then its
    /// companions, to the Trash; a named slot also loses its name, so the next
    /// save in that position starts as "Posição N". Returns the Trash URLs.
    func trash(_ game: NativeSavedGame) throws -> [URL] {
        guard FileManager.default.fileExists(atPath: game.file.path) else { throw NativeSaveLibraryError.missing(game.name) }
        var moved = [URL]()
        for file in [game.file] + game.views {
            do { if let target = try trashItem(file) { moved.append(target) } }
            catch { throw NativeSaveLibraryError.trash(file.lastPathComponent, error.localizedDescription) }
        }
        if case .named(let number) = game.kind {
            var names = self.names()
            if names.removeValue(forKey: String(number)) != nil { try writeNames(names) }
        }
        return moved
    }

    /// The original lists' table: 10 x [present 0/1, 32-byte CP1252 caption],
    /// index 0..9 = posicao-1..posicao-10 (see macos_live_shell_slots).
    func originalListTable(_ formatter: DateFormatter = Self.captionFormatter) -> [UInt8] {
        let names = self.names()
        var table = [UInt8](repeating: 0, count: 330)
        for slot in 0..<10 {
            guard let date = (try? FileManager.default.attributesOfItem(atPath: url("posicao-\(slot + 1)").path))?[.modificationDate] as? Date else { continue }
            let caption = "\(names[String(slot + 1)] ?? "Posição \(slot + 1)") · \(formatter.string(from: date))"
            let bytes = Array((caption.data(using: .windowsCP1252, allowLossyConversion: true) ?? Data()).prefix(Self.captionBytes))
            table[33 * slot] = 1; for (i, b) in bytes.enumerated() { table[33 * slot + 1 + i] = b }
        }
        return table
    }
    static var captionFormatter: DateFormatter { let formatter = DateFormatter(); formatter.dateFormat = "dd/MM HH:mm"; return formatter }
}
