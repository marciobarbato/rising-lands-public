import Foundation

/// "Atalhos modernos" (AoE2-style shortcuts). Host layer only: every command
/// becomes ordinary key/mouse input that the original already accepts, delivered
/// through the same recorded path as the player's own input (macos_live_event).
/// Nothing here writes game state; the game is read through a read-only memory
/// snapshot (macos_live_peek), between frames, on the engine's work queue.
///
/// Original facts used (engine/crates/rl-core/src):
/// - player_input.rs `group`: Ctrl+N saves group N (order 11); N selects it
///   (order 12); Shift+N selects and centres (EnginGetCentreGroupe). Only 1..9.
/// - player_input.rs `input_context`: Enter or X centre on the last attack.
/// - player_input.rs `zone`/`map`: zone 0 (no MouseZone rectangle) is the map.
///   A left press+release on your own unit/building selects it (box_selection).
///   A right press outside zone 11 deselects everything and issues no order.
/// - player_input.rs `minimap`: with nothing selected a left press on zone 10
///   recentres the view (`minimap_center`); with a selection it orders a move.
enum NativeModernHotkeys {
    static let preferenceKey = "RisingLandsModernHotkeys"
    /// On by default (coordinator decision 2026-11-11): every shortcut is additive.
    /// Engine rule set that follows this preference (user decision 2026-11-12):
    /// the Shift+click order queue, `RL_REGRA_FILA_ORDENS` (bit 1 of regras).
    /// It applies to worlds created afterwards; saves keep the rules they recorded.
    static let orderQueueRule: UInt32 = 0x2
    static func engineRules(_ defaults: UserDefaults = .standard) -> UInt32 {
        isEnabled(defaults) ? orderQueueRule : 0
    }
    static func isEnabled(_ defaults: UserDefaults = .standard) -> Bool {
        defaults.object(forKey: preferenceKey) == nil ? true : defaults.bool(forKey: preferenceKey)
    }
    static func store(_ enabled: Bool, _ defaults: UserDefaults = .standard) {
        defaults.set(enabled, forKey: preferenceKey)
    }

    /// The live driver's exact channels (macos_live_driver.c, macos_live_event):
    /// kind 2 accepts these 23 virtual keys, kind 1 the mouse messages 0x200...0x205.
    static let acceptedVirtualKeys: Set<Int> = [13, 16, 17, 18, 27, 32, 37, 38, 39, 40, 46, 88,
                                                48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 9]
    static let acceptedMouseMessages = 0x200...0x205

    /// Second tap of digit N: the original's own Shift+N. Shift stays held until a
    /// game frame has consumed the digit message (`group` polls Shift in that frame).
    static func centerGroup(_ digitKey: Int) -> [NativeModernStep] {
        precondition((49...57).contains(digitKey), "Original groups are digits 1...9")
        return [.key(vkey: 16, down: true), .key(vkey: digitKey, down: true), .key(vkey: digitKey, down: false),
                .waitConsumed, .key(vkey: 16, down: false)]
    }

    /// The original's X: centre on the last attack (Enter is the other original key).
    static let lastAlert: [NativeModernStep] = [.key(vkey: 88, down: true), .waitConsumed, .key(vkey: 88, down: false)]
}

/// One unit of host input, in the exact payload shape the game view produces.
enum NativeModernStep: Equatable, Sendable {
    case key(vkey: Int, down: Bool)
    case mouse(msg: Int, x: Int, y: Int)
    /// Deliver nothing more until every queued input was consumed by a game frame.
    case waitConsumed

    var payload: [String: Any]? {
        switch self {
        case .key(let vkey, let down): return ["kind": "key", "vkey": vkey, "down": down]
        case .mouse(let msg, let x, let y): return ["kind": "mouse", "msg": msg, "x": x, "y": y, "wParam": 0]
        case .waitConsumed: return nil
        }
    }

    /// True when the live driver accepts this input (or it is a barrier).
    var isOriginalInput: Bool {
        switch self {
        case .key(let vkey, _): return NativeModernHotkeys.acceptedVirtualKeys.contains(vkey)
        case .mouse(let msg, let x, let y):
            return NativeModernHotkeys.acceptedMouseMessages.contains(msg) && (0..<800).contains(x) && (0..<600).contains(y)
        case .waitConsumed: return true
        }
    }
}

