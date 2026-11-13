import AppKit

/// Where the player edition keeps the data imported from the player's CD.
enum NativePlayerData {
    /// Folder holding `Dados/`; `--data-dir` overrides it (smokes and tests).
    static var folder: URL {
        let args = CommandLine.arguments
        if let i = args.firstIndex(of: "--data-dir"), args.count > i + 1 { return URL(fileURLWithPath: args[i + 1], isDirectory: true) }
        return FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("RisingLands-ptbr", isDirectory: true)
    }
    static var data: URL { folder.appendingPathComponent("Dados", isDirectory: true) }
    static var resources: URL { Bundle.main.resourceURL!.appendingPathComponent("player", isDirectory: true) }
    static var manifest: URL { resources.appendingPathComponent("import-manifest.json") }
    static var ready: Bool { NativeCDImport.installed(manifestURL: manifest, folder: folder) }

    /// An import interrupted by quitting (Sair, or the app ending mid-copy) leaves its
    /// `Dados.importando-*` staging folder behind; `install` only removes it on errors.
    /// Called at launch, before any import of this process can have started.
    @discardableResult
    static func removeInterruptedImports() -> Int {
        let names = (try? FileManager.default.contentsOfDirectory(atPath: folder.path)) ?? []
        var removed = 0
        for name in names where name.hasPrefix("Dados.importando-") {
            if (try? FileManager.default.removeItem(at: folder.appendingPathComponent(name))) != nil { removed += 1 }
        }
        return removed
    }

    /// `RisingLands --import-only SOURCE [--data-dir FOLDER]`: imports without any
    /// window or activation (build verification) and prints a JSON receipt.
    static func headless(_ args: [String]) -> Int32 {
        guard let i = args.firstIndex(of: "--import-only"), args.count > i + 1 else { return 2 }
        do {
            let source = try NativeCDImport.source(URL(fileURLWithPath: args[i + 1]))
            let report = try NativeCDImport.install(manifestURL: manifest, resources: resources, source: source, folder: folder)
            print("{\"imported\":\(report.written),\"music\":\(report.music),\"manifestId\":\"\(report.manifestID)\",\"ready\":\(ready)}")
            return ready ? 0 : 1
        } catch {
            FileHandle.standardError.write(Data((error.localizedDescription + "\n").utf8))
            return 1
        }
    }
}

/// First launch: explains where the CD comes from, lets the player choose the
/// image or folder, verifies and copies it, then hands over to the game.
@MainActor
final class NativePlayerImportWindow: NSObject, NSWindowDelegate {
    let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 330), styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
    let status = NSTextField(labelWithString: "")
    let progress = NSProgressIndicator()
    let choose = NSButton(title: "Escolher o CD…", target: nil, action: nil)
    let quit = NSButton(title: "Sair", target: nil, action: nil)
    var finished: ((NativeCDImport.Report) -> Void)?
    private(set) var busy = false
    /// The last refusal shown on this window (read by the first-use smoke).
    private(set) var alert: NSAlert?

    override init() {
        super.init()
        window.title = "Rising Lands — importar o CD original"
        window.delegate = self
        let content = NSView(frame: window.contentRect(forFrameRect: window.frame))
        let title = NSTextField(labelWithString: "Bem-vindo ao Rising Lands")
        title.font = .boldSystemFont(ofSize: 17); title.frame = NSRect(x: 24, y: 280, width: 552, height: 26)
        let text = NSTextField(wrappingLabelWithString: """
        Para jogar, o aplicativo precisa dos arquivos do CD original brasileiro do Rising Lands, que não acompanham este aplicativo.

        1. Baixe a imagem do CD (os arquivos RISING_LANDS_BR_.BIN e RISING_LANDS_BR_.CUE) no Internet Archive (archive.org).
        2. Clique em “Escolher o CD…” e selecione o arquivo .BIN, .CUE ou .ISO, ou a pasta com os arquivos do CD.

        Os arquivos são conferidos e copiados para a pasta de Suporte de Aplicativos uma única vez. A música só vem da imagem .BIN.
        """)
        text.frame = NSRect(x: 24, y: 96, width: 552, height: 176)
        progress.frame = NSRect(x: 24, y: 70, width: 552, height: 16); progress.isIndeterminate = false
        progress.minValue = 0; progress.maxValue = 1; progress.isHidden = true
        status.frame = NSRect(x: 24, y: 46, width: 552, height: 20); status.lineBreakMode = .byTruncatingMiddle
        status.textColor = .secondaryLabelColor
        choose.frame = NSRect(x: 416, y: 12, width: 164, height: 30); choose.bezelStyle = .rounded; choose.keyEquivalent = "\r"
        choose.target = self; choose.action = #selector(chooseSource)
        quit.frame = NSRect(x: 320, y: 12, width: 90, height: 30); quit.bezelStyle = .rounded
        quit.target = NSApp; quit.action = #selector(NSApplication.terminate(_:))
        for view in [title, text, progress, status, choose, quit] { content.addSubview(view) }
        window.contentView = content
        window.center()
    }

    func show() { window.makeKeyAndOrderFront(nil) }

    @objc func chooseSource() {
        let panel = NSOpenPanel()
        panel.title = "Escolha a imagem ou a pasta do CD do Rising Lands"
        panel.prompt = "Importar"
        panel.message = "Selecione RISING_LANDS_BR_.BIN, o arquivo .CUE, uma imagem .ISO ou a pasta do CD."
        panel.canChooseFiles = true; panel.canChooseDirectories = true; panel.allowsMultipleSelection = false
        panel.beginSheetModal(for: window) { [weak self] answer in
            guard answer == .OK, let url = panel.url else { return }
            self?.start(url)
        }
    }

    /// Imports from `url` on a background queue; errors stay on this window.
    func start(_ url: URL) {
        guard !busy else { return }
        busy = true; choose.isEnabled = false; progress.isHidden = false; progress.doubleValue = 0
        status.stringValue = "Abrindo \(url.lastPathComponent)…"
        let manifest = NativePlayerData.manifest, resources = NativePlayerData.resources, folder = NativePlayerData.folder
        DispatchQueue.global(qos: .userInitiated).async { [weak self] in
            let result = Result { () throws -> NativeCDImport.Report in
                let source = try NativeCDImport.source(url)
                return try NativeCDImport.install(manifestURL: manifest, resources: resources, source: source, folder: folder) { fraction, message in
                    DispatchQueue.main.async { self?.progress.doubleValue = fraction; self?.status.stringValue = message }
                }
            }
            DispatchQueue.main.async { self?.complete(result) }
        }
    }

    private func complete(_ result: Result<NativeCDImport.Report, Error>) {
        busy = false; choose.isEnabled = true
        switch result {
        case .success(let report):
            status.stringValue = report.music ? "CD importado." : "CD importado, sem a música (escolha a imagem .BIN para incluí-la)."
            window.orderOut(nil)
            finished?(report)
        case .failure(let error):
            progress.isHidden = true; status.stringValue = ""
            let alert = NSAlert(); alert.alertStyle = .warning; self.alert = alert
            alert.messageText = "Não foi possível importar o CD"
            alert.informativeText = error.localizedDescription
            alert.addButton(withTitle: "OK")
            alert.beginSheetModal(for: window)
        }
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool { !busy }
}
