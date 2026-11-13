import AppKit

/// Repeatable smoke of Jogo ▸ Gerenciar partidas salvas… (player edition):
///
///   RisingLands --data-dir IMPORTED --save-directory DISPOSABLE/Partidas --save-manager-smoke REPORT.json
///
/// The save folder must be a fresh disposable one (never the player's Partidas):
/// the smoke refuses a folder that already has saves. It starts MONDE01 from the
/// original main menu (the stage-start autosave), saves posicao-1, posicao-2 and
/// rapido through the production save path, opens the manager over the running
/// stage, renames posicao-1 through the window, deletes posicao-2 and the
/// stage-start autosave to the Trash (cancelling one delete first), closes the
/// window and checks that play resumes. The files it moved to the Trash are its
/// own disposable saves; it removes exactly those Trash items at the end.
extension AppDelegate {
    static let saveManagerSmokeChecks = 23

    func startSaveManagerSmoke(_ output: URL) {
        guard let live = nativeLive else { return }
        var checks = [String: Bool]()
        var info = [String: Any]()
        var finished = false
        var trashed = [URL]()
        let started = Date()
        let library = NativeSaveLibrary(directory: live.saveDirectory)
        info["saveDirectory"] = live.saveDirectory.path
        func finish(_ error: String? = nil) {
            guard !finished else { return }; finished = true
            // Only the resultingItemURLs trashItem returned in this run, each checked
            // (a regular file directly in the user's Trash); anything else stays there.
            var removed = [String](), kept = [String]()
            let trash = (try? FileManager.default.url(for: .trashDirectory, in: .userDomainMask, appropriateFor: nil, create: false))?.standardizedFileURL
            for url in trashed {
                var directory: ObjCBool = true
                let ours = trash != nil && url.standardizedFileURL.deletingLastPathComponent() == trash
                    && FileManager.default.fileExists(atPath: url.path, isDirectory: &directory) && !directory.boolValue
                if ours, (try? FileManager.default.removeItem(at: url)) != nil { removed.append(url.path) } else { kept.append(url.path) }
            }
            info["trashItemsRemoved"] = removed; info["trashItemsKept"] = kept
            var result: [String: Any] = ["passed": error == nil && checks.count == Self.saveManagerSmokeChecks && checks.values.allSatisfy { $0 }, "checks": checks,
                                         "checksPassed": checks.values.filter { $0 }.count, "checksTotal": checks.count, "info": info,
                                         "edition": "player", "seconds": Date().timeIntervalSince(started),
                                         "commit": Bundle.main.object(forInfoDictionaryKey: "RisingLandsSourceCommit") as? String ?? ""]
            if let error { result["error"] = error }
            do { try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: output, options: .atomic) }
            catch { fputs("Save manager smoke report: \(error)\n", stderr) }
            NSApp.terminate(nil)
        }
        func wait(_ left: Int, _ label: String, _ test: @escaping () -> Bool, _ next: @escaping () -> Void) {
            guard !finished else { return }
            if test() { next(); return }
            guard left > 0 else { finish("Timed out: \(label)"); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { wait(left - 1, label, test, next) }
        }
        func frames(_ v: [String: Any]) -> UInt64 { (v["native_metrics"] as? [String: Any])?["frames"] as? UInt64 ?? 0 }
        func mode(_ v: [String: Any]) -> UInt32 { v["original_shell_mode"] as? UInt32 ?? 0 }
        func until(_ left: Int, _ label: String, _ test: @escaping ([String: Any]) -> Bool, _ then: @escaping ([String: Any]) -> Void) {
            guard !finished else { return }
            live.perform("live-status") { value in
                if test(value) { then(value); return }
                if value["live_stop"] is [String: Any] { finish("Native stop: \(value["live_stop"]!)"); return }
                guard left > 0 else { finish("Timed out: \(label)"); return }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { until(left - 1, label, test, then) }
            }
        }
        func enter() {
            live.window.makeKeyAndOrderFront(nil); _ = live.window.makeFirstResponder(live.metal)
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                if let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: live.window.windowNumber,
                                                context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36) {
                    live.window.sendEvent(event)
                }
            }
        }
        let gameMenu = NSApp.mainMenu?.items.first(where: { $0.submenu?.title == "Jogo" })?.submenu
        let actions: [String: Selector] = ["Gerenciar partidas salvas…": #selector(manageSaves), "Salvar rápido": #selector(quickSave),
                                           "Pausar": #selector(togglePause), "Importar o CD novamente…": #selector(reimportCD), "Opções…": #selector(showOptions)]
        /// Validated like the open menu would (titles such as Pausar/Continuar change).
        func enabled(_ title: String) -> Bool {
            guard let action = actions[title], let item = gameMenu?.items.first(where: { $0.action == action }) else { return false }
            return validateMenuItem(item)
        }
        func save(_ slot: String, _ name: String?, _ number: Int?, _ next: @escaping (Bool) -> Void) {
            saveSlot(slot, name: name, number: number) { ok in next(ok) }
        }
        /// Answers the sheet the manager has open with a button.
        func answer(_ manager: NativeSaveManager, _ label: String, _ response: NSApplication.ModalResponse, fill: String? = nil, _ next: @escaping (NSAlert) -> Void) {
            wait(50, label, { manager.window.attachedSheet != nil }) {
                guard let alert = manager.alerts.last, let sheet = manager.window.attachedSheet else { finish("No sheet: \(label)"); return }
                if let fill { manager.nameField?.stringValue = fill }
                manager.window.endSheet(sheet, returnCode: response)
                wait(50, label + " closed", { manager.window.attachedSheet == nil }) { next(alert) }
            }
        }
        func exists(_ slot: String) -> Bool { FileManager.default.fileExists(atPath: library.url(slot).path) }
        func companions(_ slot: String) -> Int {
            ((try? FileManager.default.contentsOfDirectory(atPath: live.saveDirectory.path)) ?? []).filter { $0.hasPrefix(slot + ".rls.") && $0.hasSuffix(".view") }.count
        }

        guard library.games().isEmpty else { finish("The save folder already has saves; use a fresh disposable folder"); return }
        let menuItems = gameMenu?.items.map(\.title) ?? []
        if let load = menuItems.firstIndex(of: "Carregar partida"), let manage = menuItems.firstIndex(of: "Gerenciar partidas salvas…") {
            checks["menuItemAfterLoad"] = manage == load + 1
        } else { checks["menuItemAfterLoad"] = false }
        info["gameMenu"] = menuItems

        until(300, "original main menu", { mode($0) == UInt32(RL_SHELL_MAIN) && $0["mission"] as? String == CampaignSelection.menu }) { _ in
            checks["availableInMainMenu"] = self.saveManagerAvailable() && enabled("Gerenciar partidas salvas…")
            enter()
            until(100, "single player", { mode($0) == UInt32(RL_SHELL_SINGLE) }) { _ in
                enter()
                until(100, "briefing", { mode($0) == UInt32(RL_SHELL_BRIEFING) }) { _ in
                    enter()
                    until(150, "MONDE01 running", { $0["mission"] as? String == "MONDE01.ETA" && mode($0) == 0 && frames($0) >= 40 }) { _ in
                        checks["stageStartAutosaved"] = exists("inicio-fase") && companions("inicio-fase") == 1
                        save("posicao-1", "Primeira base", 1) { a in save("posicao-2", "Segunda base", 2) { b in save("rapido", nil, nil) { c in
                            checks["slotsSaved"] = a && b && c && ["posicao-1", "posicao-2", "rapido"].allSatisfy { exists($0) && companions($0) == 1 }
                            open()
                        } } }
                    }
                }
            }
        }

        func open() {
            until(100, "playing after the saves", { live.isRunning && frames($0) >= 40 }) { _ in
                checks["availableWhilePlaying"] = enabled("Gerenciar partidas salvas…")
                guard let item = gameMenu?.items.first(where: { $0.action == #selector(self.manageSaves) }), let action = item.action,
                      NSApp.sendAction(action, to: item.target, from: item) else { finish("Jogo > Gerenciar partidas salvas… not sent"); return }
                wait(50, "manager sheet", { self.saveManager?.window.sheetParent === live.window }) {
                    guard let manager = self.saveManager else { return }
                    checks["menuDisabledWhileOpen"] = !enabled("Gerenciar partidas salvas…") && !enabled("Salvar rápido") && !enabled("Pausar")
                        && !enabled("Importar o CD novamente…") && !enabled("Opções…")
                    live.perform("live-status") { paused in
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1) { live.perform("live-status") { later in
                            info["framesWhenOpened"] = frames(paused); info["framesOneSecondLater"] = frames(later)
                            checks["pausedWhileOpen"] = !live.isRunning && frames(later) == frames(paused)
                            list(manager)
                        } }
                    }
                }
            }
        }

        func list(_ manager: NativeSaveManager) {
            info["listed"] = manager.games.map { ["slot": $0.slot, "name": $0.name, "stage": $0.stageTitle] }
            checks["listsSlots"] = manager.games.map(\.slot) == ["posicao-1", "posicao-2", "rapido", "inicio-fase"]
                && manager.games.map(\.name) == ["Primeira base", "Segunda base", "Salvamento rápido", "Início da fase · salvamento automático"]
                && manager.games.allSatisfy { $0.stageTitle == "Fase 1 · Normal" } && manager.table.numberOfRows == 4
            _ = manager.select("rapido")
            checks["quickNotRenamable"] = !manager.rename.isEnabled && manager.delete.isEnabled
            _ = manager.select("posicao-1")
            manager.rename.performClick(nil)
            answer(manager, "rename sheet", .alertFirstButtonReturn, fill: "  Base renomeada  ") { alert in
                let caption = NativeSaveLibrary(directory: live.saveDirectory).originalListTable()
                let text = String(data: Data(caption[1..<33].prefix(while: { $0 != 0 })), encoding: .windowsCP1252) ?? ""
                info["renameAlert"] = alert.messageText; info["originalListCaption1"] = text
                checks["renamed"] = library.names()["1"] == "Base renomeada" && manager.games.first?.name == "Base renomeada"
                    && exists("posicao-1") && companions("posicao-1") == 1
                checks["originalListSeesName"] = text.hasPrefix("Base renomeada · ") && self.slotTitle(1).hasPrefix("Base renomeada · ")
                deleteNamed(manager)
            }
        }

        func deleteNamed(_ manager: NativeSaveManager) {
            _ = manager.select("posicao-2")
            manager.delete.performClick(nil)
            answer(manager, "delete sheet", .alertFirstButtonReturn) { alert in
                info["deleteAlert"] = [alert.messageText, alert.informativeText]
                trashed += manager.lastTrashed
                checks["deleteConfirmation"] = alert.messageText == "Apagar “Segunda base”?" && alert.informativeText.contains("Lixeira")
                    && !alert.informativeText.contains("salvamento automático")
                checks["deletedToTrash"] = !exists("posicao-2") && companions("posicao-2") == 0 && manager.lastTrashed.count == 2
                    && manager.lastTrashed.allSatisfy { $0.path.contains("/.Trash/") && FileManager.default.fileExists(atPath: $0.path) }
                info["trashed"] = manager.lastTrashed.map(\.path)
                checks["deletedNameDropped"] = library.names()["2"] == nil && library.names()["1"] == "Base renomeada"
                    && manager.games.map(\.slot) == ["posicao-1", "rapido", "inicio-fase"]
                    && NativeSaveLibrary(directory: live.saveDirectory).originalListTable()[33] == 0
                cancelDelete(manager)
            }
        }

        func cancelDelete(_ manager: NativeSaveManager) {
            _ = manager.select("rapido")
            manager.delete.performClick(nil)
            answer(manager, "cancelled delete", .alertSecondButtonReturn) { _ in
                checks["cancelKeepsSlot"] = exists("rapido") && companions("rapido") == 1 && manager.games.count == 3
                deleteStageStart(manager)
            }
        }

        func deleteStageStart(_ manager: NativeSaveManager) {
            _ = manager.select("inicio-fase")
            manager.delete.performClick(nil)
            answer(manager, "stage-start delete", .alertFirstButtonReturn) { alert in
                info["stageStartAlert"] = alert.informativeText
                trashed += manager.lastTrashed
                checks["stageStartWarning"] = alert.informativeText.hasPrefix("Este é o salvamento automático do início da fase.")
                checks["stageStartDeleted"] = !exists("inicio-fase") && companions("inicio-fase") == 0 && manager.lastTrashed.count == 2
                    && manager.games.map(\.slot) == ["posicao-1", "rapido"]
                self.refreshSaveMenus()
                checks["hostMenusUpdated"] = self.loadingMenu?.items.first(where: { $0.title.hasPrefix("Início da fase") })?.isEnabled == false
                    && self.loadingMenu?.items.first(where: { $0.tag == 2 })?.isEnabled == false
                    && self.loadingMenu?.items.first(where: { $0.tag == 1 })?.title.hasPrefix("Base renomeada") == true
                // Buttons come back once the sheet has detached (posicao-1 is selected again).
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
                checks["buttonsReenabled"] = manager.selected?.slot == "posicao-1" && manager.rename.isEnabled && manager.delete.isEnabled
                captureOwnWindow(manager.window, to: output.deletingPathExtension().appendingPathExtension("png")) { captured in
                    checks["managerCaptured"] = captured
                    close(manager)
                }
                }
            }
        }

        func close(_ manager: NativeSaveManager) {
            manager.close.performClick(nil)
            wait(50, "manager closed", { self.saveManager == nil && live.window.attachedSheet == nil }) {
                checks["closedRestoresMenus"] = enabled("Gerenciar partidas salvas…") && enabled("Salvar rápido") && enabled("Pausar")
                live.perform("live-status") { first in
                    DispatchQueue.main.asyncAfter(deadline: .now() + 1) { live.perform("live-status") { later in
                        info["framesAfterClose"] = [frames(first), frames(later)]
                        checks["playResumed"] = live.isRunning && frames(later) > frames(first) && later["live_stop"] is NSNull
                        checks["loadsRenamedSlot"] = false
                        self.loadSlot("posicao-1") { ok in
                            checks["loadsRenamedSlot"] = ok
                            finish()
                        }
                    } }
                }
            }
        }
    }
}
