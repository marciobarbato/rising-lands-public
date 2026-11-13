import Foundation

// Standalone headless executable. Compile ONLY with NativeSaveLibrary.swift.
// Every file lives in a temporary folder; the fake Trash is a folder there too.
@main
struct SaveLibraryTests {
    static func expect(_ condition: Bool, _ label: String) {
        if !condition { FileHandle.standardError.write(Data("SaveLibraryTests FAILED: \(label)\n".utf8)); exit(1) }
    }
    static func write(_ url: URL, _ bytes: Data) { try! bytes.write(to: url) }
    /// An RLVIEW4 companion with only the fields the library reads.
    static func view(_ url: URL, mission: String, difficulty: UInt32, magic: String = "RLVIEW4\n") {
        var data = Data(magic.utf8) + Data(count: 64 + NativeSaveLibrary.presentationSize)
        var name = Data(mission.utf8); name.append(Data(count: 16 - name.count))
        data.append(name); data.append(contentsOf: (0..<4).map { UInt8(truncatingIfNeeded: difficulty >> (8 * $0)) })
        data.append(Data(count: 8))
        write(url, data)
    }
    /// The original lists' captions as publishSlots built them before the library existed.
    static func previousTable(_ directory: URL) -> [UInt8] {
        let names = (try? PropertyListSerialization.propertyList(from: Data(contentsOf: directory.appendingPathComponent("nomes.plist")), format: nil)) as? [String: String] ?? [:]
        let formatter = DateFormatter(); formatter.dateFormat = "dd/MM HH:mm"
        var table = [UInt8](repeating: 0, count: 330)
        for slot in 0..<10 {
            let url = directory.appendingPathComponent("posicao-\(slot + 1)").appendingPathExtension("rls")
            guard let date = (try? FileManager.default.attributesOfItem(atPath: url.path))?[.modificationDate] as? Date else { continue }
            let caption = "\(names[String(slot + 1)] ?? "Posição \(slot + 1)") · \(formatter.string(from: date))"
            let bytes = Array((caption.data(using: .windowsCP1252, allowLossyConversion: true) ?? Data()).prefix(31))
            table[33 * slot] = 1; for (i, b) in bytes.enumerated() { table[33 * slot + 1 + i] = b }
        }
        return table
    }
    static func caption(_ table: [UInt8], _ slot: Int) -> String {
        let bytes = table[(33 * slot + 1)..<(33 * slot + 33)].prefix(while: { $0 != 0 })
        return String(data: Data(bytes), encoding: .windowsCP1252) ?? ""
    }

    static func main() {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent("SaveLibraryTests-\(UUID().uuidString)", isDirectory: true)
        defer { try? FileManager.default.removeItem(at: root) }
        let folder = root.appendingPathComponent("Partidas", isDirectory: true), trashFolder = root.appendingPathComponent("Lixeira", isDirectory: true)
        try! FileManager.default.createDirectory(at: trashFolder, withIntermediateDirectories: true)
        var library = NativeSaveLibrary(directory: folder)
        library.trashItem = { url in
            let target = trashFolder.appendingPathComponent(url.lastPathComponent)
            try FileManager.default.moveItem(at: url, to: target); return target
        }

        expect(library.games().isEmpty, "a missing folder lists no saves")
        expect(library.originalListTable() == [UInt8](repeating: 0, count: 330), "a missing folder publishes ten empty slots")
        try! FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)

        let sha = String(repeating: "ab", count: 32)
        for slot in ["posicao-1", "posicao-3", "posicao-10", "rapido", "autosave", "inicio-fase", "entrada-smoke"] {
            write(folder.appendingPathComponent("\(slot).rls"), Data("RLSAVE\r\n\(slot)".utf8))
        }
        view(folder.appendingPathComponent("posicao-1.rls.\(sha).view"), mission: "MONDE07B.ETA", difficulty: 2)
        view(folder.appendingPathComponent("posicao-3.rls.\(sha).view"), mission: "MONDE12.ETA", difficulty: 1, magic: "RLVIEW3\n")
        view(folder.appendingPathComponent("rapido.rls.\(sha).view"), mission: "MONDE01.ETA", difficulty: 0)
        view(folder.appendingPathComponent("inicio-fase.rls.\(sha).view"), mission: "MONDE25.ETA", difficulty: 1)
        view(folder.appendingPathComponent("autosave.rls.\(sha).view"), mission: "MONDE03A.ETA", difficulty: 1)
        write(folder.appendingPathComponent("posicao-10.rls.\(sha).view"), Data("RLVIEW1\nold".utf8))
        write(folder.appendingPathComponent("posicao-1.rls.tmp.AB12CD"), Data("partial".utf8))
        try! library.writeNames(["1": "Base do norte", "2": "apagada antes"])

