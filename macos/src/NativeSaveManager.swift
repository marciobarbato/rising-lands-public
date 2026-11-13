import AppKit

/// Jogo ▸ Gerenciar partidas salvas…: a sheet over the game window listing the
/// existing slots (name, stage, date) with Renomear… and Apagar….
///
/// Safety: it opens only outside the original save and load lists and with no
/// save or load in progress; it pauses the game first and waits for any save
/// still being written. While it is open the game window takes no input and the
/// Jogo menu's save, load, pause, options and reimport items are disabled
/// (`saveOperationInProgress` and `saveManager`), so no other writer touches the
/// folder. The engine reads a save file only when it loads it, and the original
/// lists read the names when they open, so they simply see the result.
final class NativeSaveManager: NSObject, NSTableViewDataSource, NSTableViewDelegate, NSWindowDelegate {
    let window: NSWindow
    let table = NSTableView()
    let rename = NSButton(title: "Renomear…", target: nil, action: nil)
    let delete = NSButton(title: "Apagar…", target: nil, action: nil)
    let close = NSButton(title: "Fechar", target: nil, action: nil)
    let empty = NSTextField(labelWithString: "Nenhuma partida salva.")
    let note = NSTextField(wrappingLabelWithString: "As partidas apagadas vão para a Lixeira e podem ser recuperadas pelo Finder.")
    var library: NativeSaveLibrary
    private(set) var games = [NativeSavedGame]()
    /// The field of the open Renomear sheet (smokes fill it).
    private(set) var nameField: NSTextField?
    /// Every alert this window showed, in order (smokes read the texts).
    private(set) var alerts = [NSAlert]()
    var changed: (() -> Void)?
    var closed: (() -> Void)?
    /// Trash URLs of the last delete (smokes check them).
    private(set) var lastTrashed = [URL]()
    let dateFormatter: DateFormatter = { let f = DateFormatter(); f.dateStyle = .short; f.timeStyle = .short; return f }()

