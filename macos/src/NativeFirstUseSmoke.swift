import AppKit

/// First-use UI smoke of the player edition (diagnostic only, never a product path).
///
///   RisingLands --data-dir CLEAN --save-directory CLEAN/Partidas-smoke \
///     --first-use-smoke REPORT --first-use-image BIN --first-use-cue CUE --first-use-wrong FILE
///
/// On a data folder with no import it drives the real import window: the open
/// dialog cancelled, a file that is not the CD refused with its message, the BIN
/// imported (the window refuses to close while copying), the original main menu,
/// MONDE01 started with Enter, then Jogo > Importar o CD novamente… over the running
/// stage (confirmation, the stage paused, the dialog cancelled again, then the CUE)
/// and the main menu of the new session.
/// `--import-from BIN --first-use-quit-at F` instead presses Sair once the copy
/// passed F (the interrupted import the next launch cleans up).
extension AppDelegate {
    func quitImportMidCopy(_ importer: NativePlayerImportWindow, at fraction: Double) {
        func check() {
            if importer.busy && importer.progress.doubleValue >= fraction { importer.quit.performClick(nil); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.05, execute: check)
        }
        check()
    }

    func startFirstUseSmoke(_ output: URL) {
        let args = CommandLine.arguments
        func value(_ name: String) -> URL? {
            guard let i = args.firstIndex(of: name), args.count > i + 1 else { return nil }
            return URL(fileURLWithPath: args[i + 1])
        }
        var checks = [String: Bool]()
        var info = [String: Any]()
        var finished = false
        let started = Date()
        let folder = NativePlayerData.folder, data = NativePlayerData.data
        func staging() -> [String] {
            ((try? FileManager.default.contentsOfDirectory(atPath: folder.path)) ?? []).filter { $0.hasPrefix("Dados.importando-") }
        }
        func receipt() -> [String: Any] {
            guard let bytes = try? Data(contentsOf: data.appendingPathComponent("importacao.json")),
                  let object = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any] else { return [:] }
            return object
        }
        func finish(_ error: String? = nil) {
            guard !finished else { return }; finished = true
            var result: [String: Any] = ["passed": error == nil && checks.count == 14 && checks.values.allSatisfy { $0 }, "checks": checks,
                                         "checksPassed": checks.values.filter { $0 }.count, "checksTotal": checks.count, "info": info,
                                         "edition": "player", "seconds": Date().timeIntervalSince(started),
                                         "commit": Bundle.main.object(forInfoDictionaryKey: "RisingLandsSourceCommit") as? String ?? ""]
            if let error { result["error"] = error }
            func write() {
                do { try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: output, options: .atomic) }
                catch { fputs("First-use smoke report: \(error)\n", stderr) }
                NSApp.terminate(nil)
            }
            if let live = nativeLive {
                captureOwnWindow(live.window, to: output.deletingPathExtension().appendingPathExtension("png")) { _ in write() }
            } else { write() }
        }
        /// Polls a host condition every 0.1 s.
        func wait(_ left: Int, _ label: String, _ test: @escaping () -> Bool, _ next: @escaping () -> Void) {
            guard !finished else { return }
            if test() { next(); return }
            guard left > 0 else { finish("Timed out: \(label)"); return }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { wait(left - 1, label, test, next) }
        }
        /// Polls the live session's original shell mode.
        func waitMode(_ live: NativeLiveWindow, _ left: Int, _ mode: UInt32, _ next: @escaping () -> Void) {
            guard !finished else { return }
            live.perform("live-status") { value in
                if value["original_shell_mode"] as? UInt32 == mode && value["mission"] as? String == CampaignSelection.menu { next(); return }
                if value["live_stop"] is [String: Any] { finish("Native stop: \(value["live_stop"]!)"); return }
                guard left > 0 else { finish("Timed out: original main menu"); return }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { waitMode(live, left - 1, mode, next) }
            }
        }
        guard let image = value("--first-use-image"), let cue = value("--first-use-cue"), let wrong = value("--first-use-wrong") else {
            finish("--first-use-image, --first-use-cue and --first-use-wrong are required"); return
        }
        info["interruptedImportsRemoved"] = interruptedImportsRemoved
        checks["noInterruptedImportLeft"] = staging().isEmpty
        let importer = playerImportWindow ?? NativePlayerImportWindow()
        playerImportWindow = importer
        importer.finished = { [weak self] _ in self?.beginPlayer() }
        importer.show()
        checks["importWindowAtLaunch"] = !NativePlayerData.ready && importer.window.isVisible && nativeLive == nil
            && importer.window.title == "Rising Lands — importar o CD original"

        /// "Escolher o CD…" opens the real open panel as a sheet; Cancelar closes it.
        func cancelDialog(_ label: String, _ next: @escaping (Bool) -> Void) {
            importer.choose.performClick(nil)
            wait(50, label + " dialog", { importer.window.attachedSheet is NSOpenPanel }) {
                let panel = importer.window.attachedSheet as! NSOpenPanel
                let shown = panel.prompt == "Importar" && panel.canChooseFiles && panel.canChooseDirectories
                importer.window.endSheet(panel, returnCode: .cancel)
                wait(30, label + " dialog closed", { importer.window.attachedSheet == nil }) {
                    next(shown && !importer.busy && importer.choose.isEnabled && importer.window.isVisible && staging().isEmpty)
                }
            }
        }
        cancelDialog("first") { cancelled in
            checks["dialogCancelled"] = cancelled && !FileManager.default.fileExists(atPath: data.path)
            importer.start(wrong)
            wait(100, "wrong file alert", { importer.alert != nil && importer.window.attachedSheet != nil }) {
                let alert = importer.alert!
                info["wrongFileMessage"] = alert.informativeText
                checks["wrongFileRefused"] = alert.messageText == "Não foi possível importar o CD"
                    && alert.informativeText.contains("não parece ser o CD do Rising Lands")
                    && alert.informativeText.contains("“\(wrong.lastPathComponent)”")
                importer.window.endSheet(alert.window)
                checks["wrongFileWroteNothing"] = !FileManager.default.fileExists(atPath: data.path) && staging().isEmpty && !importer.busy
                let importStart = Date()
                importer.start(image)
                wait(1200, "copy in progress", { importer.busy && importer.progress.doubleValue > 0.45 }) {
                    checks["closeRefusedWhileCopying"] = importer.windowShouldClose(importer.window) == false && !importer.choose.isEnabled
                    wait(3000, "game after import", { [weak self] in self?.nativeLive != nil }) { [weak self] in
                        guard let self, let first = self.nativeLive else { return }
                        info["importSeconds"] = Date().timeIntervalSince(importStart)
                        waitMode(first, 300, 2) {
                            let r = receipt()
                            info["firstReceipt"] = r
                            checks["mainMenuAfterImport"] = NativePlayerData.ready && !importer.window.isVisible && first.window.isVisible
                            checks["binImportedWithMusic"] = r["music"] as? Bool == true && r["source"] as? String == image.lastPathComponent
                                && staging().isEmpty
                            playStage(first) { reimport(first) }
                        }
                    }
                }
            }
        }
        /// Um jogador > Novo jogo > briefing > MONDE01 running, by the original Enter key.
        func playStage(_ live: NativeLiveWindow, _ next: @escaping () -> Void) {
            func enter() {
                live.window.makeKeyAndOrderFront(nil); _ = live.window.makeFirstResponder(live.metal)
                for type in [NSEvent.EventType.keyDown, .keyUp] {
                    if let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: live.window.windowNumber,
                                                    context: nil, characters: "\r", charactersIgnoringModifiers: "\r", isARepeat: false, keyCode: 36) {
                        live.window.sendEvent(event)
                    }
                }
            }
            func until(_ left: Int, _ label: String, _ test: @escaping ([String: Any]) -> Bool, _ then: @escaping () -> Void) {
                guard !finished else { return }
                live.perform("live-status") { value in
                    if test(value) { then(); return }
                    if value["live_stop"] is [String: Any] { finish("Native stop: \(value["live_stop"]!)"); return }
                    guard left > 0 else { finish("Timed out: \(label)"); return }
                    DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { until(left - 1, label, test, then) }
                }
            }
            func mode(_ v: [String: Any]) -> UInt32 { v["original_shell_mode"] as? UInt32 ?? 0 }
            enter()
            until(100, "single player", { mode($0) == 3 }) {
                enter()
                until(100, "briefing", { mode($0) == 17 }) {
                    enter()
                    until(150, "MONDE01 running", { $0["mission"] as? String == "MONDE01.ETA" && mode($0) == 0
                        && (($0["native_metrics"] as? [String: Any])?["frames"] as? UInt64 ?? 0) >= 40 }) {
                        checks["stageRunningBeforeReimport"] = true
                        next()
                    }
                }
            }
        }
        /// Jogo > Importar o CD novamente… over the running MONDE01.
        func reimport(_ first: NativeLiveWindow) {
            var confirmation = ""
            // runModal blocks: answer the confirmation from inside its modal loop.
            let answer = Timer(timeInterval: 0.3, repeats: false) { _ in
                if let modal = NSApp.modalWindow {
                    confirmation = modal.contentView.map { view in
                        func texts(_ v: NSView) -> [String] { ((v as? NSTextField).map { [$0.stringValue] } ?? []) + v.subviews.flatMap(texts) }
                        return texts(view).joined(separator: " | ")
                    } ?? ""
                }
                NSApp.stopModal(withCode: .alertFirstButtonReturn)
            }
            RunLoop.main.add(answer, forMode: .modalPanel)
            guard let item = findMenuItem("Importar o CD novamente…"), let action = item.action,
                  NSApp.sendAction(action, to: item.target, from: item) else { finish("Jogo > Importar o CD novamente… not found"); return }
            info["reimportConfirmation"] = confirmation
            checks["reimportConfirmed"] = confirmation.contains("Importar o CD novamente?") && importer.window.isVisible
            first.perform("live-status") { paused in
                func frames(_ v: [String: Any]) -> UInt64 { (v["native_metrics"] as? [String: Any])?["frames"] as? UInt64 ?? 0 }
                info["framesAtReimport"] = frames(paused)
                // Paused: no stage frame advances while the import window is up.
                DispatchQueue.main.asyncAfter(deadline: .now() + 1) { first.perform("live-status") { later in
                info["framesOneSecondLater"] = frames(later)
                checks["stagePausedForReimport"] = frames(later) == frames(paused) && frames(paused) >= 40 && later["mission"] as? String == "MONDE01.ETA"
                cancelDialog("reimport") { cancelled in
                    checks["reimportDialogCancelled"] = cancelled && NativePlayerData.ready && self.nativeLive === first
                    importer.start(cue)
                    wait(3000, "game after reimport", { [weak self] in self?.nativeLive != nil && self?.nativeLive !== first }) { [weak self] in
                        guard let self, let second = self.nativeLive else { return }
                        waitMode(second, 300, 2) {
                            let r = receipt()
                            info["secondReceipt"] = r
                            checks["cueReimported"] = NativePlayerData.ready && r["music"] as? Bool == true
                                && r["source"] as? String == image.lastPathComponent && staging().isEmpty && !importer.window.isVisible
                            let windows = NSApp.windows.filter { $0.isVisible && $0.contentView is NSView && $0 !== importer.window }
                            info["visibleWindows"] = windows.map { $0.title }
                            checks["oneGameAfterReimport"] = second.window.isVisible && !first.window.isVisible
                            DispatchQueue.main.asyncAfter(deadline: .now() + 2) { finish() }
                        }
                    }
                }
                } }
            }
        }
    }

    func findMenuItem(_ title: String, in menu: NSMenu? = NSApp.mainMenu) -> NSMenuItem? {
        for candidate in menu?.items ?? [] {
            if candidate.title == title { return candidate }
            if let found = findMenuItem(title, in: candidate.submenu) { return found }
        }
        return nil
    }
}
