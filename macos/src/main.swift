import AppKit
import WebKit
import CryptoKit

final class AppDelegate: NSObject, NSApplicationDelegate, WKScriptMessageHandler, WKNavigationDelegate, NSWindowDelegate, NSMenuDelegate, NSMenuItemValidation {
    var window: NSWindow!
    var web: WKWebView!
    var nativeLive: NativeLiveWindow?
    var campaignMenu:NativeCampaignMenu?
    var cdMusicDiscURL:URL?
    var chooserSmokeChecks=[String:Bool]()
    var savingMenu:NSMenu?
    var loadingMenu:NSMenu?
    var saveOperationInProgress=false
    var saveManager:NativeSaveManager?
    var testBOutput: URL?
    var testBRecordOnly=false
    var horizonOutput: URL?
    var horizonFinished=false
    var horizonLastProgress=0.0
    var engine: Process?
    var input: FileHandle?
    var outPipe: Pipe?
    var errPipe: Pipe?
    var outputBuffer = Data()
    var errorText = ""
    let ioQueue = DispatchQueue(label: "local.risinglands.preview.engine")
    var smokeOutput: URL?
    var screenshotOutput: URL?
    var finishedSmoke = false
    var smokeLifecycle = SmokeReadiness(launched: ProcessInfo.processInfo.systemUptime)
    var smokeTimer: Timer?
    var preparingSmoke = false
    var smokeMode = "exercise"
    var readySignal: [String:Any]?
    var readyDelayMs = 0
    var smokeProgress: [String:Any]?
    // The player edition reads only data imported from the player's own CD.
    let resourceRoot = risingPlayerEdition ? NativePlayerData.data : Bundle.main.resourceURL!.appendingPathComponent("preview", isDirectory: true)
    var playerImportWindow: NativePlayerImportWindow?
    var directPlay = false
    var forwardedInspectorUpdates = 0
    var playerSmokeOutput: URL?
    var saveManagerSmokeOutput: URL?
    var interruptedImportsRemoved = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        if risingPlayerEdition { startPlayerEdition(); return }
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--smoke-test"), args.count > i + 1 { smokeOutput = URL(fileURLWithPath: args[i+1]) }
        if let i = args.firstIndex(of: "--screenshot"), args.count > i + 1 { screenshotOutput = URL(fileURLWithPath: args[i+1]) }
        smokeMode = args.contains("--inspection") ? "inspection" : "exercise"
        if smokeOutput != nil, let i = args.firstIndex(of: "--smoke-ready-delay-ms"), args.count > i+1,
           let delay = Int(args[i+1]), (0...80000).contains(delay) { readyDelayMs = delay }
        if let i=args.firstIndex(of:"--test-b-session"),args.count>i+1 { testBOutput=URL(fileURLWithPath:args[i+1]) }
        makeMenu()
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        config.userContentController.add(self, name: "engine")
        let smokeConfiguration = "window.nativeSmoke={enabled:\(smokeOutput != nil),readyDelayMs:\(readyDelayMs)};"
        config.userContentController.addUserScript(WKUserScript(source:smokeConfiguration,injectionTime:.atDocumentStart,forMainFrameOnly:true))
        testBRecordOnly = testBOutput != nil && args.contains("--test-b-record-only")
        directPlay = smokeOutput == nil && testBOutput == nil && !args.contains("--inspection") && !args.contains("--native-benchmark") && !args.contains("--native-horizon")
        if directPlay { config.userContentController.addUserScript(WKUserScript(source:"window.nativeDefaultPlay=true;",injectionTime:.atDocumentStart,forMainFrameOnly:true)) }
        if testBOutput != nil,let i=args.firstIndex(of:"--test-b-plan"),args.count>i+1,
           let plan=try? String(contentsOfFile:args[i+1],encoding:.utf8) {
            config.userContentController.addUserScript(WKUserScript(source:"window.nativeTestB=\(plan);",injectionTime:.atDocumentStart,forMainFrameOnly:true))
        }
        web = WKWebView(frame: .zero, configuration: config)
        web.navigationDelegate = self
        web.setValue(false, forKey: "drawsBackground")
        window = NSWindow(contentRect: NSRect(x:0,y:0,width:1360,height:900), styleMask:[.titled,.closable,.miniaturizable,.resizable], backing:.buffered, defer:false)
        window.title = "RisingLands-ptbr — Sessão jogável"
        window.minSize = NSSize(width:1080,height:740)
        window.contentView = web
        window.delegate = self
        window.backgroundColor = NSColor(calibratedRed:0.067,green:0.086,blue:0.098,alpha:1)
        window.center()
        if !directPlay { window.makeKeyAndOrderFront(nil) }
        NSApp.activate(ignoringOtherApps:true)
        if window.isVisible { smokeLifecycle.windowVisible(ProcessInfo.processInfo.systemUptime) }
        do { try launchEngine() }
        catch { fail("Não foi possível iniciar o Python incluído: \(error.localizedDescription)"); return }
        let page = resourceRoot.appendingPathComponent(args.contains("--inspection") || directPlay ? "ui/playback.html" : "ui/index.html")
        web.loadFileURL(page, allowingReadAccessTo:resourceRoot)
        if directPlay {
            if args.contains("--play-stop-smoke") {
                // A real named engine stop without input: MONDE12's first frame still
                // reaches an unported logic rule (any exact named stop is accepted).
                beginGame(CampaignSelection(mission:"MONDE12.ETA",difficulty:1))
            }
            else if args.contains("--save-smoke") || args.contains("--autosave-smoke") || args.contains("--save-load-smoke") || args.contains("--campaign-smoke") || args.contains("--defeat-smoke") || args.contains("--pause-smoke") || args.contains("--ux-menu-trace") {beginGame(CampaignSelection(mission:"MONDE01.ETA",difficulty:1))}
            else if args.contains("--play-smoke") {
                // Direct stage chooser (Jogo → Nova partida…) remains its own diagnostic.
                showCampaignChooser()
                DispatchQueue.main.asyncAfter(deadline:.now()+0.25){self.exerciseCampaignChooser()}
            }
            // Normal launch opens the original main menu (MenuInit 2).
            else {beginGame(CampaignSelection(mission:CampaignSelection.menu,difficulty:1))}
        }
        if let index=args.firstIndex(of:"--save-smoke"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+4){[weak self] in self?.startSaveSmoke(output)}
        }
        if let index=args.firstIndex(of:"--autosave-smoke"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+4){[weak self] in self?.startAutosaveSmoke(output)}
        }
        if let index=args.firstIndex(of:"--save-load-smoke"),args.count>index+2 {
            let save=URL(fileURLWithPath:args[index+1]),output=URL(fileURLWithPath:args[index+2])
            DispatchQueue.main.asyncAfter(deadline:.now()+4){[weak self] in self?.startSaveLoadSmoke(save,output)}
        }
        if let index=args.firstIndex(of:"--campaign-smoke"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+4){[weak self] in self?.startCampaignSmoke(output)}
        }
        if let index=args.firstIndex(of:"--defeat-smoke"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+4){[weak self] in self?.startDefeatSmoke(output)}
        }
        if let index=args.firstIndex(of:"--menu-smoke"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+3){[weak self] in self?.startMenuSmoke(output)}
        }
        if let index=args.firstIndex(of:"--ux-menu-trace"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+4){[weak self] in self?.startUXMenuTrace(output)}
        }
        if let index=args.firstIndex(of:"--pause-smoke"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+4){[weak self] in self?.startPauseSmoke(output)}
        }
        if let index=args.firstIndex(of:"--play-stop-smoke"),args.count>index+1 {
            startPlayStopSmoke(URL(fileURLWithPath:args[index+1]))
        }
        if let index=args.firstIndex(of:"--play-smoke"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+5){[weak self] in self?.finishPlaySmoke(output)}
        }
        if let index=args.firstIndex(of:"--native-horizon"),args.count>index+1 {
            horizonOutput=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+1){[weak self] in self?.startNativeHorizon()}
        }
        if let index=args.firstIndex(of:"--native-benchmark"),args.count>index+1 {
            let output=URL(fileURLWithPath:args[index+1])
            DispatchQueue.main.asyncAfter(deadline:.now()+1){[weak self] in self?.startNativeBenchmark(output,args:args)}
        }
        if smokeOutput != nil {
            smokeTimer = Timer.scheduledTimer(withTimeInterval:0.5,repeats:true) { [weak self] _ in
                guard let self = self, !self.finishedSmoke else { return }
                let now = ProcessInfo.processInfo.systemUptime
                if self.window.isVisible { self.smokeLifecycle.windowVisible(now) }
                if let reason = self.smokeLifecycle.timeout(now) {
                    self.finishSmoke(["passed":false,"error":reason])
                }
            }
        }
    }

    func makeMenu() {
        let main = NSMenu()
        let appItem = NSMenuItem(); main.addItem(appItem)
        let appMenu = NSMenu(); appItem.submenu = appMenu
        appMenu.addItem(withTitle:"Sobre RisingLands-ptbr",action:#selector(showAbout),keyEquivalent:"")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle:"Encerrar RisingLands-ptbr",action:#selector(NSApplication.terminate(_:)),keyEquivalent:"q")
        let edit = NSMenuItem(); main.addItem(edit); let editMenu = NSMenu(title:"Editar"); edit.submenu = editMenu
        editMenu.addItem(withTitle:"Copiar",action:#selector(NSText.copy(_:)),keyEquivalent:"c")
        editMenu.addItem(withTitle:"Colar",action:#selector(NSText.paste(_:)),keyEquivalent:"v")
        editMenu.addItem(withTitle:"Selecionar tudo",action:#selector(NSText.selectAll(_:)),keyEquivalent:"a")
        let gameItem=NSMenuItem();main.addItem(gameItem)
        let gameMenu=NSMenu(title:"Jogo");gameItem.submenu=gameMenu
        gameMenu.addItem(withTitle:"Jogar",action:#selector(showGame),keyEquivalent:"1")
        gameMenu.addItem(withTitle:"Inspecionar",action:#selector(showInspector),keyEquivalent:"2")
        gameMenu.addItem(withTitle:"Menu original",action:#selector(showOriginalMenu),keyEquivalent:"").target=self
        gameMenu.addItem(withTitle:"Nova partida…",action:#selector(showCampaignChooser),keyEquivalent:"n")
        gameMenu.addItem(withTitle:"Opções…",action:#selector(showOptions),keyEquivalent:"o").target=self
        gameMenu.addItem(withTitle:"Configurar teclas…",action:#selector(showHotkeys),keyEquivalent:",").target=self
        gameMenu.addItem(withTitle:"Atalhos modernos",action:#selector(toggleModernHotkeys(_:)),keyEquivalent:"").target=self
        gameMenu.addItem(withTitle:"Caminhos melhores",action:#selector(toggleModernPaths(_:)),keyEquivalent:"").target=self
        gameMenu.addItem(.separator())
        gameMenu.addItem(withTitle:"Pausar",action:#selector(togglePause),keyEquivalent:"p").target=self
        let speedItem=NSMenuItem(title:"Velocidade",action:nil,keyEquivalent:"");let speedMenu=NSMenu(title:"Velocidade")
        speedItem.submenu=speedMenu;gameMenu.addItem(speedItem)
        for speed in NativeGameSpeed.allCases {
            let item=speedMenu.addItem(withTitle:speed.menuTitle,action:#selector(selectSpeed(_:)),keyEquivalent:"");item.target=self;item.tag=speed.rawValue
        }
        speedMenu.addItem(.separator())
        speedMenu.addItem(withTitle:"Mais rápido",action:#selector(fasterSpeed),keyEquivalent:"]").target=self
        speedMenu.addItem(withTitle:"Mais devagar",action:#selector(slowerSpeed),keyEquivalent:"[").target=self
        gameMenu.addItem(.separator())
        for (title,action,key) in [("Salvar rápido",#selector(quickSave),"s"),("Carregar rápido",#selector(quickLoad),"l")] {
            let item=gameMenu.addItem(withTitle:title,action:action,keyEquivalent:key);item.target=self
        }
        for (title,save) in [("Salvar partida…",true),("Carregar partida",false)] {
            let item=NSMenuItem(title:title,action:nil,keyEquivalent:"");let slots=NSMenu(title:title)
            slots.delegate=self;slots.autoenablesItems=false;item.submenu=slots;gameMenu.addItem(item)
            if save{savingMenu=slots}else{loadingMenu=slots}
        }
        addAutosaveMenu(gameMenu)
        refreshSaveMenus()
        let musicItem=NSMenuItem();main.addItem(musicItem)
        let musicMenu=NSMenu(title:"Música do CD");musicItem.submenu=musicMenu
        musicMenu.addItem(withTitle:"Selecionar imagem original do CD…",action:#selector(selectCDMusic),keyEquivalent:"").target=self
        for track in [2,3] {
            let item=musicMenu.addItem(withTitle:"Ouvir faixa original \(track) · manual",action:#selector(auditionCDMusic(_:)),keyEquivalent:"")
            item.target=self;item.tag=track
        }
        musicMenu.addItem(withTitle:"Parar música",action:#selector(stopCDMusic),keyEquivalent:"").target=self
        musicMenu.addItem(withTitle:"Silenciar música",action:#selector(muteCDMusic(_:)),keyEquivalent:"").target=self
        let windowItem = NSMenuItem(); main.addItem(windowItem); let windowMenu = NSMenu(title:"Janela"); windowItem.submenu = windowMenu
        windowMenu.addItem(withTitle:"Minimizar",action:#selector(NSWindow.miniaturize(_:)),keyEquivalent:"m")
        windowMenu.addItem(withTitle:"Fechar",action:#selector(NSWindow.performClose(_:)),keyEquivalent:"w")
        let fullscreen=windowMenu.addItem(withTitle:"Tela cheia",action:#selector(toggleGameFullScreen),keyEquivalent:"f")
        fullscreen.target=self;fullscreen.keyEquivalentModifierMask=[.command,.control]
        windowMenu.addItem(withTitle:"Escala proporcional",action:#selector(fitGame),keyEquivalent:"").target=self
        windowMenu.addItem(withTitle:"Escala inteira",action:#selector(integerGame),keyEquivalent:"").target=self
        NSApp.mainMenu = main
    }
    func makeNativeLive(validation:Bool,selection:CampaignSelection=CampaignSelection(mission:"MONDE01.ETA",difficulty:1)) throws -> NativeLiveWindow {
        let live=try NativeLiveWindow(root:resourceRoot,validationMode:validation,selection:selection)
        if let disc=cdMusicDiscURL{live.configureCDMusic(disc:disc)}
        // The hidden research inspector gets live updates only while it is shown
        // (or recording, or in the inspection/test modes): direct play never pays
        // for JSON plus WebKit calls 35-80 times per second.
        live.periodicSnapshots = !directPlay && !risingPlayerEdition
        live.update={ [weak self,weak live] value in
            guard let self=self,!self.directPlay || self.window.isVisible || live?.recording == true else{return}
            self.forwardedInspectorUpdates+=1;self.nativeReply(nil,value:value)
        }
        live.recordedEvent={ [weak self] value in
            if let data=try? JSONSerialization.data(withJSONObject:value),let json=String(data:data,encoding:.utf8){self?.web?.evaluateJavaScript("window.recordNativeInput(\(json));",completionHandler:nil)}
        }
        live.recordingAction={ [weak self] in self?.web?.evaluateJavaScript("window.toggleRecording();",completionHandler:nil) }
        live.exportAction={ [weak self] in self?.web?.evaluateJavaScript("window.exportRecording();",completionHandler:nil) }
        live.savesChanged={ [weak self] in self?.refreshSaveMenus() }
        return live
    }
    @objc func showOptions(){if let live=nativeLive{live.openOptions()}else{NSSound.beep()}}
    @objc func showHotkeys(){nativeLive?.showHotkeys()}
    @objc func showOriginalMenu(){beginGame(CampaignSelection(mission:CampaignSelection.menu,difficulty:1))}
    @objc func togglePause(){nativeLive?.toggle()}
    @objc func selectSpeed(_ item:NSMenuItem){if let speed=NativeGameSpeed(rawValue:item.tag){nativeLive?.setSpeed(speed)}}
    @objc func fasterSpeed(){if let live=nativeLive{live.setSpeed(live.speed.faster)}}
    @objc func slowerSpeed(){if let live=nativeLive{live.setSpeed(live.speed.slower)}}
    @objc func toggleGameFullScreen(){nativeLive?.toggleFullScreen()}
    @objc func fitGame(){nativeLive?.setScaleMode(.continuousFit)}
    @objc func toggleSmoothFilter(){if let live=nativeLive{live.setSmoothFilter(!live.smoothFilter)}}
    /// AoE2-style shortcuts that become ordinary original input (NativeModernHotkeys.swift).
    @objc func toggleModernHotkeys(_ item:NSMenuItem){
        let enabled = !NativeModernHotkeys.isEnabled()
        if let live=nativeLive{MainActor.assumeIsolated{live.setModernHotkeys(enabled)}}else{NativeModernHotkeys.store(enabled)}
    }
    /// Optional modern unit paths (engine rule bit 0). Applies to the menu world
    /// and to worlds created afterwards; a running stage or a save keeps its rules.
    @objc func toggleModernPaths(_ item:NSMenuItem){
        let enabled = !NativeModernPaths.isEnabled()
        if let live=nativeLive{MainActor.assumeIsolated{live.setModernPaths(enabled)}}else{NativeModernPaths.store(enabled)}
    }
    @objc func selectDifficulty(_ item:NSMenuItem){if let value=NativeDifficulty(rawValue:UInt32(item.tag)){nativeLive?.setDifficulty(value)}}
    @objc func integerGame(){nativeLive?.setScaleMode(.integerFit)}
    func menuNeedsUpdate(_ menu:NSMenu){refreshSaveMenus()}
    @objc func selectCDMusic(){
        guard let live=nativeLive,!live.validationMode else{return}
        let panel=NSOpenPanel();panel.title="Selecione RISING_LANDS_BR_.BIN do CD brasileiro original"
        panel.canChooseDirectories=false;panel.allowsMultipleSelection=false
        panel.beginSheetModal(for:live.window){answer in
            if answer == .OK,let url=panel.url{self.cdMusicDiscURL=url;live.configureCDMusic(disc:url)}
        }
    }
    @objc func auditionCDMusic(_ item:NSMenuItem){nativeLive?.cdMusic?.audition(track:item.tag)}
    @objc func stopCDMusic(){nativeLive?.cdMusic?.stopAutomatic()}
    @objc func muteCDMusic(_ item:NSMenuItem){
        guard let host=nativeLive?.cdMusic else{return}
        host.mute(!host.player.isMuted);item.state=host.player.isMuted ? .on:.off
    }
    func validateMenuItem(_ item:NSMenuItem)->Bool {
        if let audio=validateAudioItem(item){return audio}
        if item.action == #selector(selectAutosave(_:)) {
            item.state=(nativeLive?.autosaveInterval ?? NativeLiveWindow.initialAutosave).rawValue==item.tag ? .on:.off
            return true
        }
        if item.action == #selector(togglePause) {
            item.title=nativeLive?.isRunning == true ? "Pausar":"Continuar"
            return saveManager==nil && (nativeLive.map{!$0.validationMode && $0.window.isVisible} ?? false)
        }
        if item.action == #selector(manageSaves) {return saveManagerAvailable()}
        if item.action == #selector(reimportCD) {return saveManager==nil}
        if [#selector(selectSpeed(_:)),#selector(fasterSpeed),#selector(slowerSpeed)].contains(item.action) {
            guard let live=nativeLive,!live.validationMode else{item.state = .off;return false}
            if item.action == #selector(selectSpeed(_:)){item.state=live.speed.rawValue==item.tag && NativeGameSpeed(frameRate:live.gameFPS) != nil ? .on:.off}
            return item.action == #selector(fasterSpeed) ? live.speed != .quadruple : item.action == #selector(slowerSpeed) ? live.speed != .half : true
        }
        if item.action == #selector(showOptions) {return saveManager==nil && (nativeLive.map{!$0.validationMode && $0.window.isVisible} ?? false)}
        if item.action == #selector(toggleModernHotkeys(_:)) {item.state=NativeModernHotkeys.isEnabled() ? .on:.off;return true}
        if item.action == #selector(toggleModernPaths(_:)) {item.state=NativeModernPaths.isEnabled() ? .on:.off;return true}
        if item.action == #selector(selectDifficulty(_:)) {
            item.state=nativeLive?.difficulty.rawValue==UInt32(item.tag) ? .on:.off
            return nativeLive != nil
        }
        if [#selector(fitGame),#selector(integerGame)].contains(item.action) {
            let mode:NativeViewportPolicy.Mode=item.action == #selector(fitGame) ? .continuousFit:.integerFit
            item.state=nativeLive?.scaleMode==mode ? .on:.off
            return nativeLive != nil
        }
        if item.action == #selector(toggleSmoothFilter) {
            item.state=nativeLive?.smoothFilter==true ? .on:.off
            return nativeLive != nil
        }
        if [#selector(selectCDMusic),#selector(auditionCDMusic(_:)),#selector(stopCDMusic),#selector(muteCDMusic(_:))].contains(item.action){
            return nativeLive != nil && nativeLive?.validationMode==false
        }
        if item.action == #selector(quickSave) || item.action == #selector(quickLoad) {
            guard let live=nativeLive,!live.validationMode,!live.recording,!saveOperationInProgress else{return false}
            return item.action == #selector(quickSave) || FileManager.default.fileExists(atPath:live.saveURL("rapido").path)
        }
        return true
    }
    func slotNames()->[String:String] {
        guard let url=nativeLive?.saveDirectory.appendingPathComponent("nomes.plist"),let data=try? Data(contentsOf:url),let names=try? PropertyListSerialization.propertyList(from:data,format:nil) as? [String:String] else{return [:]}
        return names
    }
    func slotTitle(_ slot:Int)->String {
        let name=slotNames()[String(slot)] ?? "Posição \(slot)"
        guard let url=nativeLive?.saveURL("posicao-\(slot)"),let date=(try? FileManager.default.attributesOfItem(atPath:url.path))?[.modificationDate] as? Date else{return "\(name) · vazia"}
        return "\(name) · \(DateFormatter.localizedString(from:date,dateStyle:.short,timeStyle:.short))"
    }
    /// The periodic autosave slot in Carregar partida (it is not one of the original
    /// list's ten positions, so the in-game original lists do not show it).
    func autosaveTitle()->String {
        guard let url=nativeLive?.saveURL(NativeAutosaveInterval.slot),let date=(try? FileManager.default.attributesOfItem(atPath:url.path))?[.modificationDate] as? Date else{return "Salvamento automático · vazio"}
        let format=DateFormatter();format.dateFormat="dd/MM HH:mm"
        return "Salvamento automático · \(format.string(from:date))"
    }
    /// Jogo → Salvamento automático: Desligado / A cada 5, 10 ou 15 minutos.
    func addAutosaveMenu(_ gameMenu:NSMenu){
        let item=NSMenuItem(title:"Salvamento automático",action:nil,keyEquivalent:"");let menu=NSMenu(title:"Salvamento automático")
        item.submenu=menu;gameMenu.addItem(item)
        for interval in NativeAutosaveInterval.allCases {
            let choice=menu.addItem(withTitle:interval.title,action:#selector(selectAutosave(_:)),keyEquivalent:"");choice.target=self;choice.tag=interval.rawValue
        }
        menu.addItem(.separator())
        menu.addItem(withTitle:"Conta o tempo de jogo, só com a partida em andamento",action:nil,keyEquivalent:"").isEnabled=false
    }
    @objc func selectAutosave(_ item:NSMenuItem){
        guard let interval=NativeAutosaveInterval(rawValue:item.tag) else{return}
        if let live=nativeLive{live.setAutosave(interval)}else if !NativeLiveWindow.diagnosticRun{interval.store()}
    }
    @objc func loadAutosave(){loadSlot(NativeAutosaveInterval.slot)}
    func refreshSaveMenus(){
        let usable = nativeLive.map{!$0.validationMode && !$0.recording} ?? false
        for (menu,save) in [(savingMenu,true),(loadingMenu,false)] {
            guard let menu=menu else{continue};menu.removeAllItems()
            for slot in 1...5 {
                let item=menu.addItem(withTitle:slotTitle(slot),action:save ? #selector(namedSave(_:)):#selector(namedLoad(_:)),keyEquivalent:"")
                item.target=self;item.tag=slot
                item.isEnabled = !saveOperationInProgress && usable && (save || FileManager.default.fileExists(atPath:nativeLive!.saveURL("posicao-\(slot)").path))
            }
            if !save {
                menu.addItem(.separator())
                let auto=menu.addItem(withTitle:"Início da fase · salvamento automático",action:#selector(loadStageStart),keyEquivalent:"");auto.target=self
                auto.isEnabled = !saveOperationInProgress && usable && FileManager.default.fileExists(atPath:nativeLive!.saveURL("inicio-fase").path)
                let periodic=menu.addItem(withTitle:autosaveTitle(),action:#selector(loadAutosave),keyEquivalent:"");periodic.target=self
                periodic.isEnabled = !saveOperationInProgress && usable && FileManager.default.fileExists(atPath:nativeLive!.saveURL(NativeAutosaveInterval.slot).path)
            }
        }
    }
    func reportSaveError(_ error:Error){
        guard let live=nativeLive else{return}
        let alert=NSAlert();alert.messageText="Não foi possível concluir a operação";alert.informativeText=error.localizedDescription
        alert.addButton(withTitle:"OK");alert.beginSheetModal(for:live.window)
    }
    func saveSlot(_ slot:String,name:String?=nil,number:Int?=nil,completion:((Bool)->Void)?=nil){
        guard let live=nativeLive,!saveOperationInProgress else{completion?(false);return}
        saveOperationInProgress=true;refreshSaveMenus()
        live.saveSlot(slot){result in
            self.saveOperationInProgress=false
            switch result {
            case .success:
                if let name=name,let number=number {
                    var names=self.slotNames();names[String(number)]=name
                    do{try PropertyListSerialization.data(fromPropertyList:names,format:.xml,options:0).write(to:live.saveDirectory.appendingPathComponent("nomes.plist"),options:.atomic)}
                    catch{self.reportSaveError(error);completion?(false);self.refreshSaveMenus();return}
                }
                completion?(true)
            case .failure(let error):self.reportSaveError(error);completion?(false)
            }
            self.refreshSaveMenus()
        }
    }
    @objc func quickSave(){saveSlot("rapido")}
    @objc func quickLoad(){loadSlot("rapido")}
    @objc func loadStageStart(){loadSlot("inicio-fase")}
    @objc func namedLoad(_ item:NSMenuItem){loadSlot("posicao-\(item.tag)")}
    @objc func namedSave(_ item:NSMenuItem){
        guard let live=nativeLive,!saveOperationInProgress else{return}
        live.metal.releaseKeys()
        let alert=NSAlert();alert.messageText="Salvar partida · posição \(item.tag)"
        alert.informativeText=FileManager.default.fileExists(atPath:live.saveURL("posicao-\(item.tag)").path) ? "O salvamento desta posição será substituído. Dê um nome à partida:":"Dê um nome à partida:"
        alert.addButton(withTitle:"Salvar");alert.addButton(withTitle:"Cancelar")
        let field=NSTextField(string:slotNames()[String(item.tag)] ?? "MONDE01 · posição \(item.tag)")
        field.frame=NSRect(x:0,y:0,width:320,height:24);alert.accessoryView=field
        alert.beginSheetModal(for:live.window){response in
            guard response == .alertFirstButtonReturn else{return}
            let name=String(field.stringValue.trimmingCharacters(in:.whitespacesAndNewlines).prefix(80))
            self.saveSlot("posicao-\(item.tag)",name:name.isEmpty ? "Posição \(item.tag)":name,number:item.tag)
        }
    }
    func loadSlot(_ slot:String,completion:((Bool)->Void)?=nil){
        guard let live=nativeLive,!saveOperationInProgress else{completion?(false);return}
        saveOperationInProgress=true;refreshSaveMenus()
        live.loadSlot(slot){result in
            self.saveOperationInProgress=false;self.refreshSaveMenus()
            switch result{case .success:completion?(true);case .failure(let error):self.reportSaveError(error);completion?(false)}
        }
    }
    // Real AppKit actions plus production save/load paths. Engine continuation
    // equality is checked independently by the native consumer tests.
    func startSaveSmoke(_ output:URL){
        guard let live=nativeLive else{return}
        var checks=[String:Bool]()
        let game=NSApp.mainMenu?.items.first(where:{$0.submenu?.title=="Jogo"})?.submenu
        checks["quickSaveHotkey"]=game?.items.first(where:{$0.title=="Salvar rápido"})?.keyEquivalent=="s"
        checks["quickLoadHotkey"]=game?.items.first(where:{$0.title=="Carregar rápido"})?.keyEquivalent=="l"
        checks["fiveNamedSlots"]=savingMenu?.items.count==5
        checks["stageStartAutosave"]=FileManager.default.fileExists(atPath:live.saveURL("inicio-fase").path)
        func waitFor(_ condition:@escaping()->Bool,_ left:Int=100,_ next:@escaping()->Void){
            if condition(){next();return}
            guard left>0 else{checks["operationCompleted"]=false;finish();return}
            DispatchQueue.main.asyncAfter(deadline:.now()+0.1){waitFor(condition,left-1,next)}
        }
        func finish(){
            live.perform("live-pause"){value in
                checks["continuesAfterLoad"]=value["live_stop"] is NSNull && live.stoppedReason==nil
                let result:[String:Any]=["passed":checks.values.allSatisfy{$0},"checks":checks,"checksPassed":checks.values.filter{$0}.count,"checksTotal":checks.count,"commit":Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? "","nativeWindowVisible":live.window.isVisible,"state":value]
                do{try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:output,options:.atomic)}catch{fputs("Save smoke report: \(error)\n",stderr)}
                NSApp.terminate(nil)
            }
        }
        if let index=game?.items.firstIndex(where:{$0.title=="Salvar rápido"}){game?.performActionForItem(at:index)}
        waitFor({!self.saveOperationInProgress && FileManager.default.fileExists(atPath:live.saveURL("rapido").path)}){
            checks["quickSaveAction"]=true
            self.savingMenu?.performActionForItem(at:0)
            waitFor({live.window.attachedSheet != nil}){
                func descendants(_ view:NSView)->[NSView]{[view]+view.subviews.flatMap{descendants($0)}}
                let views=live.window.attachedSheet?.contentView.map{descendants($0)} ?? []
                let field=views.compactMap{$0 as? NSTextField}.first(where:{$0.isEditable})
                field?.stringValue="Teste de retomada"
                checks["namedSlotDialog"]=field != nil
                views.compactMap{$0 as? NSButton}.first(where:{$0.title=="Salvar"})?.performClick(nil)
                waitFor({!self.saveOperationInProgress && FileManager.default.fileExists(atPath:live.saveURL("posicao-1").path)}){
                    self.refreshSaveMenus()
                    checks["namedSlotLabel"]=self.loadingMenu?.items.first?.title.contains("Teste de retomada")==true
                    live.work.async{
                        let world=live.status().world
                        DispatchQueue.main.async{
                            if let index=game?.items.firstIndex(where:{$0.title=="Carregar rápido"}){game?.performActionForItem(at:index)}
                            waitFor({!self.saveOperationInProgress}){
                                live.work.async{
                                    let changed=live.status().world != world
                                    DispatchQueue.main.async{
                                        checks["quickLoadReplacesWorld"]=changed
                                        DispatchQueue.main.asyncAfter(deadline:.now()+0.5){
                                            self.loadSlot("inicio-fase"){ok in
                                                checks["loadStageStart"]=ok
                                                DispatchQueue.main.asyncAfter(deadline:.now()+0.5){finish()}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    /// Periodic autosave on a real MONDE01 (`--autosave-smoke REPORT --autosave-test-frames N`,
    /// disposable save folder): the menu, three autosaves N game frames apart at 4x, the
    /// slot and its single `.view`, the player's slots and the stage-start autosave left
    /// alone, nothing written while paused, then the slot loaded back through Jogo →
    /// Carregar partida at the saved frame, and play continuing with no stop.
    func startAutosaveSmoke(_ output:URL){
        guard let live=nativeLive else{return}
        var checks=[String:Bool](),detail=[String:Any](),finished=false
        let files=FileManager.default
        let slot=live.saveURL(NativeAutosaveInterval.slot),stage=live.saveURL("inicio-fase")
        let interval=NativeLiveWindow.autosaveTestFrames ?? 0
        func modified(_ url:URL)->Date?{(try? files.attributesOfItem(atPath:url.path))?[.modificationDate] as? Date}
        func views()->[String]{((try? files.contentsOfDirectory(atPath:live.saveDirectory.path)) ?? []).filter{$0.hasPrefix(NativeAutosaveInterval.slot+".rls.") && $0.hasSuffix(".view")}}
        let game=NSApp.mainMenu?.items.first(where:{$0.submenu?.title=="Jogo"})?.submenu
        let choices=game?.items.first(where:{$0.title=="Salvamento automático"})?.submenu?.items.filter{$0.action == #selector(selectAutosave(_:))} ?? []
        checks["autosaveMenu"]=choices.map(\.title)==["Desligado","A cada 5 minutos","A cada 10 minutos","A cada 15 minutos"]
        checks["offByDefault"]=live.autosaveInterval == .off && choices.first.map{validateMenuItem($0) && $0.state == .on}==true
        checks["testInterval"]=interval>0
        checks["emptyAtStart"] = !files.fileExists(atPath:slot.path) && autosaveTitle()=="Salvamento automático · vazio"
        let stageDate=modified(stage)
        checks["stageStartAutosave"]=stageDate != nil
        func finish(_ error:String?=nil){
            guard !finished else{return};finished=true
            live.perform("live-pause"){value in
                checks["continuesAfterLoad"]=value["live_stop"] is NSNull && live.stoppedReason==nil
                var result:[String:Any]=["passed":error==nil && checks.values.allSatisfy{$0},"checks":checks,"checksPassed":checks.values.filter{$0}.count,"checksTotal":checks.count,"commit":Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? "","nativeWindowVisible":live.window.isVisible,"state":value,"autosave":detail]
                if let error{result["error"]=error}
                do{try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:output,options:.atomic)}catch{fputs("Autosave smoke report: \(error)\n",stderr)}
                NSApp.terminate(nil)
            }
        }
        /// Worker values read on the game queue.
        func written(_ next:@escaping(Int,[UInt64])->Void){live.work.async{let count=live.autosaveCount,frames=live.autosaveFrames;DispatchQueue.main.async{next(count,frames)}}}
        func waitFor(_ condition:@escaping(@escaping(Bool)->Void)->Void,_ left:Int,_ what:String,_ next:@escaping()->Void){
            condition{ok in
                if ok{next();return}
                guard left>0 else{checks[what]=false;finish("timeout: "+what);return}
                DispatchQueue.main.asyncAfter(deadline:.now()+0.25){waitFor(condition,left-1,what,next)}
            }
        }
        live.setSpeed(.quadruple)
        // Three periodic autosaves: 3 x N frames at 4x (120 frames/s); budget 75 s.
        waitFor({done in written{count,_ in done(count>=3)}},300,"threeAutosaves"){
            written{count,frames in
                checks["threeAutosaves"]=count>=3
                let gaps=zip(frames.dropFirst(),frames).map{$0-$1}
                detail["frames"]=frames;detail["gaps"]=gaps;detail["intervalFrames"]=interval
                checks["gapsAreTheInterval"]=gaps.count>=2 && gaps.allSatisfy{$0>=interval && $0<interval+60}
                let data=try? Data(contentsOf:slot)
                let hash=data.map{SHA256.hash(data:$0).map{String(format:"%02x",$0)}.joined()} ?? ""
                checks["slotWithOneView"]=data != nil && views()==[NativeAutosaveInterval.slot+".rls.\(hash).view"]
                let others=((try? files.contentsOfDirectory(atPath:live.saveDirectory.path)) ?? []).filter{$0.hasPrefix("posicao-") || $0.hasPrefix("rapido")}
                checks["playerSlotsUntouched"]=others.isEmpty && modified(stage)==stageDate
                self.refreshSaveMenus()
                let entry=self.loadingMenu?.items.first(where:{$0.action == #selector(self.loadAutosave)})
                checks["loadEntry"]=entry?.isEnabled==true && entry?.title.range(of:#"^Salvamento automático · \d\d/\d\d \d\d:\d\d$"#,options:.regularExpression) != nil
                detail["loadEntry"]=entry?.title ?? ""
                // Paused: no frames, no autosave, for longer than one interval at 4x.
                live.perform("live-pause"){_ in
                    DispatchQueue.main.asyncAfter(deadline:.now()+0.5){
                        written{pausedCount,_ in
                            let pausedDate=modified(slot)
                            DispatchQueue.main.asyncAfter(deadline:.now()+Double(interval)/120+2){
                                written{laterCount,_ in
                                    checks["noAutosaveWhilePaused"]=laterCount==pausedCount && modified(slot)==pausedDate
                                    live.work.async{
                                        let world=live.status().world
                                        DispatchQueue.main.async{
                                            guard let entry,let index=self.loadingMenu?.index(of:entry) else{checks["loadsBack"]=false;finish("no load entry");return}
                                            self.loadingMenu?.performActionForItem(at:index)
                                            waitFor({done in done(!self.saveOperationInProgress)},120,"loadsBack"){
                                                live.work.async{
                                                    let s=live.status(),saved=live.autosaveFrames.last ?? 0
                                                    var mission=[CChar](repeating:0,count:16),difficulty:UInt32=0
                                                    macos_live_selection(live.driver,&mission,16,&difficulty)
                                                    let name=String(cString:mission)
                                                    DispatchQueue.main.async{
                                                        detail["loadedFrames"]=s.frames;detail["lastAutosaveFrame"]=saved;detail["mission"]=name
                                                        checks["loadsBack"]=s.world != world && name=="MONDE01.ETA"
                                                        // loadSlot restarts play right after its completion, so this read
                                                        // lands a few frames later: the loaded world starts at the saved
                                                        // frame and has run less than one second at 4x since.
                                                        checks["resumesAtSavedFrame"]=saved>0 && s.frames>=saved && s.frames<saved+120
                                                        DispatchQueue.main.asyncAfter(deadline:.now()+2){finish()}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    @objc func showInspector() { nativeLive?.periodicSnapshots=true;window.makeKeyAndOrderFront(nil) }
    @objc func showGame() {
        if nativeLive == nil {showCampaignChooser();return}
        web?.evaluateJavaScript("window.followNativeGame?.();",completionHandler:nil)
        nativeLive?.window.makeKeyAndOrderFront(nil)
    }
    @objc func showCampaignChooser() {
        do {
            if campaignMenu == nil {
                let menu=try NativeCampaignMenu(root:resourceRoot)
                menu.selected={ [weak self] selection in self?.beginGame(selection) }
                menu.cancelled={ [weak self] in if self?.nativeLive != nil{self?.showGame()} }
                campaignMenu=menu
            }
            nativeLive?.metal.releaseKeys()
            nativeLive?.perform("live-menu",completion:nil)
            campaignMenu?.show(canReturn:nativeLive != nil)
        }catch{fail("Não foi possível abrir a campanha: \(error.localizedDescription)")}
    }
    func beginGame(_ selection:CampaignSelection) {
        do {
            let previous=nativeLive
            let live=try makeNativeLive(validation:false,selection:selection);nativeLive=live
            campaignMenu?.window.orderOut(nil)
            previous?.closeCDMusic()
            previous?.perform("live-pause"){_ in previous?.window.orderOut(nil)}
            live.perform("live-reset"){value in
                guard value["live_stop"] is NSNull else{return}
                live.perform("live-step"){value in
                    if value["live_stop"] is NSNull {live.perform("live-play",completion:nil)}
                }
            }
            web?.evaluateJavaScript("window.followNativeGame?.();",completionHandler:nil)
            refreshSaveMenus()
        }catch{fail("Não foi possível iniciar a fase: \(error.localizedDescription)")}
    }
    // Real controls, same selection callback as a player. Diagnostic only.
    func exerciseCampaignChooser(){
        guard let menu=campaignMenu else{return}
        chooserSmokeChecks["chooserVisibleBeforeWorld"]=menu.window.isVisible && nativeLive==nil
        chooserSmokeChecks["all38FilesIn25Groups"]=menu.catalog.count==25 && menu.catalog.values.reduce(0){$0+$1.count}==38
        chooserSmokeChecks["ptbrDifficultyLabels"]=menu.difficulty.itemTitles==["Fácil","Normal","Difícil"]
        menu.stage.selectItem(withTag:17);menu.changeStage();menu.variant.selectItem(at:3);menu.difficulty.selectItem(at:2)
        chooserSmokeChecks["groupedVariantSelection"]=menu.selection.mission=="MONDE17C.ETA" && menu.selection.difficulty==2
        menu.window.displayIfNeeded()
        guard let index=CommandLine.arguments.firstIndex(of:"--play-smoke"),CommandLine.arguments.count>index+1 else{return}
        let url=URL(fileURLWithPath:CommandLine.arguments[index+1]).deletingPathExtension().appendingPathExtension("chooser.png")
        captureOwnWindow(menu.window,to:url){passed in
            self.chooserSmokeChecks["chooserCaptured"]=passed
            menu.stage.selectItem(withTag:1);menu.changeStage();menu.difficulty.selectItem(at:1)
            self.chooserSmokeChecks["singleVersionStage"]=menu.variant.numberOfItems==1 && !menu.variant.isEnabled
            menu.start.performClick(nil)
            // Real menu path: the play smoke runs at 4x (80 frames/s) over the TickClock.
            self.chooserSmokeChecks["speedMenuQuadruple"]=self.performGameMenu(speedTag:NativeGameSpeed.quadruple.rawValue) && self.nativeLive?.speed == .quadruple
        }
    }
    /// Smoke helper: dispatches an actual menu item exactly as a click would.
    @discardableResult func performGameMenu(title:String?=nil,speedTag:Int?=nil)->Bool {
        guard let game=NSApp.mainMenu?.items.first(where:{$0.submenu?.title=="Jogo"})?.submenu else{return false}
        let item:NSMenuItem?
        if let tag=speedTag {item=game.items.first(where:{$0.title=="Velocidade"})?.submenu?.items.first(where:{$0.tag==tag && $0.action==#selector(selectSpeed(_:))})}
        else {item=game.items.first(where:{$0.action==#selector(togglePause)})}
        guard let item,let menu=item.menu else{return false}
        menu.update()
        guard item.isEnabled,title==nil || item.title==title else{return false}
        menu.performActionForItem(at:menu.index(of:item));return true
    }
    /// Original main menu -> Um jogador -> Novo jogo -> briefing -> MONDE01 play,
    /// through actual Enter key NSEvents on the game view.
    func startMenuSmoke(_ output:URL){
        guard let live=nativeLive else{return}
        var checks=[String:Bool](),finished=false
        func finish(_ error:String?=nil){
            guard !finished else{return};finished=true
            live.perform("live-pause"){value in
                var result:[String:Any]=["passed":error==nil && checks.values.allSatisfy{$0} && checks.count==9,"checks":checks,"state":value,"nativeWindowVisible":live.window.isVisible,
                                         "commit":Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? ""]
                if let error=error{result["error"]=error}
                let screenshot=self.screenshotOutput ?? output.deletingPathExtension().appendingPathExtension("png")
                captureOwnWindow(live.window,to:screenshot){captured in
                    if !captured{result["passed"]=false}
                    do{try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:output,options:.atomic)}catch{fputs("Menu smoke report: \(error)\n",stderr)}
                    NSApp.terminate(nil)
                }
            }
        }
        func poll(_ left:Int,_ test:@escaping([String:Any])->Bool,_ next:@escaping([String:Any])->Void){
            guard !finished else{return}
            live.perform("live-status"){value in
                if test(value){next(value);return}
                if value["live_stop"] is [String:Any]{finish("Native stop: \(value["live_stop"]!)");return}
                guard left>0 else{finish("Menu condition timed out");return}
                DispatchQueue.main.asyncAfter(deadline:.now()+0.1){poll(left-1,test,next)}
            }
        }
        func press(_ code:UInt16,_ text:String)->Bool{
            live.window.makeKeyAndOrderFront(nil)
            guard live.window.makeFirstResponder(live.metal) else{return false}
            for type in [NSEvent.EventType.keyDown,.keyUp]{
                guard let event=NSEvent.keyEvent(with:type,location:.zero,modifierFlags:[],timestamp:0,windowNumber:live.window.windowNumber,context:nil,characters:text,charactersIgnoringModifiers:text,isARepeat:false,keyCode:code) else{return false}
                live.window.sendEvent(event)
            }
            return true
        }
        func enter()->Bool{press(36,"\r")}
        func escape()->Bool{press(53,"\u{1b}")}
        /// Down `count` times from the reset selection, then Enter (paced like key taps).
        func choose(_ count:Int,_ done:@escaping(Bool)->Void){
            if count==0{done(enter());return}
            let sent=press(125,"\u{F701}")
            DispatchQueue.main.asyncAfter(deadline:.now()+0.2){sent ? choose(count-1,done):done(false)}
        }
        func mode(_ value:[String:Any])->UInt32{value["original_shell_mode"] as? UInt32 ?? 0}
        func newGame(){
            checks["enterSingle"]=enter()
            poll(100,{mode($0)==3}){_ in
                checks["enterNewGame"]=enter()
                poll(100,{mode($0)==17}){_ in
                    checks["originalBriefing"] = !live.metal.isHidden
                    captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("briefing.png")){_ in
                        checks["enterStart"]=enter()
                        poll(150,{$0["mission"] as? String=="MONDE01.ETA" && mode($0)==0 && (($0["native_metrics"] as? [String:Any])?["frames"] as? UInt64 ?? 0)>=40}){_ in
                            finish()
                        }
                    }
                }
            }
        }
        // Main menu items 2 and 3 (engine E1/E2): the same session returns to main after each.
        poll(100,{mode($0)==2}){main in
            checks["originalMainAtLaunch"]=main["mission"] as? String==CampaignSelection.menu && !live.metal.isHidden && live.stopPanel.isHidden
            // A pointer resting on a menu item keeps it selected: park it outside the items for the arrows.
            live.benchmarkPointer(4,4)
            DispatchQueue.main.asyncAfter(deadline:.now()+0.3){choose(3){sent in
                poll(100,{mode($0)==10 && (live.credits?.shown ?? 0)>=5}){_ in
                    captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("credits.png")){captured in
                        checks["creditsAnimated"]=sent && captured
                        checks["creditsEscape"]=escape()
                        poll(100,{mode($0)==2}){_ in
                            choose(2){sent in
                                checks["introductionChosen"]=sent
                                poll(150,{_ in live.movieActive && live.defeatMovie.picturesPresented>=15}){_ in
                                    checks["introductionEscape"]=escape()
                                    poll(100,{mode($0)==2 && !live.movieActive}){_ in newGame()}
                                }
                            }
                        }
                    }
                }
            }}
        }
    }
    func finishPlaySmoke(_ output:URL) {
        guard let live=nativeLive else{return}
        // Pause and resume through the real Jogo menu (Command-P item): no frame
        // advances while paused, play resumes at the selected speed.
        func frames(_ value:[String:Any])->UInt64{(value["native_metrics"] as? [String:Any])?["frames"] as? UInt64 ?? 0}
        let pausedByMenu=performGameMenu(title:"Pausar")
        DispatchQueue.main.asyncAfter(deadline:.now()+0.3){live.perform("live-status"){paused in
            DispatchQueue.main.asyncAfter(deadline:.now()+0.4){live.perform("live-status"){still in
                let resumedByMenu=self.performGameMenu(title:"Continuar")
                DispatchQueue.main.asyncAfter(deadline:.now()+0.6){live.perform("live-status"){resumed in
                    self.chooserSmokeChecks["menuPauseStops"]=pausedByMenu && paused["native_running"] as? Bool==false
                    self.chooserSmokeChecks["pauseHoldsFrame"]=still["native_running"] as? Bool==false && (still["phase"] as? String)==(paused["phase"] as? String)
                    self.chooserSmokeChecks["menuResumeAdvances"]=resumedByMenu && resumed["native_running"] as? Bool==true && (resumed["native_metrics"] as? [String:Any])?["gameSpeed"] as? String=="4x" && frames(resumed)>=20
                    self.finishPlaySmokeReport(output,live:live)
                }}
            }}
        }}
    }
    func finishPlaySmokeReport(_ output:URL,live:NativeLiveWindow) {
        // Direct play with the inspector hidden: no live update reached WebKit.
        chooserSmokeChecks["noInspectorUpdatesInDirectPlay"]=directPlay && forwardedInspectorUpdates==0
        live.perform("live-pause"){value in
            var result=value
            result["commit"]=Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String
            result["nativeWindowVisible"]=live.window.isVisible
            result["inspectorWindowVisible"]=self.window.isVisible
            result["chooserChecks"]=self.chooserSmokeChecks
            result["passed"]=live.window.isVisible && !self.window.isVisible && !live.validationMode && (value["phase_index"] as? Int ?? 0)>=2*141 && value["live_stop"] is NSNull && self.chooserSmokeChecks.count==11 && self.chooserSmokeChecks.values.allSatisfy{$0} && self.campaignMenu?.window.isVisible==false
            let screenshot=self.screenshotOutput ?? output.deletingPathExtension().appendingPathExtension("png")
            live.capture(screenshot){readback in
                result["metalReadback"]=readback
                if readback["error"] != nil{result["passed"]=false}
                if let data=try? JSONSerialization.data(withJSONObject:result,options:[.sortedKeys,.prettyPrinted]){try? data.write(to:output,options:.atomic)}
                NSApp.terminate(nil)
            }
        }
    }
    func startCampaignSmoke(_ output:URL){
        guard let live=nativeLive else{return}
        var checks=[String:Bool](),finished=false
        func finish(_ error:String?=nil){
            guard !finished else{return};finished=true
            live.perform("live-pause"){value in
                var result:[String:Any]=["passed":error==nil && checks.values.allSatisfy{$0},"checks":checks,"checksPassed":checks.values.filter{$0}.count,"checksTotal":checks.count,"syntheticFixture":true,"humanWinClaim":false,"state":value,"nativeWindowVisible":live.window.isVisible,"commit":Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? ""]
                if let error=error{result["error"]=error}
                do{try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:output,options:.atomic)}catch{fputs("Campaign smoke report: \(error)\n",stderr)}
                NSApp.terminate(nil)
            }
        }
        func poll(_ left:Int,_ test:@escaping([String:Any])->Bool,_ next:@escaping([String:Any])->Void){
            guard !finished else{return}
            live.perform("live-status"){value in
                if test(value){next(value);return}
                guard left>0 else{finish("Campaign UI condition timed out");return}
                DispatchQueue.main.asyncAfter(deadline:.now()+0.1){poll(left-1,test,next)}
            }
        }
        loadSlot("posicao-1"){loaded in
            checks["fixtureLoadedThroughSavePath"]=loaded
            guard loaded else{finish("Synthetic victory fixture could not load");return}
            poll(100,{$0["campaign_outcome"] as? UInt32==1}){victory in
                checks["originalResultMode"]=victory["original_shell_mode"] as? UInt32==19
                checks["originalResultVisible"] = !live.metal.isHidden && live.stopPanel.isHidden
                checks["gameplaySuspended"]=victory["native_running"] as? Bool==false
                func enter()->Bool{
                    live.window.makeKeyAndOrderFront(nil)
                    guard live.window.makeFirstResponder(live.metal) else{return false}
                    for down in [true,false]{
                        guard let event=NSEvent.keyEvent(with:down ? .keyDown:.keyUp,location:.zero,modifierFlags:[],timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:live.window.windowNumber,context:nil,characters:"\r",charactersIgnoringModifiers:"\r",isARepeat:false,keyCode:36) else{return false}
                        live.window.sendEvent(event)
                    }
                    return true
                }
                live.window.displayIfNeeded()
                captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("png")){captured in
                    checks["resultCaptured"]=captured
                    checks["resultEnterRouted"]=enter()
                    poll(100,{$0["original_shell_mode"] as? UInt32==17}){briefing in
                        checks["originalBriefingVisible"] = !live.metal.isHidden && live.stopPanel.isHidden
                        checks["briefingRetainsSourceMission"]=briefing["mission"] as? String=="MONDE01.ETA"
                        checks["briefingGameplaySuspended"]=briefing["native_running"] as? Bool==false
                        captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("briefing.png")){captured in
                            checks["briefingCaptured"]=captured
                            checks["briefingEnterRouted"]=enter()
                            poll(100,{$0["mission"] as? String=="MONDE02.ETA" && $0["live_stop"] is NSNull}){next in
                                let beforeMusic=victory["cd_music"] as? [String:Any] ?? [:]
                                let nextMusic=next["cd_music"] as? [String:Any] ?? [:]
                                checks["nextOriginalMusicRequested"]=(nextMusic["requests"] as? Int ?? 0)>(beforeMusic["requests"] as? Int ?? 0)
                                checks["nextOriginalMusicStarted"]=(nextMusic["starts"] as? Int ?? 0)>(beforeMusic["starts"] as? Int ?? 0) && [2,3].contains(nextMusic["track"] as? Int ?? 0) && nextMusic["failures"] as? Int==0
                                checks["continueActionLoadsNext"]=true
                                checks["nextTitle"]=live.window.title.contains("MONDE02")
                                checks["resultCleared"]=live.stopPanel.isHidden && live.continueButton.isHidden
                                checks["nextAutosaveExists"]=FileManager.default.fileExists(atPath:live.saveURL("inicio-fase").path)
                                self.loadSlot("inicio-fase"){loaded in
                                    checks["nextAutosaveReloads"]=loaded
                                    poll(100,{(($0["native_metrics"] as? [String:Any])?["frames"] as? UInt64 ?? 0)>=20}){resumed in
                                        checks["nextStageContinues"]=resumed["mission"] as? String=="MONDE02.ETA" && resumed["live_stop"] is NSNull
                                        live.capture(output.deletingPathExtension().appendingPathExtension("metal.png")){readback in
                                            checks["nextMetalReadback"]=readback["error"]==nil
                                            finish()
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    func startDefeatSmoke(_ output:URL){
        guard let live=nativeLive else{return}
        var checks=[String:Bool](),finished=false
        let sourceHash=(try? Data(contentsOf:live.saveURL("posicao-1"))).map{SHA256.hash(data:$0).map{String(format:"%02x",$0)}.joined()} ?? ""
        var framesBeforeEscape = -1
        func finish(_ error:String?=nil){
            guard !finished else{return};finished=true
            live.perform("live-status"){value in
                var report:[String:Any]=["moviePicturesBeforeEscape":framesBeforeEscape,"passed":error==nil && checks.values.allSatisfy{$0},"checks":checks,"state":value,"sourceSaveSHA256":sourceHash,"fixture":"caller-provided saved defeat; provenance in harness","humanWinClaim":false,"nativeWindowVisible":live.window.isVisible,"commit":Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? ""]
                if let error=error{report["error"]=error}
                do{try JSONSerialization.data(withJSONObject:report,options:[.prettyPrinted,.sortedKeys]).write(to:output,options:.atomic)}catch{fputs("Defeat smoke report: \(error)\n",stderr)}
                NSApp.terminate(nil)
            }
        }
        func poll(_ left:Int){
            live.perform("live-status"){value in
                if value["original_shell_mode"] as? UInt32==2{
                    checks["originalMainAfterMovie"]=true
                    checks["originalMoviePath"]=(value["movie_path"] as? String)?.hasSuffix("\\events\\Over_fr.avi")==true
                    let movie=value["movie_status"] as? String ?? ""
                    // The host decodes the Cinepak/ADPCM original; PlayVideo's Escape key-up ends it (0x47c96e).
                    checks["originalMoviePicturesShown"]=framesBeforeEscape>=15
                    checks["originalMovieEndsOnEscape"]=movie=="Filme original interrompido com Esc"
                    checks["originalMainVisible"] = !live.metal.isHidden && live.stopPanel.isHidden
                    checks["gameplaySuspended"]=value["native_running"] as? Bool==false
                    let frame=(value["native_metrics"] as? [String:Any])?["frames"] as? UInt64
                    captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("png")){captured in
                        checks["mainCaptured"]=captured
                        DispatchQueue.main.asyncAfter(deadline:.now()+0.5){
                            live.perform("live-status"){after in
                                checks["sourceFrameRetained"]=(after["native_metrics"] as? [String:Any])?["frames"] as? UInt64==frame
                                live.capture(output.deletingPathExtension().appendingPathExtension("metal.png")){readback in
                                    checks["mainMetalReadback"]=readback["error"]==nil
                                    live.window.makeKeyAndOrderFront(nil)
                                    var sent=live.window.makeFirstResponder(live.metal)
                                    for down in [true,false]{
                                        if let event=NSEvent.keyEvent(with:down ? .keyDown:.keyUp,location:.zero,modifierFlags:[],timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:live.window.windowNumber,context:nil,characters:"\r",charactersIgnoringModifiers:"\r",isARepeat:false,keyCode:36){live.window.sendEvent(event)}else{sent=false}
                                    }
                                    checks["mainEnterRouted"]=sent
                                    func single(_ remaining:Int){
                                        live.perform("live-status"){state in
                                            if state["original_shell_mode"] as? UInt32==3{
                                                checks["originalSinglePlayerVisible"] = !live.metal.isHidden && live.stopPanel.isHidden
                                                checks["singlePlayerRetainsSourceFrame"]=(state["native_metrics"] as? [String:Any])?["frames"] as? UInt64==frame && state["native_running"] as? Bool==false
                                                // GPU completion precedes display presentation. Wait for
                                                // actual presented callbacks after mode3 before asking the
                                                // compositor for its window image.
                                                let started=ProcessInfo.processInfo.systemUptime
                                                func presented(_ attempts:Int){
                                                    // Menus present through the play presenter (or the one-shot path).
                                                    let count=live.uxPresentedTimes(since:started).count
                                                    if count>=2{
                                                        checks["singlePlayerPresented"]=true
                                                        captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("single.png")){captured in
                                                            checks["singlePlayerCaptured"]=captured
                                                            live.capture(output.deletingPathExtension().appendingPathExtension("single-metal.png")){picture in
                                                                checks["singlePlayerMetalReadback"]=picture["error"]==nil;finish()
                                                            }
                                                        }
                                                    }else if attempts>0{
                                                        DispatchQueue.main.asyncAfter(deadline:.now()+0.05){presented(attempts-1)}
                                                    }else{finish("Original SinglePlayer drawables were not presented · "+live.uxPresentationDiagnostics(since:started))}
                                                }
                                                presented(40)
                                            }else if remaining>0 && state["live_stop"] is NSNull{
                                                DispatchQueue.main.asyncAfter(deadline:.now()+0.1){single(remaining-1)}
                                            }else{finish("Original SinglePlayer entry did not complete")}
                                        }
                                    }
                                    single(100)
                                }
                            }
                        }
                    }
                }else if left>0 && value["live_stop"] is NSNull{
                    // One second of decoded pictures, then the original's Escape key-up.
                    if framesBeforeEscape<0 && live.defeatMovie.picturesPresented>=15{
                        framesBeforeEscape=live.defeatMovie.picturesPresented
                        live.window.makeKeyAndOrderFront(nil);live.window.makeFirstResponder(live.metal)
                        for down in [true,false]{
                            if let event=NSEvent.keyEvent(with:down ? .keyDown:.keyUp,location:.zero,modifierFlags:[],timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:live.window.windowNumber,context:nil,characters:"\u{1b}",charactersIgnoringModifiers:"\u{1b}",isARepeat:false,keyCode:53){live.window.sendEvent(event)}
                        }
                    }
                    DispatchQueue.main.asyncAfter(deadline:.now()+0.1){poll(left-1)}
                }else{finish("Original defeat did not return to main menu")}
            }
        }
        let movie=live.root.appendingPathComponent("movies/OVER_FR.AVI")
        checks["originalMovieBundled"]=(try? Data(contentsOf:movie)).map{SHA256.hash(data:$0).map{String(format:"%02x",$0)}.joined()}=="47e7bc81ce64579309902b75f6696824d45581e1c1b6851b63d0d610c152d37d"
        loadSlot("posicao-1"){loaded in
            checks["defeatSaveLoaded"]=loaded
            if loaded{poll(1000)}else{finish("Defeat fixture could not load")}
        }
    }
    func startPlayStopSmoke(_ output:URL) {
        guard let live=nativeLive else{return}
        // Test-only acceleration, no input: wait for the stage's real named stop.
        live.work.async{live.gameFPS=0}
        let deadline=ProcessInfo.processInfo.systemUptime+110
        var heartbeat=0,checking=false
        Timer.scheduledTimer(withTimeInterval:0.1,repeats:true){timer in
            heartbeat+=1
            guard !checking else{return};checking=true
            live.perform("live-status"){value in
                checking=false
                if value["live_stop"] is [String:Any]{
                    timer.invalidate()
                    self.verifyPlayStopSmoke(output,live:live,value:value,heartbeat:heartbeat,input:[:])
                }else if ProcessInfo.processInfo.systemUptime>deadline{
                    timer.invalidate()
                    self.finishPlayStopSmokeWithoutStop(output,live:live,value:value)
                }
            }
        }
    }
    /// No stage stops without input any more (2026-11-07 probe, 38 stages, 72,000 frames;
    /// commit 24d2dc34). Accepted only with proof that this run really played that far with
    /// no input and no stop; the build guard accepts only this exact reason, on this smoke.
    /// The stop panel is NOT exercised here. A real stop still takes verifyPlayStopSmoke.
    func finishPlayStopSmokeWithoutStop(_ output:URL,live:NativeLiveWindow,value:[String:Any]) {
        let metrics=value["native_metrics"] as? [String:Any]
        let frames=metrics?["frames"] as? UInt64 ?? 0
        // Either it is still playing after 72,000 frames, or the original game itself ended
        // the stage (an original victory/defeat outcome, e.g. MONDE12 with no input loses
        // once worlds use the zero allocation fill) with no engine stop.
        let outcome=(value["campaign_outcome"] as? NSNumber)?.intValue ?? 0
        let playedOn=value["native_running"] as? Bool==true&&frames>=72_000
        let originalEnd=value["native_running"] as? Bool==false&&outcome != 0&&frames>=1_200
        guard value["mission"] as? String=="MONDE12.ETA",metrics?["humanEvents"] as? UInt64==0,value["live_stop"] is NSNull,playedOn||originalEnd else{
            writePlayStopSmoke(output,result:["passed":false,"error":"No named engine stop within 110 seconds","snapshot":value]);return
        }
        captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("png")){captured in
            self.writePlayStopSmoke(output,result:["passed":captured,"declaredSkip":"No engine stop reachable on the consumer path","stopPanelExercised":false,"framesWithoutStop":frames,"snapshot":value])
        }
    }
    /// A real save as input (--save-load-smoke SAVE REPORT [--save-load-seconds N]): the save
    /// and its presentation companion are copied into the disposable smoke slots (the input is
    /// only read), loaded through the production load, then: the original pause menu when it
    /// was saved there, Jogo > Opções… over the pause, Escape back to play, N seconds of real
    /// mouse/key NSEvents on the game view, and the players table (alliances, sympathies) and
    /// rules compared before and after. Frame time is reported, not judged.
    func startSaveLoadSmoke(_ save:URL,_ output:URL){
        guard let live=nativeLive else{return}
        let args=CommandLine.arguments
        let seconds=args.firstIndex(of:"--save-load-seconds").flatMap{args.count>$0+1 ? Int(args[$0+1]):nil} ?? 120
        var checks=[String:Bool](),finished=false,info=[String:Any](),copied=[URL]()
        info["save"]=save.lastPathComponent;info["playSeconds"]=seconds
        func finish(_ error:String?=nil){
            guard !finished else{return};finished=true
            for url in copied{try? FileManager.default.removeItem(at:url)}
            live.perform("live-pause"){value in
                var result:[String:Any]=["passed":error==nil && !checks.isEmpty && checks.values.allSatisfy{$0},"checks":checks,"checksPassed":checks.values.filter{$0}.count,"checksTotal":checks.count,
                                         "info":info,"state":value,"nativeWindowVisible":live.window.isVisible,"commit":Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? ""]
                if let error{result["error"]=error}
                let screenshot=self.screenshotOutput ?? output.deletingPathExtension().appendingPathExtension("png")
                captureOwnWindow(live.window,to:screenshot){captured in
                    if !captured{result["passed"]=false}
                    do{try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:output,options:.atomic)}catch{fputs("Save-load smoke report: \(error)\n",stderr)}
                    NSApp.terminate(nil)
                }
            }
        }
        func poll(_ left:Int,_ test:@escaping([String:Any])->Bool,_ next:@escaping([String:Any])->Void){
            guard !finished else{return}
            live.perform("live-status"){value in
                if test(value){next(value);return}
                if value["live_stop"] is [String:Any]{finish("Native stop: \(value["live_stop"]!)");return}
                guard left>0 else{finish("Condition timed out");return}
                DispatchQueue.main.asyncAfter(deadline:.now()+0.1){poll(left-1,test,next)}
            }
        }
        func escape(_ transitions:[Bool]=[true,false])->Bool{
            live.window.makeKeyAndOrderFront(nil)
            guard live.window.makeFirstResponder(live.metal) else{return false}
            for down in transitions{guard self.sendPlayStopSmokeEscape(live,down:down) else{return false}}
            return true
        }
        func key(_ code:UInt16,down:Bool){
            live.window.makeKeyAndOrderFront(nil);_=live.window.makeFirstResponder(live.metal)
            if let event=NSEvent.keyEvent(with:down ? .keyDown:.keyUp,location:.zero,modifierFlags:[],timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:live.window.windowNumber,context:nil,characters:"",charactersIgnoringModifiers:"",isARepeat:false,keyCode:code){live.window.sendEvent(event)}
        }
        func frames(_ value:[String:Any])->UInt64{(value["native_metrics"] as? [String:Any])?["frames"] as? UInt64 ?? 0}
        func mode(_ value:[String:Any])->UInt32{value["original_shell_mode"] as? UInt32 ?? 0}
        func masks(_ state:[String:Any])->[Int]{(state["players"] as? [[String:Any]] ?? []).map{$0["allianceMask"] as? Int ?? -1}}
        // Deterministic input over the map area of the 800x600 picture (HUD excluded).
        var seed:UInt32=0x5eed1234,inputs=0
        func next(_ bound:Int)->Int{seed=seed &* 1664525 &+ 1013904223;return Int(seed>>8)%bound}
        func play(_ deadline:Double,_ resumed:[String:Any],_ loaded:[String:Any]){
            guard !finished else{return}
            if ProcessInfo.processInfo.systemUptime>=deadline{
                for code:UInt16 in [123,124,125,126]{key(code,down:false)}
                live.perform("live-status"){value in
                    let metrics=value["native_metrics"] as? [String:Any] ?? [:]
                    info["playFrames"]=frames(value)
                    info["frameTime"]=metrics.filter{$0.key.hasPrefix("frame")}
                    info["presentation"]=metrics.filter{["presented","displayFPS","minOneSecondFPS","gameFPS","gameSpeed","humanEvents"].contains($0.key)}
                    info["inputsSent"]=inputs
                    checks["noStopDuringPlay"]=value["live_stop"] is NSNull && live.stoppedReason==nil && live.stopPanel.isHidden
                    checks["inputReachedEngine"]=(metrics["humanEvents"] as? UInt64 ?? 0)>0
                    live.allianceState{after in
                        info["afterPlay"]=after
                        checks["playersTableRead"]=after["playersRead"] as? Int32==0 || after["playersRead"] as? Int==0
                        checks["alliancesKept"]=masks(after)==masks(loaded) && !masks(after).isEmpty
                        checks["rulesKept"]=after["rules"] as? Int==loaded["rules"] as? Int
                        checks["missionKept"]=after["mission"] as? String==loaded["mission"] as? String
                        finish()
                    }
                }
                return
            }
            live.perform("live-status"){value in
                if value["live_stop"] is [String:Any]{finish("Native stop during play: \(value["live_stop"]!)");return}
                let m=mode(value)
                if m==16{_=escape()}                                   // a click opened the original pause: resume
                else if m != 0{info["shellDuringPlay"]=Int(m);checks["noStopDuringPlay"]=true;finish();return}   // the stage ended (result screen)
                else{
                    switch next(8){
                    case 0,1,2:live.benchmarkClick(Double(40+next(560)),Double(40+next(380)));inputs+=1
                    case 3,4:live.benchmarkClick(Double(40+next(560)),Double(40+next(380)),right:true);inputs+=1
                    case 5:let code=UInt16(123+next(4));key(code,down:true);DispatchQueue.main.asyncAfter(deadline:.now()+0.4){key(code,down:false)};inputs+=1
                    default:live.benchmarkPointer(Double(40+next(560)),Double(40+next(380)))
                    }
                }
                DispatchQueue.main.asyncAfter(deadline:.now()+0.5){play(deadline,resumed,loaded)}
            }
        }
        func optionsThenResume(_ loaded:[String:Any]){
            checks["optionsMenuAction"]=NSApp.sendAction(#selector(self.showOptions),to:self,from:nil)
            poll(100,{mode($0)==16}){_ in
                live.waitOptions(open:true){opened in
                    checks["optionsOverPause"]=opened
                    checks["optionsEscape"]=escape()
                    live.waitOptions(open:false){closed in
                        checks["optionsEscape"]=checks["optionsEscape"]==true && closed
                        checks["escapeResumes"]=escape()
                        poll(100,{mode($0)==0 && $0["native_running"] as? Bool==true && frames($0)>=10}){resumed in
                            checks["resumedPlay"]=resumed["live_stop"] is NSNull
                            live.allianceState{state in
                                info["afterResume"]=state
                                checks["alliancesAfterResume"]=masks(state)==masks(loaded)
                                play(ProcessInfo.processInfo.systemUptime+Double(seconds),resumed,loaded)
                            }
                        }
                    }
                }
            }
        }
        poll(150,{frames($0)>=10 && $0["native_running"] as? Bool==true}){_ in
            do{
                let data=try Data(contentsOf:save)
                let sha=SHA256.hash(data:data).map{String(format:"%02x",$0)}.joined()
                let view=URL(fileURLWithPath:save.path+".\(sha).view")
                info["saveSHA256"]=sha
                guard FileManager.default.fileExists(atPath:view.path) else{finish("Presentation companion missing: \(view.lastPathComponent)");return}
                let slot=live.saveURL("entrada-smoke")
                try FileManager.default.createDirectory(at:live.saveDirectory,withIntermediateDirectories:true)
                let targets=[slot,URL(fileURLWithPath:slot.path+".\(sha).view")]
                for target in targets{try? FileManager.default.removeItem(at:target)}
                try FileManager.default.copyItem(at:save,to:targets[0]);copied.append(targets[0])
                try FileManager.default.copyItem(at:view,to:targets[1]);copied.append(targets[1])
            }catch{finish("Copy into the smoke slots: \(error.localizedDescription)");return}
            let started=ProcessInfo.processInfo.systemUptime
            live.loadSlot("entrada-smoke"){result in
                if case .failure(let error)=result{checks["loaded"]=false;finish("Load: \(error.localizedDescription)");return}
                checks["loaded"]=true;info["loadSeconds"]=ProcessInfo.processInfo.systemUptime-started
                live.allianceState{loaded in
                    info["afterLoad"]=loaded
                    checks["playersTableRead"]=loaded["playersRead"] as? Int32==0 || loaded["playersRead"] as? Int==0
                    // Every load plays on, a save made in the original pause list included
                    // (GameLoad then MenuInit 18, macos_live_load_resume).
                    poll(100,{mode($0)==16 || ($0["native_running"] as? Bool==true && frames($0)>=10)}){value in
                        checks["loadPlays"]=mode(value)==0 && value["native_running"] as? Bool==true
                        live.work.async{let resumed=live.loadedFromPause;DispatchQueue.main.async{info["savedInPause"]=resumed>0}}
                        captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("loaded.png")){captured in
                            checks["loadedCaptured"]=captured
                            optionsThenResume(loaded)
                        }
                    }
                }
            }
        }
    }
    /// In-game Escape -> original pause menu (MenuInit 16) -> Escape -> rl_shell_resume
    /// -> play continues, through actual key NSEvents on the game view.
    func startPauseSmoke(_ output:URL){
        guard let live=nativeLive else{return}
        var checks=[String:Bool](),finished=false
        func finish(_ error:String?=nil){
            guard !finished else{return};finished=true
            live.perform("live-pause"){value in
                var result:[String:Any]=["passed":error==nil && checks.values.allSatisfy{$0} && checks.count==14,"checks":checks,"state":value,"nativeWindowVisible":live.window.isVisible,
                                         "commit":Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? ""]
                if let error=error{result["error"]=error}
                let screenshot=self.screenshotOutput ?? output.deletingPathExtension().appendingPathExtension("png")
                captureOwnWindow(live.window,to:screenshot){captured in
                    if !captured{result["passed"]=false}
                    do{try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:output,options:.atomic)}catch{fputs("Pause smoke report: \(error)\n",stderr)}
                    NSApp.terminate(nil)
                }
            }
        }
        func poll(_ left:Int,_ test:@escaping([String:Any])->Bool,_ next:@escaping([String:Any])->Void){
            guard !finished else{return}
            live.perform("live-status"){value in
                if test(value){next(value);return}
                if value["live_stop"] is [String:Any]{finish("Native stop: \(value["live_stop"]!)");return}
                guard left>0 else{finish("Pause condition timed out");return}
                DispatchQueue.main.asyncAfter(deadline:.now()+0.1){poll(left-1,test,next)}
            }
        }
        func escape(_ transitions:[Bool]=[true,false])->Bool{
            live.window.makeKeyAndOrderFront(nil)
            guard live.window.makeFirstResponder(live.metal) else{return false}
            for down in transitions{guard self.sendPlayStopSmokeEscape(live,down:down) else{return false}}
            return true
        }
        func frames(_ value:[String:Any])->UInt64{(value["native_metrics"] as? [String:Any])?["frames"] as? UInt64 ?? 0}
        func mode(_ value:[String:Any])->UInt32{value["original_shell_mode"] as? UInt32 ?? 0}
        func playing(_ value:[String:Any])->Bool{mode(value)==0 && value["native_running"] as? Bool==true && frames(value)>=10}
        // From the open pause menu: SALVAR (item 1, row y 311..331) > position 1 of
        // list14 (row 0, y 286..306), which saves the paused world and resumes; then
        // Escape > CARREGAR (item 0) > position 1 of list12. The original continues a
        // loaded game (GameLoad 0x46b88d, then MenuInit 18 at 0x42c57a), so the load
        // must play on instead of reopening the pause in which the game was saved.
        func saveAndLoadThroughLists(){
            live.benchmarkClick(400,321)
            poll(100,{mode($0)==14}){_ in
                live.benchmarkClick(400,296)
                poll(150,playing){_ in
                    checks["pauseListSaveResumes"]=true
                    checks["escapeBeforeLoad"]=escape([true])
                    poll(100,{mode($0)==16}){_ in
                        checks["escapeBeforeLoad"]=checks["escapeBeforeLoad"]==true && escape([false])
                        live.benchmarkClick(400,296)
                        poll(100,{mode($0)==12}){_ in
                            live.benchmarkClick(400,296)
                            poll(200,playing){loaded in
                                checks["pauseListSaveLoadsPlaying"]=loaded["live_stop"] is NSNull && loaded["mission"] as? String=="MONDE01.ETA"
                                live.work.async{
                                    let resumed=live.loadedFromPause,saved=FileManager.default.fileExists(atPath:live.saveURL("posicao-1").path)
                                    DispatchQueue.main.async{checks["loadResumedSavedPause"]=resumed==1 && saved;finish()}
                                }
                            }
                        }
                    }
                }
            }
        }
        poll(100,{frames($0)>=10 && $0["native_running"] as? Bool==true}){_ in
            // The original polls Escape as a held key: hold it until the pause appears.
            checks["escapeInGame"]=escape([true])
            poll(100,{mode($0)==16}){paused in
                checks["escapeInGame"]=checks["escapeInGame"]==true && escape([false])
                checks["originalPauseMenu"]=paused["native_running"] as? Bool==false && !live.metal.isHidden && live.stopPanel.isHidden && live.stopReport.isEmpty
                checks["systemArrowInMenu"] = !live.metal.hidesSystemCursor
                captureOwnWindow(live.window,to:output.deletingPathExtension().appendingPathExtension("pause.png")){captured in
                    checks["pauseCaptured"]=captured
                    checks["escapeInMenu"]=escape()
                    poll(100,{mode($0)==0 && $0["native_running"] as? Bool==true && frames($0)>=10}){resumed in
                        checks["resumedPlay"]=resumed["mission"] as? String=="MONDE01.ETA"
                        checks["noStopAfterResume"]=resumed["live_stop"] is NSNull
                        // Jogo > Opções… (Command-O) in play: original pause first, then the screen.
                        checks["optionsShortcutInPlay"]=NSApp.sendAction(#selector(self.showOptions),to:self,from:nil)
                        poll(100,{mode($0)==16}){_ in
                            live.waitOptions(open:true){opened in
                                checks["optionsOverPause"]=opened
                                checks["optionsEscape"]=escape()
                                live.waitOptions(open:false){closed in
                                    checks["optionsEscape"]=checks["optionsEscape"]==true && closed
                                    saveAndLoadThroughLists()
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    func sendPlayStopSmokeEscape(_ live:NativeLiveWindow,down:Bool)->Bool {
        guard live.window.firstResponder === live.metal,
              let event=NSEvent.keyEvent(with:down ? .keyDown:.keyUp,location:.zero,modifierFlags:[],timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:live.window.windowNumber,context:nil,characters:"\u{1b}",charactersIgnoringModifiers:"\u{1b}",isARepeat:false,keyCode:53) else{return false}
        live.window.sendEvent(event)
        return live.metal.heldKeys.contains(27)==down
    }
    func writePlayStopSmoke(_ output:URL,result:[String:Any]) {
        var result=result
        result["commit"]=Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String
        result["nativeWindowVisible"]=nativeLive?.window.isVisible ?? false
        do{try JSONSerialization.data(withJSONObject:result,options:[.sortedKeys,.prettyPrinted]).write(to:output,options:.atomic)}
        catch{fputs("PlayStopSmokeWriteFailed: \(error)\n",stderr)}
        NSApp.terminate(nil)
    }
    func verifyPlayStopSmoke(_ output:URL,live:NativeLiveWindow,value:[String:Any],heartbeat:Int,input:[String:Any],activated:Bool=false) {
        if !activated {
            NSApp.activate(ignoringOtherApps:true)
            DispatchQueue.main.asyncAfter(deadline:.now()+0.15){self.verifyPlayStopSmoke(output,live:live,value:value,heartbeat:heartbeat,input:input,activated:true)}
            return
        }
        let stop=value["live_stop"] as? [String:Any] ?? [:]
        let detail=stop["detail"] as? String ?? ""
        let report=live.stopReport
        var checks=[String:Bool]()
        checks["stageWithoutInput"]=value["mission"] as? String=="MONDE12.ETA" && (value["native_metrics"] as? [String:Any])?["humanEvents"] as? UInt64==0
        checks["actualNamedStop"]=detail.range(of:"0x[0-9a-fA-F]{6,8}",options:.regularExpression) != nil
        checks["reportContainsExactDetail"] = !detail.isEmpty && report.contains(detail) && live.stopText.string==report
        checks["reportVisible"] = !live.stopPanel.isHidden && live.stopText.isSelectable && !live.stopText.isEditable
        checks["simulationStopped"]=value["native_running"] as? Bool == false && !live.button.isEnabled && !live.stepButton.isEnabled
        checks["mainRunLoopResponsive"]=heartbeat>1
        // A stopped game must show the system arrow again (the game cursor is frozen).
        checks["systemArrowRestored"] = !live.metal.hidesSystemCursor
        // Exercise the same copy action as the player, then restore the clipboard.
        let clipboard=NSPasteboard.general
        let previous=(clipboard.pasteboardItems ?? []).map{item -> NSPasteboardItem in
            let copy=NSPasteboardItem();for type in item.types{if let data=item.data(forType:type){copy.setData(data,forType:type)}};return copy
        }
        live.copyStopButton.performClick(nil)
        checks["copyReport"]=clipboard.string(forType:.string)==report
        clipboard.clearContents();if !previous.isEmpty{clipboard.writeObjects(previous)}
        let gameMenu=NSApp.mainMenu?.items.first(where:{$0.submenu?.title=="Jogo"})?.submenu
        gameMenu?.performActionForItem(at:1)
        checks["inspectionMenuResponsive"]=window.isVisible
        gameMenu?.performActionForItem(at:0)
        checks["gameMenuResponsive"]=live.window.isKeyWindow
        live.window.setContentSize(NSSize(width:640,height:550));live.window.contentView?.layoutSubtreeIfNeeded()
        checks["resizedReportVisible"]=live.stopPanel.frame.maxY<=live.metal.frame.minY && live.stopText.visibleRect.width>400
        let screenshot=output.deletingPathExtension().appendingPathExtension("png")
        do{
            guard let view=live.window.contentView,let bitmap=view.bitmapImageRepForCachingDisplay(in:view.bounds) else{throw NSError(domain:"StopPanelCaptureUnavailable",code:1)}
            view.cacheDisplay(in:view.bounds,to:bitmap)
            guard let data=bitmap.representation(using:.png,properties:[:]) else{throw NSError(domain:"StopPanelCaptureEncoding",code:1)}
            try data.write(to:screenshot,options:.atomic);checks["stopPanelCaptured"]=true
        }catch{checks["stopPanelCaptured"]=false}
        // Real clicks after the stop must preserve the first diagnostic and world.
        live.benchmarkClick(400,272);live.benchmarkPointer(500,300)
        live.perform("live-step"){after in
            checks["stopLatchedAfterInput"]=live.stopReport==report && NSDictionary(dictionary:after["live_stop"] as? [String:Any] ?? [:]).isEqual(to:stop) && after["phase"] as? String == value["phase"] as? String
            live.resetButton.performClick(nil)
            live.perform("live-status"){reset in
                checks["resetResponsive"]=reset["phase"] as? String == "state_s0" && reset["live_stop"] is NSNull && live.stopPanel.isHidden && live.button.isEnabled && live.stepButton.isEnabled
                // Reiniciar recreates the same stopped stage (MONDE12), not another one.
                checks["resetSameStage"]=reset["mission"] as? String == "MONDE12.ETA"
                live.perform("live-step"){stepped in
                    checks["freshWorldAdvances"]=stepped["phase_index"] as? Int == 0 && stepped["live_stop"] is NSNull
                    checks["freshStageWithoutHeldKeys"]=stepped["mission"] as? String == "MONDE12.ETA" && live.metal.heldKeys.isEmpty
                    self.writePlayStopSmoke(output,result:["passed":checks.values.allSatisfy{$0},"checks":checks,"checksPassed":checks.values.filter{$0}.count,"checksTotal":checks.count,"firstStop":value,"report":report,"mainRunLoopTicks":heartbeat,"screenshot":screenshot.path,"declaredInput":input,"afterReset":reset,"afterFreshStep":stepped])
                }
            }
        }
    }
    @objc func showAbout() {
        if risingPlayerEdition {
            let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
            let commit = String((Bundle.main.object(forInfoDictionaryKey: "RisingLandsSourceCommit") as? String ?? "").prefix(12))
            NSApp.orderFrontStandardAboutPanel(options:[.applicationName:"Rising Lands", .applicationVersion:version, .version:commit,
                .credits:playerAboutCredits(commit:commit)])
            return
        }
        NSApp.orderFrontStandardAboutPanel(options:[.applicationName:"RisingLands-ptbr", .applicationVersion:"0.1", .credits:NSAttributedString(string:"Sessão interativa de pesquisa reconstruída. Consulte a procedência e os limites dentro do aplicativo.")])
    }
    func launchEngine() throws {
        let process = Process()
        process.executableURL = Bundle.main.resourceURL!.appendingPathComponent("Python/bin/python3")
        process.arguments = ["-I","-B","-u",resourceRoot.appendingPathComponent("engine.py").path]
        process.currentDirectoryURL = resourceRoot
        var env = ProcessInfo.processInfo.environment
        env.removeValue(forKey:"PYTHONPATH"); env.removeValue(forKey:"PYTHONHOME")
        env.removeValue(forKey:"RISING_APP_ROOT")
        env["PYTHONDONTWRITEBYTECODE"] = "1"
        process.environment = env
        let stdinPipe = Pipe(); let stdoutPipe = Pipe(); let stderrPipe = Pipe()
        process.standardInput = stdinPipe; process.standardOutput = stdoutPipe; process.standardError = stderrPipe
        input = stdinPipe.fileHandleForWriting; outPipe = stdoutPipe; errPipe = stderrPipe
        stdoutPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else { handle.readabilityHandler = nil; return }
            self?.ioQueue.async { [weak self] in self?.consume(data) }
        }
        stderrPipe.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let data = handle.availableData
            guard !data.isEmpty else { handle.readabilityHandler = nil; return }
            let text = String(data:data,encoding:.utf8) ?? ""
            DispatchQueue.main.async { [weak self] in self?.errorText = String(((self?.errorText ?? "") + text).suffix(24000)) }
        }
        process.terminationHandler = { [weak self] p in
            DispatchQueue.main.async {
                guard let self = self, !self.finishedSmoke else { return }
                if p.terminationStatus != 0 { self.fail("O motor Python encerrou com código \(p.terminationStatus).\n\(self.errorText)") }
            }
        }
        engine = process
        try process.run()
    }
    func consume(_ data:Data) {
        outputBuffer.append(data)
        while let range = outputBuffer.range(of:Data([10])) {
            let line = outputBuffer.subdata(in:0..<range.lowerBound)
            outputBuffer.removeSubrange(0..<range.upperBound)
            guard let string = String(data:line,encoding:.utf8), (try? JSONSerialization.jsonObject(with:line)) != nil else { continue }
            DispatchQueue.main.async { [weak self] in
                self?.web?.evaluateJavaScript("window.receive(\(string));", completionHandler:{ _,error in
                    if let error = error { self?.errorText += "\nUI: \(error.localizedDescription)" }
                })
            }
        }
    }
    func userContentController(_ userContentController:WKUserContentController,didReceive message:WKScriptMessage) {
        if message.frameInfo.isMainFrame, smokeOutput != nil, let body = message.body as? [String:Any] {
            if body["action"] as? String == "smokeProgress" {
                smokeProgress = body
                return
            }
            if body["action"] as? String == "appInitializationFailed" {
                finishSmoke(["passed":false,"error":body["error"] as? String ?? "Initialization failed"])
                return
            }
            if body["action"] as? String == "appReady" {
                let expected = Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String
                guard body["protocol"] as? Int == 1, body["mode"] as? String == smokeMode,
                      body["worker"] as? Bool == true, body["assets"] as? Bool == true,
                      body["commit"] as? String == expected, expected != nil else {
                    finishSmoke(["passed":false,"error":"Invalid app readiness signal"]); return
                }
                readySignal = body
                smokeLifecycle.ready(ProcessInfo.processInfo.systemUptime)
                startSmokeIfReady()
                return
            }
        }
        if message.frameInfo.isMainFrame, let body = message.body as? [String:Any],
           body["action"] as? String == "capture", smokeOutput != nil,
           let shot = screenshotOutput, let label = body["label"] as? String,
           ["before", "after", "paint"].contains(label), let requestID = body["id"] {
            web.takeSnapshot(with:nil) { [weak self] image,error in
                var response:[String:Any] = ["id":requestID,"ok":false,"error":"Falha na captura nativa"]
                if let image = image, let tiff = image.tiffRepresentation,
                   let bitmap = NSBitmapImageRep(data:tiff), let png = bitmap.representation(using:.png,properties:[:]) {
                    let url = shot.deletingPathExtension().appendingPathExtension(label + ".png")
                    do {
                        try png.write(to:url)
                        let hash = SHA256.hash(data:png).map { String(format:"%02x",$0) }.joined()
                        response = ["id":requestID,"ok":true,"capture":["sha256":hash,"path":url.path]]
                    } catch { response["error"] = error.localizedDescription }
                }
                if let data = try? JSONSerialization.data(withJSONObject:response), let json = String(data:data,encoding:.utf8) {
                    self?.web?.evaluateJavaScript("window.receive(\(json));",completionHandler:nil)
                }
            }
            return
        }
        if message.frameInfo.isMainFrame,let body=message.body as? [String:Any],body["action"] as? String == "recordingIndicator",let active=body["active"] as? Bool {
            nativeLive?.setRecording(active);return
        }
        if message.frameInfo.isMainFrame, let body = message.body as? [String:Any],
           body["action"] as? String == "saveInput", let requestID = body["id"],
           let content = body["content"] as? [String:Any],
           let encoded = try? JSONSerialization.data(withJSONObject:content,options:[.prettyPrinted,.sortedKeys]),
           encoded.count <= 64*1024*1024 {
            let data=(body["text"] as? String).map{Data($0.utf8)} ?? (encoded + Data([10]))
            guard data.count<=64*1024*1024,(try? JSONSerialization.jsonObject(with:data)) is [String:Any] else{nativeReply(requestID,error:"Entrada exportada inválida");return}
            func save(_ url:URL) {
                do{try data.write(to:url,options:.atomic);self.nativeReply(requestID,value:["saved":true,"path":url.path,"sha256":SHA256.hash(data:data).map{String(format:"%02x",$0)}.joined()])}
                catch{self.nativeReply(requestID,error:error.localizedDescription)}
            }
            if let output=testBOutput {save(output.deletingPathExtension().appendingPathExtension("input.json"));return}
            // Automated smoke uses its explicitly declared evidence directory;
            // production always asks for a destination with NSSavePanel.
            if body["smokeEvidence"] as? Bool == true,let shot=smokeOutput{save(shot.deletingPathExtension().appendingPathExtension("input.json"));return}
            let panel = NSSavePanel()
            panel.title = "Exportar entrada gravada"
            panel.nameFieldStringValue = body["script"] as? Bool == true ? "monde01-input.json" : "monde01-macos-journal.json"
            panel.beginSheetModal(for:nativeLive?.window.isVisible == true ? nativeLive!.window:window) { [weak self] answer in
                if answer == .OK,let url=panel.url{save(url)}else{self?.nativeReply(requestID,value:["saved":false])}
            }
            return
        }
        if message.frameInfo.isMainFrame, let body=message.body as? [String:Any],body["action"] as? String == "playback",let operation=body["operation"] as? String {
            if operation.hasPrefix("live-") {
                do {
                    if nativeLive == nil {
                        nativeLive=try makeNativeLive(validation:smokeOutput != nil || testBRecordOnly)
                        nativeLive?.benchmarkInput = testBOutput != nil && !testBRecordOnly
                        web?.evaluateJavaScript("window.syncRecordingIndicator();",completionHandler:nil)
                    }
                    nativeLive?.window.makeKeyAndOrderFront(nil)
                    nativeLive?.perform(operation == "live-open" ? "live-reset":operation) { [weak self] value in self?.nativeReply(body["id"],value:value) }
                } catch { nativeReply(body["id"],error:"Falha nativa: \(error.localizedDescription)") }
                return
            }
            if operation == "test-b-gesture",testBOutput != nil,let live=nativeLive,let gesture=body["gesture"] as? [String:Any] {
                NSApp.activate(ignoringOtherApps:true);live.window.makeKeyAndOrderFront(nil);live.window.makeFirstResponder(live.metal)
                guard NSApp.isActive,live.window.isKeyWindow,live.window.firstResponder === live.metal else{nativeReply(body["id"],error:"TestBInputFocusUnavailable");return}
                let flags:NSEvent.ModifierFlags=(gesture["control"] as? Bool == true) ? [.control]:[]
                if let x=gesture["x"] as? Double,let y=gesture["y"] as? Double {
                    live.benchmarkClick(x,y,right:gesture["right"] as? Bool == true,clickCount:gesture["double"] as? Bool == true ? 2:1,flags:flags)
                }else if let key=gesture["keyCode"] as? Int,let down=gesture["down"] as? Bool {
                    if let event=NSEvent.keyEvent(with:down ? .keyDown:.keyUp,location:.zero,modifierFlags:flags,timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:live.window.windowNumber,context:nil,characters:"",charactersIgnoringModifiers:"",isARepeat:false,keyCode:UInt16(key)){live.window.sendEvent(event)}
                }else if let event=NSEvent.keyEvent(with:.flagsChanged,location:.zero,modifierFlags:flags,timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:live.window.windowNumber,context:nil,characters:"",charactersIgnoringModifiers:"",isARepeat:false,keyCode:59){live.window.sendEvent(event)}
                live.inputBarrier{[weak self] frame in self?.nativeReply(body["id"],value:["frame":frame,"focused":true])};return
            }
            if operation == "test-b-result",let output=testBOutput,let value=body["result"] as? [String:Any] {
                var result=value;result["commit"]=Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") ?? NSNull()
                if let data=try? JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]){try? data.write(to:output,options:.atomic)}
                engine?.terminate();NSApp.terminate(nil);return
            }
            if operation == "record-barrier" {
                if let live=nativeLive{live.inputBarrier{[weak self] frame in self?.nativeReply(body["id"],value:["frame":frame])}}
                else{nativeReply(body["id"],value:["frame":129])};return
            }
            if operation == "record-native-smoke",smokeOutput != nil,let live=nativeLive {
                recordNativeSmoke(body,live:live,deadline:ProcessInfo.processInfo.systemUptime+2);return
            }
            if operation == "native-capture",let live=nativeLive,let shot=screenshotOutput {
                live.capture(shot.deletingPathExtension().appendingPathExtension("metal.png")){[weak self] value in self?.nativeReply(body["id"],value:value)}
                return
            }
            if operation == "read",let live=nativeLive,live.window.isVisible,
               let address=body["address"] as? String,let raw=(address.lowercased().hasPrefix("0x") ? UInt32(address.dropFirst(2),radix:16):UInt32(address,radix:10)),let size=body["size"] as? Int,(1...256).contains(size) {
                live.read(raw,size){[weak self] value in if let error=value["error"] as? String{self?.nativeReply(body["id"],error:error)}else{self?.nativeReply(body["id"],value:value)}};return
            }
            if operation == "read",let live=nativeLive,live.window.isVisible {nativeReply(body["id"],error:"Endereço ou tamanho de leitura nativa inválido.");return}
            if window.isVisible,["open","reset","phase","frame"].contains(operation),let live=nativeLive {live.perform("live-pause",completion:nil);live.window.orderOut(nil)}
        }
        guard message.frameInfo.isMainFrame, let body = message.body as? [String:Any], let action = body["action"] as? String,
              ["init","reset","select","order","pause","play","speed","step","pump","state","economy","combat","interact","playback"].contains(action),
              let data = try? JSONSerialization.data(withJSONObject:body) else { return }
        ioQueue.async { [weak self] in
            do { try self?.input?.write(contentsOf:data + Data([10])) }
            catch { DispatchQueue.main.async { self?.fail("Falha na comunicação com o motor: \(error.localizedDescription)") } }
        }
    }
    // Runs the production native window, worker, Metal pipeline and visible drawables.
    func startNativeBenchmark(_ output:URL,args:[String]) {
        do {
            func argument(_ key:String,_ fallback:String)->String{if let index=args.firstIndex(of:key),args.count>index+1{return args[index+1]};return fallback}
            let mode=argument("--benchmark-mode","play"),duration=Double(argument("--benchmark-seconds","65")) ?? 0
            let rate=Double(argument("--game-fps","0")) ?? -1
            guard ["play","test"].contains(mode),duration>=60,duration<=180,rate>=0,rate<=120 else{throw NSError(domain:"InvalidNativeBenchmarkArguments",code:1)}
            let stage=try NativeBenchmarkArguments.parse(args,mode:mode)
            let live=try makeNativeLive(validation:mode=="test",selection:CampaignSelection(mission:stage.mission,difficulty:stage.difficulty));nativeLive=live
            // Timing is invalid when another app covers the game. This applies
            // only to the explicit bounded benchmark, never normal play.
            live.window.level = .floating
            live.window.orderFrontRegardless()
            var firstStop:[String:Any]?
            var gestures=[[String:Any]]()
            live.gameFPS=rate;live.update={value in
                if firstStop==nil,value["live_stop"] is [String:Any]{firstStop=value}
            };live.benchmarkInput=args.contains("--benchmark-orders")
            live.rateControl.selectItem(at:NativeGameSpeed(frameRate:rate)?.rawValue ?? -1)
            live.perform("live-reset"){_ in live.perform("live-step"){_ in live.perform("live-play"){_ in
                if live.benchmarkInput && args.contains("--benchmark-orders-selected"){
                    // Exact historical 60-fps fatigue workload: keep the moved actor selected.
                    // Observe the assigned frame without supplying a wall-clock frame to logic.
                    for (delay,gesture) in [(8.0,0),(8.2,1),(8.5,2)] {
                        DispatchQueue.main.asyncAfter(deadline:.now()+delay){
                            live.inputBarrier{frame in
                                gestures.append(["gesture":gesture,"observedNextFrame":frame])
                                switch gesture{
                                case 0:live.benchmarkClick(560,528)
                                case 1:live.benchmarkClick(624,432)
                                default:live.benchmarkPointer(400,272)
                                }
                            }
                        }
                    }
                }else if live.benchmarkInput{
                    // Anchor real AppKit gestures to the startup prefix, so uncapped
                    // play also receives orders before the later dependency horizon.
                    var gesture=0,lastGestureFrame=128
                    var inputTimer:Timer?
                    inputTimer=Timer.scheduledTimer(withTimeInterval:0.01,repeats:true){_ in
                        live.inputBarrier{frame in
                            guard gesture<4,frame>=lastGestureFrame+8 else{return}
                            gestures.append(["gesture":gesture,"observedNextFrame":frame])
                            switch gesture{
                            case 0:live.benchmarkClick(560,528)
                            case 1:live.benchmarkClick(624,432)
                            case 2:live.benchmarkClick(400,272,right:true)
                            default:live.benchmarkPointer(400,272);inputTimer?.invalidate()
                            }
                            gesture+=1;lastGestureFrame=frame
                        }
                    }
                }
                var lastPulse=ProcessInfo.processInfo.systemUptime,maxPulseGap=0.0,pulses=0
                let pulse=Timer.scheduledTimer(withTimeInterval:0.05,repeats:true){_ in let now=ProcessInfo.processInfo.systemUptime;maxPulseGap=max(maxPulseGap,now-lastPulse);lastPulse=now;pulses+=1}
                DispatchQueue.main.asyncAfter(deadline:.now()+duration){
                    live.perform("live-pause"){value in
                        pulse.invalidate()
                        var result=value;result["mainTimerMaxGapMS"]=maxPulseGap*1000;result["mainTimerPulses"]=pulses;result["mainTimerMaxDelayMS"]=max(0,maxPulseGap-0.05)*1000;result["commit"]=Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String
                        result["native_window_visible"]=live.window.isVisible
                        result["requestedSeconds"]=duration
                        result["benchmark_mission"]=stage.mission;result["benchmark_difficulty"]=stage.difficulty
                        result["benchmark_gestures"]=gestures
                        result["benchmark_workload"]=args.contains("--benchmark-orders-selected") ? "historical-selected-actor":"prefix-anchored-deselect"
                        result["first_stop_snapshot"]=firstStop ?? NSNull() as Any
                        result["screenMaximumFPS"]=live.window.screen?.maximumFramesPerSecond ?? 0
                        result["native_window_unoccluded"]=live.window.occlusionState.contains(.visible)
                        result["passed"]=live.window.isVisible && value["live_stop"] is NSNull
                        live.capture(output.deletingPathExtension().appendingPathExtension("png")){readback in
                            result["metal_readback"]=readback
                            if readback["error"] != nil{result["passed"]=false}
                            do{let data=try JSONSerialization.data(withJSONObject:result,options:[.sortedKeys,.prettyPrinted]);try data.write(to:output,options:.atomic)}catch{fputs("NativeBenchmarkWriteFailed: \(error)\n",stderr)}
                            self.engine?.terminate();NSApp.terminate(nil)
                        }
                    }
                }
            }}}
        }catch{
            if let data=try? JSONSerialization.data(withJSONObject:["passed":false,"error":error.localizedDescription]){try? data.write(to:output,options:.atomic)}
            NSApp.terminate(nil)
        }
    }
    func startNativeHorizon() {
        do {
            nativeLive=try NativeLiveWindow(root:resourceRoot,validationMode:true)
            nativeLive?.update={ [weak self] value in
                guard let self=self,!self.horizonFinished else{return}
                if value["live_stop"] is [String:Any] {self.finishNativeHorizon(value);return}
                let now=ProcessInfo.processInfo.systemUptime
                if now-self.horizonLastProgress>2,let path=self.horizonOutput,let data=try? JSONSerialization.data(withJSONObject:value,options:[.sortedKeys]) {
                    self.horizonLastProgress=now;try? data.write(to:path.appendingPathExtension("progress.json"),options:.atomic)
                }
            }
            nativeLive?.perform("live-reset"){[weak self] _ in
                self?.nativeLive?.work.async{[weak self] in guard let live=self?.nativeLive else{return};macos_live_audit_window(live.driver,18401)}
                self?.nativeLive?.perform("live-step"){[weak self] _ in self?.nativeLive?.perform("live-play",completion:nil)}
            }
            DispatchQueue.main.asyncAfter(deadline:.now()+1100){[weak self] in
                guard let self=self,!self.horizonFinished else{return}
                self.nativeLive?.perform("live-pause"){value in var value=value;value["horizon_pending"]=true;self.finishNativeHorizon(value)}
            }
        } catch {
            if let path=horizonOutput,let data=try? JSONSerialization.data(withJSONObject:["passed":false,"error":error.localizedDescription]){try? data.write(to:path,options:.atomic)}
            NSApp.terminate(nil)
        }
    }
    func finishNativeHorizon(_ value:[String:Any]) {
        guard !horizonFinished,let path=horizonOutput,let live=nativeLive else{return};horizonFinished=true
        live.capture(path.deletingPathExtension().appendingPathExtension("metal.png")){[weak self] readback in
            guard let self=self else{return};var result=value
            result["commit"]=Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String
            result["native_window_visible"]=live.window.isVisible;result["metal_readback"]=readback
            result["passed"]=(value["verified_phases"] as? UInt64 ?? 0)>=18401 && (value["verified_surfaces"] as? UInt64 ?? 0)>=9200 && value["live_stop"] is [String:Any] && readback["error"]==nil && live.window.isVisible
            if let data=try? JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]){try? data.write(to:path,options:.atomic)}
            NSApp.terminate(nil)
        }
    }
    // A visible window is not necessarily key while the user works in another
    // app. AppKit can discard synthetic clicks sent to an inactive application.
    // Wait for activation before dispatch; retain the existing smoke budget.
    func recordNativeSmoke(_ body:[String:Any],live:NativeLiveWindow,deadline:Double) {
        NSApp.activate(ignoringOtherApps:true)
        live.window.makeKeyAndOrderFront(nil)
        guard NSApp.isActive && live.window.isKeyWindow else {
            guard ProcessInfo.processInfo.systemUptime<deadline else {
                nativeReply(body["id"],error:"A janela Metal não obteve foco para validar a entrada.");return
            }
            DispatchQueue.main.asyncAfter(deadline:.now()+0.01){[weak self] in self?.recordNativeSmoke(body,live:live,deadline:deadline)}
            return
        }
        guard live.window.makeFirstResponder(live.metal),live.window.firstResponder === live.metal else {
            nativeReply(body["id"],error:"A superfície Metal não recebeu foco para validar a entrada.");return
        }
        let focus:[String:Any]=["appActive":NSApp.isActive,"windowKey":live.window.isKeyWindow,"gameFirstResponder":live.window.firstResponder === live.metal]
        let down=body["stage"] as? String == "press"
        if !down,let event=NSEvent.keyEvent(with:.flagsChanged,location:.zero,modifierFlags:[],timestamp:0,windowNumber:live.window.windowNumber,context:nil,characters:"",charactersIgnoringModifiers:"",isARepeat:false,keyCode:59){live.window.sendEvent(event)}
        if down {
            let p=live.metal.convert(NSPoint(x:live.metal.bounds.width/2,y:live.metal.bounds.height*(1-272.0/600.0)),to:nil)
            for type in [NSEvent.EventType.leftMouseDown,.leftMouseUp]{if let event=NSEvent.mouseEvent(with:type,location:p,modifierFlags:.control,timestamp:0,windowNumber:live.window.windowNumber,context:nil,eventNumber:1,clickCount:1,pressure:1){live.window.sendEvent(event)}}
        }
        live.inputBarrier{[weak self] frame in self?.nativeReply(body["id"],value:["frame":frame,"recordingIndicator":live.recordButton.title,"inputFocus":focus])}
    }
    func nativeReply(_ id:Any?,value:[String:Any]=[:],error:String?=nil) {
        var value=value;value["commit"]=Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String
        var response:[String:Any]=["ok":error==nil,"playback":value]
        if let id=id{response["id"]=id}else{response["nativeUpdate"]=true}
        if let error=error{response["error"]=error}
        if let data=try? JSONSerialization.data(withJSONObject:response),let json=String(data:data,encoding:.utf8){web?.evaluateJavaScript("window.receive(\(json));",completionHandler:nil)}
    }
    func webView(_ webView:WKWebView,decidePolicyFor navigationAction:WKNavigationAction,decisionHandler:@escaping(WKNavigationActionPolicy)->Void) {
        guard let url = navigationAction.request.url else { decisionHandler(.cancel); return }
        decisionHandler(url.isFileURL && url.standardizedFileURL.path.hasPrefix(resourceRoot.path + "/") ? .allow : .cancel)
    }
    func webView(_ webView:WKWebView,didFinish navigation:WKNavigation!) {
        guard smokeOutput != nil else { return }
        smokeLifecycle.documentLoaded(ProcessInfo.processInfo.systemUptime)
        startSmokeIfReady()
    }
    func webViewWebContentProcessDidTerminate(_ webView:WKWebView) {
        fail("Web content process terminated")
    }
    func captureSmokeFrame(_ label:String, completion:@escaping()->Void) {
        guard let shot = screenshotOutput else { completion(); return }
        let url = label == "final" ? shot : shot.deletingPathExtension().appendingPathExtension(label + ".png")
        if label != "final" && FileManager.default.fileExists(atPath:url.path) { completion(); return }
        web.takeSnapshot(with:nil) { image,error in
            if let image = image, let tiff = image.tiffRepresentation,
               let bitmap = NSBitmapImageRep(data:tiff), let png = bitmap.representation(using:.png,properties:[:]) {
                try? png.write(to:url,options:.atomic)
            }
            completion()
        }
    }
    func startSmokeIfReady() {
        guard smokeOutput != nil, !finishedSmoke, !preparingSmoke,
              smokeLifecycle.readyAt != nil, smokeLifecycle.documentLoadedAt != nil else { return }
        preparingSmoke = true
        captureSmokeFrame("before") { [weak self] in
            guard let self = self, !self.finishedSmoke else { return }
            let now = ProcessInfo.processInfo.systemUptime
            guard self.smokeLifecycle.startChecks(now) else {
                self.finishSmoke(["passed":false,"error":self.smokeLifecycle.timeout(now) ?? "Smoke readiness incomplete"]); return
            }
            self.web.callAsyncJavaScript("return await window.runSmokeTests().catch(e=>({passed:false,error:String(e),stack:e.stack,diagnostics:window.smokeDiagnostics?window.smokeDiagnostics():{}}));",arguments:[:],in:nil,in:.page) { [weak self] result in
                switch result {
                case .success(let value):self?.finishSmoke(value as? [String:Any] ?? ["passed":false,"error":"Invalid smoke response"])
                case .failure(let error):self?.finishSmoke(["passed":false,"error":error.localizedDescription])
                }
            }
        }
    }
    func finishSmoke(_ result:[String:Any]) {
        guard !finishedSmoke else { return }; finishedSmoke = true
        smokeTimer?.invalidate()
        let now = ProcessInfo.processInfo.systemUptime
        var final = result
        if result["passed"] as? Bool == true {
            if let reason = smokeLifecycle.timeout(now) { final["passed"] = false; final["error"] = reason }
            if smokeLifecycle.checksStartedAt == nil { final["passed"] = false; final["error"] = "Checks started without readiness" }
        }
        final["readiness"] = smokeLifecycle.diagnostics(now)
        final["readySignal"] = readySignal
        final["lastProgress"] = smokeProgress
        final["diagnosticReadyDelayMs"] = readyDelayMs
        final["nativeWindowVisible"] = window.isVisible
        final["bundlePath"] = Bundle.main.bundlePath
        final["pythonStderr"] = errorText
        // Commit the result before attempting screenshots: even a hung WebView
        // cannot erase the phase-specific failure JSON.
        if let url = smokeOutput, let data = try? JSONSerialization.data(withJSONObject:final,options:[.prettyPrinted,.sortedKeys]) { try? data.write(to:url,options:.atomic) }
        DispatchQueue.main.asyncAfter(deadline:.now()+10) { NSApp.terminate(nil) }
        captureSmokeFrame("after") { [weak self] in
            self?.captureSmokeFrame("final") { NSApp.terminate(nil) }
        }
    }
    func fail(_ message:String) {
        if smokeOutput != nil { finishSmoke(["passed":false,"error":message]); return }
        let alert = NSAlert(); alert.messageText = "RisingLands-ptbr"; alert.informativeText = message; alert.alertStyle = .critical; alert.runModal()
    }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender:NSApplication)->Bool { true }
    func applicationWillTerminate(_ notification:Notification) {
        nativeLive?.flushSaves()
        nativeLive?.closeCDMusic()
        outPipe?.fileHandleForReading.readabilityHandler = nil
        errPipe?.fileHandleForReading.readabilityHandler = nil
        try? input?.close()
        if engine?.isRunning == true { engine?.terminate() }
    }
}
/// `--ux-menu-trace <out.json>`: presentation measurement of the original pause
/// menu (MONDE01): 10 s idle, 10 s of synthetic pointer moves at 120 Hz, then
/// "Salvar" -> slot 1 and the time until play is presented again. Diagnostic
/// saves only (temporary directory). Run only on the Mac mini with its display.
extension AppDelegate {
    func startUXMenuTrace(_ output:URL){
        guard let live=nativeLive else{return}
        var result:[String:Any]=["commit":Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? ""]
        func write(_ error:String?=nil){
            if let error{result["error"]=error}
            do{try JSONSerialization.data(withJSONObject:result,options:[.prettyPrinted,.sortedKeys]).write(to:output,options:.atomic)}catch{fputs("UX trace: \(error)\n",stderr)}
            NSApp.terminate(nil)
        }
        func key(_ code:UInt16,_ chars:String,_ down:Bool){
            if let event=NSEvent.keyEvent(with:down ? .keyDown:.keyUp,location:.zero,modifierFlags:[],timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:live.window.windowNumber,context:nil,characters:chars,charactersIgnoringModifiers:chars,isARepeat:false,keyCode:code){live.window.sendEvent(event)}
        }
        func tap(_ code:UInt16,_ chars:String){key(code,chars,true);key(code,chars,false)}
        func poll(_ left:Int,_ test:@escaping([String:Any])->Bool,_ next:@escaping([String:Any])->Void){
            live.perform("live-status"){value in
                if test(value){next(value);return}
                if value["live_stop"] is [String:Any]{write("Native stop");return}
                guard left>0 else{write("Trace condition timed out");return}
                DispatchQueue.main.asyncAfter(deadline:.now()+0.05){poll(left-1,test,next)}
            }
        }
        func mode(_ v:[String:Any])->UInt32{v["original_shell_mode"] as? UInt32 ?? 0}
        func frames(_ v:[String:Any])->UInt64{(v["native_metrics"] as? [String:Any])?["frames"] as? UInt64 ?? 0}
        func stats(_ times:[Double])->[String:Any]{
            let gaps=zip(times.dropFirst(),times).map{($0-$1)*1000}.sorted()
            func q(_ p:Double)->Double{gaps.isEmpty ? 0:gaps[min(gaps.count-1,Int(Double(gaps.count-1)*p))]}
            return ["presented":times.count,"gapP50ms":q(0.5),"gapP99ms":q(0.99),"gapMaxms":gaps.last ?? 0]
        }
        live.window.makeKeyAndOrderFront(nil);live.window.makeFirstResponder(live.metal)
        poll(400,{frames($0)>=10 && $0["native_running"] as? Bool==true}){_ in
            key(53,"\u{1b}",true)
            poll(200,{mode($0)==16}){_ in
                key(53,"\u{1b}",false)
                let idle=ProcessInfo.processInfo.systemUptime+0.5
                DispatchQueue.main.asyncAfter(deadline:.now()+10.5){
                    let idleEnd=ProcessInfo.processInfo.systemUptime
                    result["pauseIdle"]=stats(live.uxPresentedTimes(since:idle).filter{$0<=idleEnd})
                    live.uxCounters{before in
                        let start=ProcessInfo.processInfo.systemUptime;var sent=0
                        let timer=Timer(timeInterval:1.0/120,repeats:true){t in
                            let phase=ProcessInfo.processInfo.systemUptime-start
                            if phase>10{t.invalidate();return}
                            live.benchmarkPointer(400+250*cos(phase*2),300+200*sin(phase*3));sent+=1
                        }
                        RunLoop.main.add(timer,forMode:.common)
                        DispatchQueue.main.asyncAfter(deadline:.now()+10.3){
                            let end=ProcessInfo.processInfo.systemUptime
                            live.uxCounters{after in
                                var moving=stats(live.uxPresentedTimes(since:start+0.2).filter{$0<=end})
                                let received=(after["movesReceived"] as? Int ?? 0)-(before["movesReceived"] as? Int ?? 0)
                                let applied=(after["movesApplied"] as? Int ?? 0)-(before["movesApplied"] as? Int ?? 0)
                                moving["movesSent"]=sent;moving["movesReceived"]=received;moving["movesApplied"]=applied
                                moving["movesAppliedPerRefresh"]=Double(applied)/max(1,(end-start)*60)
                                result["pauseMoving"]=moving
                                // Salvar (second item) -> list 14 -> Enter on slot 1 (SAVE_SLOT).
                                tap(125,"\u{f701}");tap(36,"\r")
                                poll(200,{mode($0)==14}){_ in
                                    let clicked=ProcessInfo.processInfo.systemUptime;tap(36,"\r")
                                    poll(400,{mode($0)==0 && $0["native_running"] as? Bool==true}){_ in
                                        let resumed=ProcessInfo.processInfo.systemUptime
                                        DispatchQueue.main.asyncAfter(deadline:.now()+1.5){
                                            let first=live.uxPresentedTimes(since:clicked).first ?? 0
                                            live.uxCounters{final in
                                                result["save"]=["clickToResumedMs":(resumed-clicked)*1000,"clickToFirstPresentedMs":first>0 ? (first-clicked)*1000:-1,
                                                                "saveTrace":final["saveTrace"] ?? [],"saveNotice":final["saveNotice"] ?? "","clickedAt":clicked]
                                                write()
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
// Player build verification: import without any window or activation.
if risingPlayerEdition && CommandLine.arguments.contains("--import-only") { exit(NativePlayerData.headless(CommandLine.arguments)) }
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
