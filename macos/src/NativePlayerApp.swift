import AppKit

/// Player edition start: import the player's CD once, then the original main
/// menu. No web inspector, no Python process, no research menus or controls.
extension AppDelegate {
    func startPlayerEdition() {
        directPlay = true
        makePlayerMenu()
        // The research inspector does not exist in this edition; this hidden,
        // never-shown placeholder only keeps the shared window-state code valid.
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 10, height: 10), styleMask: [.titled], backing: .buffered, defer: true)
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--player-smoke"), args.count > i + 1 { playerSmokeOutput = URL(fileURLWithPath: args[i + 1]) }
        NSApp.activate(ignoringOtherApps: true)
        if NativePlayerData.ready { beginPlayer() } else { showPlayerImport() }
    }

    func showPlayerImport() {
        let importer = playerImportWindow ?? NativePlayerImportWindow()
        playerImportWindow = importer
        importer.finished = { [weak self] _ in self?.beginPlayer() }
        importer.show()
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--import-from"), args.count > i + 1 { importer.start(URL(fileURLWithPath: args[i + 1])) }
    }

    func beginPlayer() {
        guard NativePlayerData.ready else { showPlayerImport(); return }
        beginGame(CampaignSelection(mission: CampaignSelection.menu, difficulty: NativeLiveWindow.initialDifficulty.rawValue))
        if let output = playerSmokeOutput {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self] in self?.startPlayerSmoke(output) }
        }
    }

    @objc func reimportCD() {
        let alert = NSAlert()
        alert.messageText = "Importar o CD novamente?"
        alert.informativeText = "Os dados do CD serão conferidos e copiados de novo. As partidas salvas não são alteradas."
        alert.addButton(withTitle: "Importar"); alert.addButton(withTitle: "Cancelar")
        guard alert.runModal() == .alertFirstButtonReturn else { return }
        nativeLive?.metal.releaseKeys()
        nativeLive?.perform("live-pause", completion: nil)
        showPlayerImport()
    }

    func makePlayerMenu() {
        let main = NSMenu()
        let appItem = NSMenuItem(); main.addItem(appItem)
        let appMenu = NSMenu(); appItem.submenu = appMenu
        appMenu.addItem(withTitle: "Sobre o Rising Lands", action: #selector(showAbout), keyEquivalent: "")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Ocultar o Rising Lands", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        appMenu.addItem(withTitle: "Encerrar o Rising Lands", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        let edit = NSMenuItem(); main.addItem(edit); let editMenu = NSMenu(title: "Editar"); edit.submenu = editMenu
        editMenu.addItem(withTitle: "Copiar", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "Colar", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: "Selecionar tudo", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        let gameItem = NSMenuItem(); main.addItem(gameItem)
        let gameMenu = NSMenu(title: "Jogo"); gameItem.submenu = gameMenu
        gameMenu.addItem(withTitle: "Pausar", action: #selector(togglePause), keyEquivalent: "p").target = self
        let speedItem = NSMenuItem(title: "Velocidade", action: nil, keyEquivalent: ""); let speedMenu = NSMenu(title: "Velocidade")
        speedItem.submenu = speedMenu; gameMenu.addItem(speedItem)
        for speed in NativeGameSpeed.allCases {
            let item = speedMenu.addItem(withTitle: speed.menuTitle, action: #selector(selectSpeed(_:)), keyEquivalent: ""); item.target = self; item.tag = speed.rawValue
        }
        speedMenu.addItem(.separator())
        speedMenu.addItem(withTitle: "Mais rápido", action: #selector(fasterSpeed), keyEquivalent: "]").target = self
        speedMenu.addItem(withTitle: "Mais devagar", action: #selector(slowerSpeed), keyEquivalent: "[").target = self
        let difficultyItem = NSMenuItem(title: "Dificuldade", action: nil, keyEquivalent: ""); let difficultyMenu = NSMenu(title: "Dificuldade")
        difficultyItem.submenu = difficultyMenu; gameMenu.addItem(difficultyItem)
        for difficulty in NativeDifficulty.allCases {
            let item = difficultyMenu.addItem(withTitle: difficulty.title, action: #selector(selectDifficulty(_:)), keyEquivalent: ""); item.target = self; item.tag = Int(difficulty.rawValue)
        }
        difficultyMenu.addItem(.separator())
        difficultyMenu.addItem(withTitle: "Vale a partir do próximo início de fase", action: nil, keyEquivalent: "").isEnabled = false
        gameMenu.addItem(.separator())
        for (title, action, key) in [("Salvar rápido", #selector(quickSave), "s"), ("Carregar rápido", #selector(quickLoad), "l")] {
            gameMenu.addItem(withTitle: title, action: action, keyEquivalent: key).target = self
        }
        for (title, save) in [("Salvar partida…", true), ("Carregar partida", false)] {
            let item = NSMenuItem(title: title, action: nil, keyEquivalent: ""); let slots = NSMenu(title: title)
            slots.delegate = self; slots.autoenablesItems = false; item.submenu = slots; gameMenu.addItem(item)
            if save { savingMenu = slots } else { loadingMenu = slots }
        }
        gameMenu.addItem(.separator())
        gameMenu.addItem(withTitle: "Opções…", action: #selector(showOptions), keyEquivalent: "o").target = self
        gameMenu.addItem(withTitle: "Configurar teclas…", action: #selector(showHotkeys), keyEquivalent: ",").target = self
        gameMenu.addItem(withTitle: "Atalhos modernos", action: #selector(toggleModernHotkeys(_:)), keyEquivalent: "").target = self
        gameMenu.addItem(withTitle: "Caminhos melhores", action: #selector(toggleModernPaths(_:)), keyEquivalent: "").target = self
        gameMenu.addItem(withTitle: "Importar o CD novamente…", action: #selector(reimportCD), keyEquivalent: "").target = self
        refreshSaveMenus()
        addAudioMenu(main)
        let windowItem = NSMenuItem(); main.addItem(windowItem); let windowMenu = NSMenu(title: "Janela"); windowItem.submenu = windowMenu
        windowMenu.addItem(withTitle: "Minimizar", action: #selector(NSWindow.miniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: "Fechar", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        let fullscreen = windowMenu.addItem(withTitle: "Tela cheia", action: #selector(toggleGameFullScreen), keyEquivalent: "f")
        fullscreen.target = self; fullscreen.keyEquivalentModifierMask = [.command, .control]
        windowMenu.addItem(withTitle: "Escala proporcional", action: #selector(fitGame), keyEquivalent: "").target = self
        windowMenu.addItem(withTitle: "Escala inteira", action: #selector(integerGame), keyEquivalent: "").target = self
        windowMenu.addItem(withTitle: "Suavizar imagem", action: #selector(toggleSmoothFilter), keyEquivalent: "").target = self
        NSApp.mainMenu = main
    }

    /// Áudio: sound effects and CD music, each with a mute toggle and four levels.
    /// Host presentation only (NativeAudioSettings); it applies at once and is stored
    /// with the other app preferences.
    func addAudioMenu(_ main: NSMenu) {
        let audioItem = NSMenuItem(); main.addItem(audioItem)
        let audioMenu = NSMenu(title: "Áudio"); audioItem.submenu = audioMenu
        for (mute, title, action, select) in [("Silenciar efeitos", "Volume dos efeitos", #selector(toggleEffectsMute), #selector(selectEffectsVolume(_:))),
                                              ("Silenciar música", "Volume da música", #selector(toggleMusicMute), #selector(selectMusicVolume(_:)))] {
            if !audioMenu.items.isEmpty { audioMenu.addItem(.separator()) }
            audioMenu.addItem(withTitle: mute, action: action, keyEquivalent: "").target = self
            let levelsItem = NSMenuItem(title: title, action: nil, keyEquivalent: ""); let levels = NSMenu(title: title)
            levelsItem.submenu = levels; audioMenu.addItem(levelsItem)
            for level in NativeAudioSettings.levels {
                let item = levels.addItem(withTitle: NativeAudioSettings.title(level), action: select, keyEquivalent: "")
                item.target = self; item.tag = Int((level * 100).rounded())
            }
        }
    }
    func changeAudio(_ change: (inout NativeAudioSettings) -> Void) {
        var value = nativeLive?.audioSettings ?? NativeLiveWindow.initialAudio
        change(&value)
        if let live = nativeLive { live.setAudio(value) } else if !NativeLiveWindow.diagnosticRun { value.store() }
    }
    @objc func toggleEffectsMute() { changeAudio { $0.effectsMuted.toggle() } }
    @objc func toggleMusicMute() { changeAudio { $0.musicMuted.toggle() } }
    @objc func selectEffectsVolume(_ item: NSMenuItem) { if let level = NativeAudioSettings.level(tag: item.tag) { changeAudio { $0.effectsVolume = level } } }
    @objc func selectMusicVolume(_ item: NSMenuItem) { if let level = NativeAudioSettings.level(tag: item.tag) { changeAudio { $0.musicVolume = level } } }
    /// Menu state of the Áudio items; nil for any other item.
    func validateAudioItem(_ item: NSMenuItem) -> Bool? {
        let value = nativeLive?.audioSettings ?? NativeLiveWindow.initialAudio
        switch item.action {
        case #selector(toggleEffectsMute): item.state = value.effectsMuted ? .on : .off
        case #selector(toggleMusicMute): item.state = value.musicMuted ? .on : .off
        case #selector(selectEffectsVolume(_:)): item.state = NativeAudioSettings.level(tag: item.tag) == value.effectsVolume ? .on : .off
        case #selector(selectMusicVolume(_:)): item.state = NativeAudioSettings.level(tag: item.tag) == value.musicVolume ? .on : .off
        default: return nil
        }
        return true
    }

    /// About panel text: what comes from where in this build. The original game
    /// data is the player's own CD; everything listed as modern did not exist in 1997.
    func playerAboutCredits(commit: String) -> NSAttributedString {
        var imported = "importados do CD original do jogador"
        if let data = try? Data(contentsOf: NativePlayerData.data.appendingPathComponent("importacao.json")),
           let receipt = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let stamp = receipt["importedAt"] as? String, let date = ISO8601DateFormatter().date(from: stamp) {
            let format = DateFormatter(); format.locale = Locale(identifier: "pt_BR"); format.dateStyle = .long; format.timeStyle = .none
            imported += " em \(format.string(from: date))"
            if let files = receipt["files"] as? Int { imported += " (\(files) arquivos)" }
        }
        let text = """
        Rising Lands para macOS, em português do Brasil.

        Jogo original: Rising Lands, Microïds, 1997. Este aplicativo é um projeto independente, sem vínculo com a Microïds.

        PROCEDÊNCIA
        Motor: reescrito em Rust por engenharia reversa do programa original e conferido contra ele em testes de comparação.
        Dados: gráficos, sons, fases e música são \(imported) e conferidos por SHA-256. O aplicativo não traz esses arquivos; algumas tabelas derivadas vêm cifradas e só abrem com o CD.
        Compilação: commit \(commit).

        RECURSOS DESTE PORT
        Não existiam no jogo de 1997: velocidade de jogo, dificuldades Fácil e Difícil (Normal é o jogo original), escalas de janela, filtro suave, atalhos configuráveis, atalhos modernos e o formato de salvamento.
        """
        return NSAttributedString(string: text, attributes: [.font: NSFont.systemFont(ofSize: NSFont.smallSystemFontSize), .foregroundColor: NSColor.labelColor])
    }

    static func menuTitles(_ menu: NSMenu?) -> [String] {
        (menu?.items ?? []).flatMap { [$0.title] + menuTitles($0.submenu) }
    }

    /// Player smoke: the shipped edition opens the original menu from imported
    /// data with no research chrome, then plays into MONDE01 by keyboard.
    func startPlayerSmoke(_ output: URL) {
        guard let live = nativeLive else { return }
        var checks = [String: Bool](), finished = false
        let resources = Bundle.main.resourceURL!
        let titles = Self.menuTitles(NSApp.mainMenu)
        checks["researchChromeAbsent"] = live.window.contentView?.subviews.allSatisfy { $0 === live.metal || $0 === live.stopPanel } == true
        checks["noInspectorOrPython"] = web == nil && engine == nil && !window.isVisible
            && !FileManager.default.fileExists(atPath: resources.appendingPathComponent("Python").path)
            && !FileManager.default.fileExists(atPath: resources.appendingPathComponent("preview").path)
        checks["researchMenusAbsent"] = !titles.contains { ["Inspecionar", "Nova partida…", "Jogar", "Menu original", "Música do CD"].contains($0) }
            && (titles.contains("Pausar") || titles.contains("Continuar")) // validateMenuItem retitles it while paused
            && ["Salvar rápido", "Configurar teclas…", "Importar o CD novamente…"].allSatisfy(titles.contains)
        checks["noInspectorUpdates"] = forwardedInspectorUpdates == 0
        checks["importedDataReady"] = NativePlayerData.ready && live.root.standardizedFileURL == NativePlayerData.data.standardizedFileURL
        func finish(_ error: String? = nil) {
            guard !finished else { return }; finished = true
            live.perform("live-pause") { value in
                var result: [String: Any] = ["passed": error == nil && checks.values.allSatisfy { $0 } && checks.count == 20, "checks": checks,
                                             "state": value, "nativeWindowVisible": live.window.isVisible, "edition": "player",
                                             "commit": Bundle.main.object(forInfoDictionaryKey: "RisingLandsSourceCommit") as? String ?? ""]
                if let error = error { result["error"] = error }
                let screenshot = output.deletingPathExtension().appendingPathExtension("png")
                captureOwnWindow(live.window, to: screenshot) { captured in
                    if !captured { result["passed"] = false }
                    do { try JSONSerialization.data(withJSONObject: result, options: [.prettyPrinted, .sortedKeys]).write(to: output, options: .atomic) }
                    catch { fputs("Player smoke report: \(error)\n", stderr) }
                    NSApp.terminate(nil)
                }
            }
        }
        func poll(_ left: Int, _ test: @escaping ([String: Any]) -> Bool, _ next: @escaping ([String: Any]) -> Void) {
            guard !finished else { return }
            live.perform("live-status") { value in
                if test(value) { next(value); return }
                if value["live_stop"] is [String: Any] { finish("Native stop: \(value["live_stop"]!)"); return }
                guard left > 0 else { finish("Player condition timed out"); return }
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { poll(left - 1, test, next) }
            }
        }
        func press(_ characters: String, _ keyCode: UInt16) -> Bool {
            live.window.makeKeyAndOrderFront(nil)
            guard live.window.makeFirstResponder(live.metal) else { return false }
            for type in [NSEvent.EventType.keyDown, .keyUp] {
                guard let event = NSEvent.keyEvent(with: type, location: .zero, modifierFlags: [], timestamp: 0, windowNumber: live.window.windowNumber, context: nil,
                                                   characters: characters, charactersIgnoringModifiers: characters, isARepeat: false, keyCode: keyCode) else { return false }
                live.window.sendEvent(event)
            }
            return true
        }
        func enter() -> Bool { press("\r", 36) }
        func item(_ title: String, in menu: NSMenu? = NSApp.mainMenu) -> NSMenuItem? {
            for candidate in menu?.items ?? [] {
                if candidate.title == title { return candidate }
                if let found = item(title, in: candidate.submenu) { return found }
            }
            return nil
        }
        /// The real menu path: the item's own action, then its own validation.
        func choose(_ title: String) -> NSMenuItem? {
            guard let found = item(title), let action = found.action, NSApp.sendAction(action, to: found.target, from: found) else { return nil }
            _ = validateMenuItem(found)
            return found
        }
        func exerciseMenus() {
            checks["speedMenu"] = NativeGameSpeed.allCases.allSatisfy { titles.contains($0.menuTitle) } && choose(NativeGameSpeed.double.menuTitle)?.state == .on && live.speed == .double
            checks["difficultyMenu"] = NativeDifficulty.allCases.allSatisfy { titles.contains($0.title) } && live.difficulty == .normal && choose("Difícil")?.state == .on && live.difficulty == .hard
            let layout = live.window.contentView as? NativeGameLayout
            let integer = choose("Escala inteira")?.state == .on && live.scaleMode == .integerFit
            layout?.layoutSubtreeIfNeeded()
            let multiple = (live.metal.frame.width * live.window.backingScaleFactor).truncatingRemainder(dividingBy: 800) == 0
            checks["scaleModes"] = integer && multiple && choose("Escala proporcional")?.state == .on && live.scaleMode == .continuousFit
            checks["smoothFilter"] = choose("Suavizar imagem")?.state == .on && live.smoothFilter
            live.showHotkeys()
            checks["hotkeyWindow"] = live.hotkeyPreferences?.window?.isVisible == true && titles.contains("Tela cheia")
            live.hotkeyPreferences?.window?.close()
            let commit = String((Bundle.main.object(forInfoDictionaryKey: "RisingLandsSourceCommit") as? String ?? "").prefix(12))
            let about = playerAboutCredits(commit: commit).string
            // Áudio: mute the effects (mixer output 0), music at 50% (the stream's level).
            let effects = choose("Silenciar efeitos")?.state == .on && live.audioSettings.effectsMuted
            let music = item("Volume da música").flatMap { menu in menu.submenu?.items.first { $0.tag == 50 } }
            if let music, let action = music.action { _ = NSApp.sendAction(action, to: music.target, from: music); _ = validateMenuItem(music) }
            checks["audioMenu"] = effects && music?.state == .on && live.audioSettings.musicVolume == 0.5 && live.audioSettings.effectsGain == 0
                && (live.cdMusic.map { $0.player.volume == 0.5 } ?? true)
            checks["aboutProvenance"] = !commit.isEmpty && ["PROCEDÊNCIA", "commit \(commit)", "SHA-256", "Microïds", "arquivos)"].allSatisfy(about.contains)
        }
        func mode(_ value: [String: Any]) -> UInt32 { value["original_shell_mode"] as? UInt32 ?? 0 }
        poll(150, { mode($0) == 2 }) { main in
            checks["originalMainAtLaunch"] = main["mission"] as? String == CampaignSelection.menu && !live.metal.isHidden && live.stopPanel.isHidden
            exerciseMenus()
            // Jogo > Opções… (Command-O) over the original main menu; Escape closes it.
            let options = item("Opções…")
            checks["optionsShortcut"] = options?.keyEquivalent == "o" && options?.keyEquivalentModifierMask == .command
            _ = choose("Opções…")
            live.waitOptions(open: true) { opened in
            checks["optionsOpenAtMain"] = opened
            checks["optionsClosedByEscape"] = press("\u{1b}", 53)
            live.waitOptions(open: false) { closed in
            checks["optionsClosedByEscape"] = checks["optionsClosedByEscape"] == true && closed
            checks["enterSingle"] = enter()
            poll(100, { mode($0) == 3 }) { _ in
                checks["enterNewGame"] = enter()
                poll(100, { mode($0) == 17 }) { _ in
                    checks["briefingThenPlay"] = enter()
                    poll(150, { $0["mission"] as? String == "MONDE01.ETA" && mode($0) == 0 && (($0["native_metrics"] as? [String: Any])?["frames"] as? UInt64 ?? 0) >= 40 }) { played in
                        checks["difficultyApplied"] = played["difficulty"] as? UInt32 == NativeDifficulty.hard.rawValue
                        finish()
                    }
                }
            }
            }
            }
        }
    }
}