/// Read-only view of the original memory the planner needs. Addresses are the
/// original's (see engine/crates/rl-core/src and a play bot in the private research repository).
struct NativeModernSnapshot: Equatable {
    struct Unit: Equatable { var index: Int, owner: Int, kind: Int, state: Int, cell: Int, onMap: Bool }
    struct Structure: Equatable { var index: Int, owner: Int, type: Int, state: Int, cells: [Int] }
    struct Zone: Equatable {
        var id: Int, x0: Int, y0: Int, x1: Int, y1: Int
        func contains(_ x: Int, _ y: Int) -> Bool { x0 <= x && y0 <= y && x <= x1 && y <= y1 }
    }

    var inMission = true
    var localPlayer = 0
    var viewX = 0, viewY = 0, viewW = 25, viewH = 17
    var mapW = 128, mapH = 128
    var margin = 0
    var minimapVisible = true
    var minimapX = 0, minimapY = 0, minimapW = 128, minimapH = 128
    /// MouseZone rectangles in test order (first match wins; none = map, zone 0).
    var zones: [Zone] = []
    var selectedUnits = 0
    var selectedStructure = 0xffff
    var units: [Unit] = []
    var structures: [Structure] = []

    var hasSelection: Bool { selectedUnits != 0 || selectedStructure != 0xffff }

    func zone(atX x: Int, y: Int) -> Int { zones.first { $0.contains(x, y) }?.id ?? 0 }

    /// Pixel of a cell's centre in the current view, if a click there reaches the map
    /// handler on that cell (inside the view minus margin, zone 0).
    func clickPixel(cell: Int) -> (x: Int, y: Int)? {
        let (cx, cy) = (cell & 255, cell >> 8)
        let (x, y) = ((cx - viewX) * 32 + 16, (cy - viewY) * 32 + 16)
        let m = margin * 32
        guard x >= m, y >= m, x < viewW * 32 - m, y < viewH * 32 - m, x < 800, y < 600 else { return nil }
        return zone(atX: x, y: y) == 0 ? (x, y) : nil
    }

    /// A map pixel for a right press that only deselects: zone 0, inside the view.
    func deselectPixel() -> (x: Int, y: Int)? {
        let m = margin * 32 + 8
        let width = viewW * 32, height = viewH * 32
        var candidates = [(width / 2, height / 2)]
        for fy in [1, 3] { for fx in [1, 3] { candidates.append((width * fx / 4, height * fy / 4)) } }
        candidates.append(contentsOf: [(m, m), (width - m - 1, m), (m, height - m - 1), (width - m - 1, height - m - 1)])
        return candidates.first { x, y in
            x >= m && y >= m && x < width - m && y < height - m && x < 800 && y < 600 && zone(atX: x, y: y) == 0
        }
    }

    /// Exact replica of the original `minimap_center` (player_input.rs) for a press at (dx, dy).
    func minimapOrigin(dx: Int, dy: Int) -> (x: Int, y: Int) {
        var bx = (dx - (viewW * minimapW / mapW) / 2) * mapW / minimapW
        var by = (dy - (viewH * minimapH / mapH) / 2) * mapH / minimapH
        if Int16(truncatingIfNeeded: bx) < 0 { bx = 0 }
        if Int16(truncatingIfNeeded: by) < 0 { by = 0 }
        if Int(Int16(truncatingIfNeeded: bx)) + viewW > mapW { bx = mapW - viewW }
        if UInt32(truncatingIfNeeded: viewH + Int(Int16(truncatingIfNeeded: by))) > UInt32(mapH) { by = mapH - viewH }
        return (bx, by)
    }