    init(library: NativeSaveLibrary) {
        self.library = library
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 620, height: 380), styleMask: [.titled], backing: .buffered, defer: false)
        super.init()
        window.title = "Partidas salvas"
        let content = NSView(frame: window.contentRect(forFrameRect: window.frame))
        let title = NSTextField(labelWithString: "Partidas salvas")
        title.font = .boldSystemFont(ofSize: 15); title.frame = NSRect(x: 20, y: 340, width: 580, height: 22)
        for (identifier, heading, width) in [("name", "Nome", 260.0), ("stage", "Fase", 150.0), ("date", "Data", 150.0)] {
            let column = NSTableColumn(identifier: .init(identifier)); column.title = heading; column.width = width
            table.addTableColumn(column)
        }
        table.usesAlternatingRowBackgroundColors = true; table.allowsEmptySelection = true
        table.dataSource = self; table.delegate = self
        table.target = self; table.doubleAction = #selector(renameSelected)
        let scroll = NSScrollView(frame: NSRect(x: 20, y: 96, width: 580, height: 236))
        scroll.documentView = table; scroll.hasVerticalScroller = true; scroll.borderType = .bezelBorder
        empty.frame = NSRect(x: 20, y: 200, width: 580, height: 20); empty.alignment = .center; empty.textColor = .secondaryLabelColor
        note.frame = NSRect(x: 20, y: 58, width: 580, height: 32); note.textColor = .secondaryLabelColor; note.font = .systemFont(ofSize: 11)
        rename.target = self; rename.action = #selector(renameSelected); rename.frame = NSRect(x: 20, y: 16, width: 130, height: 32)
        delete.target = self; delete.action = #selector(deleteSelected); delete.frame = NSRect(x: 156, y: 16, width: 130, height: 32)
        close.target = self; close.action = #selector(closeManager); close.frame = NSRect(x: 470, y: 16, width: 130, height: 32)
        close.keyEquivalent = "\u{1b}"
        for view in [title, scroll, empty, note, rename, delete, close] as [NSView] { content.addSubview(view) }
        window.contentView = content
        reload()
    }

    func reload(select slot: String? = nil) {
        let previous = slot ?? selected?.slot
        games = library.games()
        table.reloadData()
        if let previous, let row = games.firstIndex(where: { $0.slot == previous }) { table.selectRowIndexes([row], byExtendingSelection: false) }
        else if !games.isEmpty { table.selectRowIndexes([0], byExtendingSelection: false) }
        empty.isHidden = !games.isEmpty
        updateButtons()
    }
    var selected: NativeSavedGame? { games.indices.contains(table.selectedRow) ? games[table.selectedRow] : nil }
    func select(_ slot: String) -> Bool {
        guard let row = games.firstIndex(where: { $0.slot == slot }) else { return false }
        table.selectRowIndexes([row], byExtendingSelection: false); updateButtons(); return true
    }
    func updateButtons() {
        rename.isEnabled = selected?.canRename == true && window.attachedSheet == nil
        delete.isEnabled = selected != nil && window.attachedSheet == nil
    }

    func numberOfRows(in tableView: NSTableView) -> Int { games.count }
    func tableView(_ tableView: NSTableView, viewFor column: NSTableColumn?, row: Int) -> NSView? {
        let game = games[row]
        let text: String
        switch column?.identifier.rawValue {
        case "name": text = game.name
        case "stage": text = game.stageTitle
        default: text = dateFormatter.string(from: game.date)
        }
        let cell = NSTextField(labelWithString: text); cell.lineBreakMode = .byTruncatingTail
        if column?.identifier.rawValue == "name", !game.canRename { cell.textColor = .secondaryLabelColor }
        return cell
    }
    func tableViewSelectionDidChange(_ notification: Notification) { updateButtons() }

    func show(over parent: NSWindow) {
        reload()
        parent.beginSheet(window) { [weak self] _ in self?.closed?() }
        window.makeFirstResponder(table)
    }
    @objc func closeManager() {
        guard window.attachedSheet == nil else { return }
        window.sheetParent?.endSheet(window)
    }

    func present(_ alert: NSAlert, _ done: @escaping (NSApplication.ModalResponse) -> Void) {
        alerts.append(alert)
        alert.beginSheetModal(for: window) { [weak self] response in
            self?.nameField = nil; done(response)
            // The sheet is still attached while its handler runs.
            DispatchQueue.main.async { self?.updateButtons() }
        }
        updateButtons()
    }
    func report(_ error: Error) {
        let alert = NSAlert(); alert.alertStyle = .warning
        alert.messageText = "Não foi possível concluir a operação"; alert.informativeText = error.localizedDescription
        alert.addButton(withTitle: "OK")
        present(alert) { _ in }
    }

    @objc func renameSelected() {
        guard let game = selected, game.canRename, window.attachedSheet == nil else { return }
        let alert = NSAlert()
        alert.messageText = "Renomear “\(game.name)”"
        alert.informativeText = "É o nome que as listas Salvar e Carregar do jogo mostram. Elas exibem os primeiros \(NativeSaveLibrary.visibleNameCharacters) caracteres, seguidos da data."
        alert.addButton(withTitle: "Renomear"); alert.addButton(withTitle: "Cancelar")
        let field = NSTextField(string: game.name); field.frame = NSRect(x: 0, y: 0, width: 320, height: 24)
        alert.accessoryView = field; nameField = field
        alert.window.initialFirstResponder = field
        let text = { field.stringValue }
        present(alert) { [weak self] response in
            guard let self, response == .alertFirstButtonReturn else { return }
            do { try self.library.rename(game, to: text()); self.reload(select: game.slot); self.changed?() }
            catch { self.reload(); DispatchQueue.main.async { self.report(error) } }
        }
    }

    @objc func deleteSelected() {
        guard let game = selected, window.attachedSheet == nil else { return }
        let alert = NSAlert(); alert.alertStyle = .warning
        alert.messageText = "Apagar “\(game.name)”?"
        var text = "\(game.stageTitle) · \(dateFormatter.string(from: game.date)). A partida vai para a Lixeira; enquanto a Lixeira não for esvaziada, ela pode ser recuperada pelo Finder, junto do arquivo .view."
        if game.isStageStart {
            text = "Este é o salvamento automático do início da fase. Sem ele, Carregar partida ▸ Início da fase fica indisponível até a próxima fase começar.\n\n" + text
        }
        alert.informativeText = text
        alert.addButton(withTitle: "Mover para a Lixeira"); alert.addButton(withTitle: "Cancelar")
        present(alert) { [weak self] response in
            guard let self, response == .alertFirstButtonReturn else { return }
            do { self.lastTrashed = try self.library.trash(game) }
            catch { self.reload(); self.changed?(); DispatchQueue.main.async { self.report(error) }; return }
            self.reload(); self.changed?()
        }
    }
}

extension AppDelegate {
    /// Original save/load lists (RL_SHELL_LOAD_LIST, LOAD_MENU, SAVE_LIST) read
    /// the captions when they open; the manager stays closed while one is shown.
    static let originalSaveListModes: Set<UInt32> = [UInt32(RL_SHELL_LOAD_LIST), UInt32(RL_SHELL_LOAD_MENU), UInt32(RL_SHELL_SAVE_LIST)]
    func saveManagerAvailable() -> Bool {
        guard let live = nativeLive, !live.validationMode, !live.recording, !saveOperationInProgress, saveManager == nil,
              live.window.isVisible, live.window.attachedSheet == nil else { return false }
        return !Self.originalSaveListModes.contains(live.originalShellMode)
    }

    @objc func manageSaves() {
        guard saveManagerAvailable(), let live = nativeLive else { NSSound.beep(); return }
        live.metal.releaseKeys()
        let wasRunning = live.isRunning
        if wasRunning { live.perform("live-pause", completion: nil) }
        saveOperationInProgress = true; refreshSaveMenus()
        // Any save captured before the pause is written before the list is read.
        live.work.async {
            live.waitForSaves()
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                let manager = NativeSaveManager(library: NativeSaveLibrary(directory: live.saveDirectory))
                self.saveManager = manager
                manager.changed = { [weak self] in self?.refreshSaveMenus() }
                manager.closed = { [weak self] in
                    guard let self else { return }
                    self.saveManager = nil; self.saveOperationInProgress = false; self.refreshSaveMenus()
                    live.window.makeFirstResponder(live.metal)
                    if wasRunning && !live.isRunning { live.perform("live-play", completion: nil) }
                }
                manager.show(over: live.window)
            }
        }
    }
}