        var games = library.games()
        expect(games.map(\.slot) == ["posicao-1", "posicao-3", "posicao-10", "rapido", "inicio-fase", "autosave"], "named slots in order, then quick, stage start and periodic autosave; other files ignored")
        expect(games.map(\.name) == ["Base do norte", "Posição 3", "Posição 10", "Salvamento rápido", "Início da fase · salvamento automático", "Salvamento automático"], "stored and default names")
        expect(games.map(\.stageTitle) == ["Fase 7B · Difícil", "Fase 12 · Normal", "Fase desconhecida", "Fase 1 · Fácil", "Fase 25 · Normal", "Fase 3A · Normal"], "stage and difficulty from the companion")
        expect(games[0].views.map(\.lastPathComponent) == ["posicao-1.rls.\(sha).view"], "the temporary write is not a companion")
        expect(games.map(\.canRename) == [true, true, true, false, false, false], "only posicao-N has a name")
        expect(games.map(\.isStageStart) == [false, false, false, false, true, false], "stage start flagged")
        expect(library.originalListTable() == previousTable(folder), "original list captions unchanged")
        expect(caption(library.originalListTable(), 0).hasPrefix("Base do norte · "), "posicao-1 caption")

        // Rename.
        expect(try! library.rename(games[1], to: "  Ataque à ponte  ") == "Ataque à ponte", "rename trims")
        expect(library.names()["3"] == "Ataque à ponte" && library.names()["1"] == "Base do norte", "rename keeps the other names")
        expect(library.games()[1].name == "Ataque à ponte", "renamed in the list")
        expect(caption(library.originalListTable(), 2).hasPrefix("Ataque à ponte · "), "the original list sees the new name (CP1252)")
        expect(library.originalListTable() == previousTable(folder), "captions still match after rename")
        let long = String(repeating: "x", count: 100)
        expect(try! library.rename(games[2], to: long).count == NativeSaveLibrary.maximumNameLength, "long names are cut like Salvar partida…")
        do { try library.rename(games[1], to: "   \n"); expect(false, "empty name refused") } catch NativeSaveLibraryError.emptyName {} catch { expect(false, "empty name error") }
        do { try library.rename(games[3], to: "Rápido"); expect(false, "quick save has no name") } catch NativeSaveLibraryError.notRenamable {} catch { expect(false, "not renamable error") }
        expect(library.names()["3"] == "Ataque à ponte", "a refused rename changes nothing")

        // Delete to the (fake) Trash.
        games = library.games()
        let moved = try! library.trash(games[1])
        expect(moved.map(\.lastPathComponent) == ["posicao-3.rls", "posicao-3.rls.\(sha).view"], "the .rls first, then its companion")
        expect(moved.allSatisfy { FileManager.default.fileExists(atPath: $0.path) }, "both files are in the Trash")
        expect(!FileManager.default.fileExists(atPath: folder.appendingPathComponent("posicao-3.rls").path), "slot file gone")
        expect(library.names()["3"] == nil && library.names()["1"] == "Base do norte", "the deleted slot's name is dropped, others kept")
        expect(library.games().map(\.slot) == ["posicao-1", "posicao-10", "rapido", "inicio-fase", "autosave"], "deleted slot no longer listed")
        expect(library.originalListTable()[33 * 2] == 0, "the original lists see an empty position 3")
        expect(library.originalListTable() == previousTable(folder), "captions still match after delete")
        do { _ = try library.trash(games[1]); expect(false, "a deleted slot cannot be deleted again") } catch NativeSaveLibraryError.missing {} catch { expect(false, "missing error") }
        do { try library.rename(games[1], to: "Fantasma"); expect(false, "a deleted slot cannot be renamed") } catch NativeSaveLibraryError.missing {} catch { expect(false, "missing rename error") }
        expect(library.names()["3"] == nil, "renaming a deleted slot writes no name")

        let stageStart = library.games().first(where: \.isStageStart)!
        expect(try! library.trash(stageStart).count == 2, "the stage-start autosave can be deleted")
        expect(try! library.trash(library.games().last!).map(\.lastPathComponent) == ["autosave.rls", "autosave.rls.\(sha).view"], "the periodic autosave can be deleted")
        expect(library.names() == ["1": "Base do norte", "2": "apagada antes", "10": String(repeating: "x", count: 80)], "deleting an unnamed slot keeps the names")

        var failing = library
        failing.trashItem = { _ in throw NSError(domain: "teste", code: 1, userInfo: [NSLocalizedDescriptionKey: "sem permissão"]) }
        do { _ = try failing.trash(library.games()[0]); expect(false, "a Trash failure is reported") }
        catch NativeSaveLibraryError.trash(let file, let reason) { expect(file == "posicao-1.rls" && reason == "sem permissão", "failure names the file") }
        catch { expect(false, "trash error") }
        expect(FileManager.default.fileExists(atPath: folder.appendingPathComponent("posicao-1.rls").path) && library.names()["1"] == "Base do norte", "a failed delete keeps the slot and its name")
        print("SaveLibraryTests passed")
    }
}