    /// Minimap press pixel whose recentring brings `cell` closest to the view centre,
    /// or nil when the minimap is hidden or not pressable.
    func minimapPress(centering cell: Int) -> (x: Int, y: Int)? {
        guard minimapVisible, mapW > 0, mapH > 0, minimapW > 0, minimapH > 0 else { return nil }
        let wantX = min(max((cell & 255) - viewW / 2, 0), max(mapW - viewW, 0))
        let wantY = min(max((cell >> 8) - viewH / 2, 0), max(mapH - viewH, 0))
        func best(_ count: Int, _ origin: (Int) -> Int, _ want: Int) -> [Int] {
            (0..<count).sorted { abs(origin($0) - want) < abs(origin($1) - want) }
        }
        let xs = best(minimapW, { minimapOrigin(dx: $0, dy: 0).x }, wantX)
        let ys = best(minimapH, { minimapOrigin(dx: 0, dy: $0).y }, wantY)
        for dy in ys.prefix(8) {
            for dx in xs.prefix(8) {
                let (x, y) = (minimapX + dx, minimapY + dy)
                if (0..<800).contains(x), (0..<600).contains(y), zone(atX: x, y: y) == 10 { return (x, y) }
            }
        }
        return nil
    }

    /// Reads the snapshot through `peek`, which returns the bytes of each requested
    /// (address, size) from ONE read-only projection, or nil on any failure.
    static func read(_ peek: ([(UInt32, Int)]) -> [[UInt8]]?) -> NativeModernSnapshot? {
        func u8(_ b: [UInt8], _ o: Int = 0) -> Int { Int(b[o]) }
        func u16(_ b: [UInt8], _ o: Int = 0) -> Int { Int(b[o]) | Int(b[o + 1]) << 8 }
        func u32(_ b: [UInt8], _ o: Int = 0) -> UInt32 { UInt32(b[o]) | UInt32(b[o + 1]) << 8 | UInt32(b[o + 2]) << 16 | UInt32(b[o + 3]) << 24 }
        func i32(_ b: [UInt8], _ o: Int) -> Int { Int(Int32(bitPattern: u32(b, o))) }
        let first: [(UInt32, Int)] = [(0x4e2f78, 1), (0x508041, 1), (0x508720, 8), (0x497168, 4), (0x508f90, 1),
                                      (0x497285, 1), (0x507d68, 4), (0x496fe0, 4), (0x50a9b4, 4),
                                      (0x4e19a0, 4), (0x4e0f38, 4)]
        guard let a = peek(first), a.count == first.count else { return nil }
        var s = NativeModernSnapshot()
        s.inMission = u8(a[0]) == 0x12
        s.localPlayer = u8(a[1])
        (s.viewX, s.viewY, s.viewW, s.viewH) = (u16(a[2]), u16(a[2], 2), u16(a[2], 4), u16(a[2], 6))
        (s.mapW, s.mapH) = (u16(a[3]), u16(a[3], 2))
        s.margin = Int(Int8(bitPattern: a[4][0]))
        s.minimapVisible = u8(a[5]) != 0
        (s.minimapX, s.minimapY, s.minimapW, s.minimapH) = (u16(a[6]), u16(a[6], 2), u16(a[7]), u16(a[7], 2))
        let (zoneFirst, zoneLast) = (u16(a[8]), u16(a[8], 2))
        let (unitBase, gridBase) = (u32(a[9]), u32(a[10]))
        guard s.inMission, s.localPlayer < 4, zoneFirst <= zoneLast, zoneLast < 64, s.mapH <= 256 else {
            return s.inMission ? nil : s
        }
        let p = UInt32(s.localPlayer)
        let second: [(UInt32, Int)] = [(0x5099b4 + 16 * UInt32(zoneFirst), 16 * (zoneLast - zoneFirst + 1)),
                                       (0x508728 + 512 * p, 2), (0x495be0 + 2 * p, 2),
                                       (unitBase, 800 * 276), (0x49a0c8, 1000 * 276)]
        guard let b = peek(second), b.count == second.count else { return nil }
        s.zones = (0...(zoneLast - zoneFirst)).map { i in
            Zone(id: zoneFirst + i, x0: i32(b[0], 16 * i), y0: i32(b[0], 16 * i + 4), x1: i32(b[0], 16 * i + 8), y1: i32(b[0], 16 * i + 12))
        }
        s.selectedUnits = u16(b[1])
        s.selectedStructure = u16(b[2])
        s.units = (0..<800).compactMap { i in
            let o = 276 * i, state = u8(b[3], o + 2)
            guard state != 0, state != 4 else { return nil }
            return Unit(index: i, owner: u8(b[3], o), kind: u8(b[3], o + 1), state: state, cell: u16(b[3], o + 8),
                        onMap: u8(b[3], o + 0x72) != 0)
        }
        // Own sanctuaries only: their solid cells come from the hover grid word
        // (bit 15 set, bits 5..14 = structure index), as bin/victory_bot/view.rs does.
        var own: [(Int, Int, Int, Int)] = []
        for i in 0..<1000 {
            let o = 276 * i
            let (state, owner, type) = (u8(b[4], o + 5), u8(b[4], o + 4), u8(b[4], o + 8))
            if state != 0, owner == s.localPlayer, type == 2 { own.append((i, u16(b[4], o + 2), state, type)) }
        }
        guard !own.isEmpty else { return s }
        let third: [(UInt32, Int)] = [(0x495d8c + 52 * 2, 4), (gridBase, 2 * 256 * s.mapH)]
        guard let c = peek(third), c.count == third.count else { return nil }
        let (fw, fh) = (u16(c[0]), u16(c[0], 2))
        s.structures = own.map { index, origin, state, type in
            var cells: [Int] = []
            for row in 0..<fh { for col in 0..<fw {
                let cell = (origin + row * 256 + col) & 0xffff
                guard 2 * cell + 1 < c[1].count else { continue }
                let word = u16(c[1], 2 * cell)
                if word & 0x8000 != 0, (word & 0x7fe0) >> 5 == index { cells.append(cell) }
            } }
            return Structure(index: index, owner: s.localPlayer, type: type, state: state, cells: cells)
        }
        return s
    }
}

/// Chooses targets and the next stage of input. Each stage is verified against a
/// fresh snapshot before the next one is planned.
struct NativeModernPlanner {
    enum Target: Equatable { case unit(Int), structure(Int) }
    enum Stage: Equatable {
        /// Deliver these steps, wait until consumed, take a new snapshot, plan again.
        case deliver([NativeModernStep])
        /// Deliver these steps; the command is complete.
        case finish([NativeModernStep])
        /// Nothing (more) can be done with original input; deliver nothing.
        case abort
    }
    struct Progress: Equatable { var deselected = false, recentered = false }

    /// Workers/carriers and army kinds (player_pages.rs 148-157, player_input.rs 85-87).
    static let workerKinds: Set<Int> = [5, 6, 7, 13]
    static let armyKinds: Set<Int> = [0, 1, 2, 3, 8, 11, 12, 15, 16, 18, 19, 20, 21, 22, 23, 24, 25]
    /// Unit state 7 is idle (frame_units.rs arrival, player_orders.rs set_idle).
    static let idleState = 7

    private var last: [String: Int] = [:]

    /// Next idle worker/army unit or own sanctuary after the previous choice, cycling.
    mutating func choose(_ command: NativeHotkeyInputRouter.ModernCommand, in s: NativeModernSnapshot) -> Target? {
        let key: String, candidates: [Int]
        switch command {
        case .nextIdleWorker, .nextIdleMilitary:
            let kinds = command == .nextIdleWorker ? Self.workerKinds : Self.armyKinds
            key = command == .nextIdleWorker ? "worker" : "army"
            candidates = s.units.filter {
                $0.owner == s.localPlayer && $0.state == Self.idleState && $0.onMap && kinds.contains($0.kind)
            }.map(\.index)
        case .mainStructure:
            key = "structure"
            candidates = s.structures.filter { $0.owner == s.localPlayer && $0.type == 2 && !$0.cells.isEmpty }.map(\.index)
        default:
            return nil
        }
        guard !candidates.isEmpty else { return nil }
        let previous = last[key] ?? -1
        let chosen = candidates.first { $0 > previous } ?? candidates[0]
        last[key] = chosen
        return command == .mainStructure ? .structure(chosen) : .unit(chosen)
    }

    /// Cells a click may use for the target in this snapshot (empty if it is gone).
    static func cells(_ target: Target, in s: NativeModernSnapshot) -> [Int] {
        switch target {
        case .unit(let index):
            guard let unit = s.units.first(where: { $0.index == index }), unit.owner == s.localPlayer, unit.onMap else { return [] }
            return [unit.cell]
        case .structure(let index):
            return s.structures.first { $0.index == index && $0.owner == s.localPlayer }?.cells ?? []
        }
    }

    /// One stage: deselect (verified), recentre with the minimap (verified), then click.
    static func next(_ target: Target, in s: NativeModernSnapshot, progress: inout Progress) -> Stage {
        guard s.inMission else { return .abort }
        let cells = cells(target, in: s)
        guard !cells.isEmpty else { return .abort }
        if s.hasSelection {
            // A selection could turn the click (or a minimap press) into an order.
            guard !progress.deselected, let pixel = s.deselectPixel() else { return .abort }
            progress.deselected = true
            return .deliver([.mouse(msg: 0x204, x: pixel.x, y: pixel.y)])
        }
        if let pixel = cells.lazy.compactMap({ s.clickPixel(cell: $0) }).first {
            return .finish([.mouse(msg: 0x201, x: pixel.x, y: pixel.y), .mouse(msg: 0x202, x: pixel.x, y: pixel.y)])
        }
        guard !progress.recentered, let pixel = s.minimapPress(centering: cells[cells.count / 2]) else { return .abort }
        progress.recentered = true
        return .deliver([.mouse(msg: 0x201, x: pixel.x, y: pixel.y), .mouse(msg: 0x202, x: pixel.x, y: pixel.y)])
    }
}

extension NativeModernSnapshot {
    /// Same as `read`, over a peek that fills ONE buffer with the ranges concatenated
    /// in request order (macos_live_peek's contract).
    static func read(concatenated peek: (_ addresses: [UInt32], _ sizes: [UInt32]) -> [UInt8]?) -> NativeModernSnapshot? {
        read { ranges in
            guard let bytes = peek(ranges.map(\.0), ranges.map { UInt32($0.1) }),
                  bytes.count == ranges.reduce(0, { $0 + $1.1 }) else { return nil }
            var offset = 0
            return ranges.map { range in
                defer { offset += range.1 }
                return Array(bytes[offset..<offset + range.1])
            }
        }
    }
}

/// Player pointer, written by the game view on the main thread and read by the
/// engine work queue when a command returns the cursor to the player.
final class NativeModernPointer: @unchecked Sendable {
    private let lock = NSLock()
    private var value = (x: 400, y: 300)
    func set(_ x: Int, _ y: Int) { lock.lock(); value = (x, y); lock.unlock() }
    func get() -> (x: Int, y: Int) { lock.lock(); defer { lock.unlock() }; return value }
}

/// Carries one command across game frames. `pump` runs between frames on the
/// engine work queue, after each advance; every host closure runs there too.
final class NativeModernJob {
    struct Host {
        /// Same path as the player's input (macos_live_event + recording). false = refused.
        var deliver: ([String: Any]) -> Bool
        /// macos_live_pending_input.
        var pendingInput: () -> UInt32
        /// Completed game frames.
        var frames: () -> UInt64
        /// Fresh read-only snapshot (macos_live_peek).
        var snapshot: () -> NativeModernSnapshot?
        /// Player pointer to return the game cursor to at the end.
        var pointer: () -> (x: Int, y: Int)
    }
    enum State: Equatable { case running, finished, aborted }

    /// A command never takes longer than this many frames (about 10 s at 1x).
    static let frameBudget: UInt64 = 300
    private(set) var state = State.running
    /// Every input this job delivered, in order (tests replay it by hand).
    private(set) var delivered: [(frame: UInt64, step: NativeModernStep)] = []
    let target: NativeModernPlanner.Target?
    private var replan: Bool
    private var queue: [NativeModernStep]
    private var progress = NativeModernPlanner.Progress()
    private var waitFrom: UInt64?
    private var started: UInt64?
    private var held = Set<Int>()
    private var movedMouse = false

    var usesMouse: Bool { target != nil }

    /// Key-only commands (group centring, last alert): fixed steps.
    init(steps: [NativeModernStep]) { target = nil; replan = false; queue = steps }
    /// Mouse commands: planned stage by stage from fresh snapshots.
    init(target: NativeModernPlanner.Target) { self.target = target; replan = true; queue = [] }

    func pump(_ host: Host) {
        guard state == .running else { return }
        let now = host.frames()
        if started == nil { started = now }
        if now - started! > Self.frameBudget { return abort(host) }
        while state == .running {
            guard let step = queue.first else {
                guard replan, let target else { state = .finished; return }
                guard let snapshot = host.snapshot() else { return abort(host) }
                switch NativeModernPlanner.next(target, in: snapshot, progress: &progress) {
                case .deliver(let steps): queue = steps + [.waitConsumed]
                case .finish(let steps):
                    let pointer = host.pointer()
                    queue = steps + [.mouse(msg: 0x200, x: pointer.x, y: pointer.y), .waitConsumed]
                    replan = false
                case .abort: return abort(host)
                }
                continue
            }
            if step == .waitConsumed {
                let frame = host.frames()
                if waitFrom == nil { waitFrom = frame }
                guard host.pendingInput() == 0, frame > waitFrom! else { return }
                waitFrom = nil
                queue.removeFirst()
                continue
            }
            guard send(step, host) else { return abort(host) }
            queue.removeFirst()
        }
    }

    private func send(_ step: NativeModernStep, _ host: Host) -> Bool {
        guard step.isOriginalInput, let payload = step.payload, host.deliver(payload) else { return false }
        delivered.append((host.frames(), step))
        switch step {
        case .key(let vkey, let down): if down { held.insert(vkey) } else { held.remove(vkey) }
        case .mouse: movedMouse = true
        case .waitConsumed: break
        }
        return true
    }

    /// Releases synthetic keys and returns the cursor; delivers nothing else.
    private func abort(_ host: Host) {
        state = .aborted
        queue.removeAll()
        for vkey in held.sorted() { _ = send(.key(vkey: vkey, down: false), host) }
        if movedMouse { let p = host.pointer(); _ = send(.mouse(msg: 0x200, x: p.x, y: p.y), host) }
        state = .aborted
    }
}

/// Shift+click order queue markers (engine rule `RL_REGRA_FILA_ORDENS`): the
/// pending waypoints of the local player's selected units, in original 800x600
/// pixels, one path per unit. Read-only (one snapshot plus `rl_fila_ordens`);
/// drawn by the host over the picture, never part of the game or a recording.
struct NativeOrderQueueMarkers: Equatable {
    struct Point: Equatable { var x: Int, y: Int }
    /// Waypoint centres per selected unit with waypoints, oldest first.
    var paths: [[Point]] = []
    /// The map part of the picture (the view's cells), where markers may show.
    var mapWidth = 0, mapHeight = 0
    static let empty = NativeOrderQueueMarkers()
    /// Mission state, local player, view (x, y, w, h), the four selection lists.
    static let addresses: [UInt32] = [0x4e2f78, 0x508041, 0x508720, 0x508728]
    static let sizes: [UInt32] = [1, 1, 8, 4 * 512]
    /// At most this many selected units are drawn.
    static let maxUnits = 64

    static func read(peek: ([UInt32], [UInt32]) -> [UInt8]?, queue: (Int) -> [UInt16]) -> NativeOrderQueueMarkers? {
        guard let b = peek(addresses, sizes), b.count == Int(sizes.reduce(0, +)) else { return nil }
        func u16(_ o: Int) -> Int { Int(b[o]) | Int(b[o + 1]) << 8 }
        guard b[0] == 0x12 else { return .empty }
        let local = Int(b[1]) & 3
        let (vx, vy, vw, vh) = (u16(2), u16(4), u16(6), u16(8))
        let list = 10 + 512 * local
        let count = min(u16(list), maxUnits, 255)
        var markers = NativeOrderQueueMarkers(paths: [], mapWidth: vw * 32, mapHeight: vh * 32)
        for k in 0..<count {
            let cells = queue(u16(list + 2 + 2 * k))
            if cells.isEmpty { continue }
            markers.paths.append(cells.map {
                Point(x: (Int($0 & 255) - vx) * 32 + 16, y: (Int($0 >> 8) - vy) * 32 + 16)
            })
        }
        return markers
    }
}
