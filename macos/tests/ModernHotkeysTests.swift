import Foundation

/// Standalone: compile with NativeHotkeyBindings, NativeHotkeyInputRouter and
/// NativeModernHotkeys. No engine, no window. The world-state equivalence on a
/// real MONDE01 is checked in the private research repository (it needs CD data).
@main
@MainActor
struct ModernHotkeysTests {
    typealias Router = NativeHotkeyInputRouter
    typealias Command = Router.ModernCommand
    typealias Step = NativeModernStep
    typealias Snapshot = NativeModernSnapshot
    typealias Planner = NativeModernPlanner
    struct Failure: Error { let message: String }

    static func check(_ value: Bool, _ message: String, line: UInt = #line) throws {
        if !value { throw Failure(message: "Line \(line): \(message)") }
    }

    final class Clock { var now: TimeInterval = 100 }

    static func router(enabled: Bool) -> (Router, Clock, () -> [String], () -> [Command]) {
        let clock = Clock()
        var events: [String] = [], commands: [Command] = []
        let router = Router(clock: { clock.now }, sink: { events.append("\($0.virtualKey):\($0.down ? "d" : "u")") })
        router.modernEnabled = enabled
        router.modernSink = { commands.append($0) }
        router.setActive(true)
        return (router, clock, { events }, { commands })
    }

    static func routerCommands() throws {
        // Disabled: modern keys stay unhandled and digits are exactly the original channel.
        var (r, clock, events, commands) = router(enabled: false)
        try check(r.keyDown(keyCode: 47, modifiers: []) == .unhandled, "Disabled modern key handled")
        r.keyUp(keyCode: 47, modifiers: [])
        for _ in 0..<2 { r.keyDown(keyCode: 18, modifiers: []); r.keyUp(keyCode: 18, modifiers: []) }
        try check(events() == ["49:d", "49:u", "49:d", "49:u"] && commands().isEmpty, "Disabled double tap changed input")

        (r, clock, events, commands) = router(enabled: true)
        for (code, command): (UInt16, Command) in [(47, .nextIdleWorker), (43, .nextIdleMilitary), (4, .mainStructure), (115, .lastAlert)] {
            try check(r.keyDown(keyCode: code, modifiers: []) == .handled, "Modern key unhandled \(code)")
            try check(r.keyDown(keyCode: code, modifiers: [], isRepeat: true) == .handled, "Modern repeat leaked \(code)")
            try check(r.keyUp(keyCode: code, modifiers: []) == .handled, "Modern release leaked \(code)")
            try check(commands().last == command, "Wrong command for \(code)")
        }
        try check(events().isEmpty && commands().count == 4, "Modern keys emitted original channels")
        try check(r.keyDown(keyCode: 47, modifiers: .shift) == .unhandled, "Modern default is not exact")
        r.keyUp(keyCode: 47, modifiers: [])

        // Double tap: the first tap is the original N; the second, within 0.35 s, is a command.
        (r, clock, events, commands) = router(enabled: true)
        r.keyDown(keyCode: 18, modifiers: []); r.keyUp(keyCode: 18, modifiers: [])
        clock.now += 0.2
        try check(r.keyDown(keyCode: 18, modifiers: []) == .handled, "Second tap unhandled")
        r.keyUp(keyCode: 18, modifiers: [])
        try check(events() == ["49:d", "49:u"] && commands() == [.centerGroup(49)], "Double tap differs")
        clock.now += 0.1
        r.keyDown(keyCode: 18, modifiers: []); r.keyUp(keyCode: 18, modifiers: [])
        try check(events().count == 4 && commands().count == 1, "A third tap must start over")
        clock.now += 0.5
        r.keyDown(keyCode: 18, modifiers: []); r.keyUp(keyCode: 18, modifiers: [])
        try check(events().count == 6 && commands().count == 1, "Slow taps are two original selections")
        // Another digit, Ctrl+N (save) or a different key in between breaks the pair.
        clock.now += 0.1
        r.keyDown(keyCode: 19, modifiers: []); r.keyUp(keyCode: 19, modifiers: [])
        try check(commands().count == 1, "Different digits paired")
        r.keyDown(keyCode: 19, modifiers: .control); r.keyUp(keyCode: 19, modifiers: [])
        r.keyDown(keyCode: 19, modifiers: []); r.keyUp(keyCode: 19, modifiers: [])
        try check(commands().count == 1, "Ctrl+N then N paired")
        r.keyDown(keyCode: 29, modifiers: []); r.keyUp(keyCode: 29, modifiers: [])
        r.keyDown(keyCode: 29, modifiers: []); r.keyUp(keyCode: 29, modifiers: [])
        try check(commands().count == 1, "Digit 0 is not an original group")
        // Shift+N stays the original Shift+N (select and centre).
        r.keyDown(keyCode: 20, modifiers: .shift); r.keyUp(keyCode: 20, modifiers: [])
        try check(events().suffix(4) == ["16:d", "51:d", "16:u", "51:u"], "Shift+N not original")
        // Focus loss clears a pending first tap.
        r.keyDown(keyCode: 21, modifiers: []); r.keyUp(keyCode: 21, modifiers: [])
        r.setActive(false); r.setActive(true)
        r.keyDown(keyCode: 21, modifiers: []); r.keyUp(keyCode: 21, modifiers: [])
        try check(commands().count == 1 && r.heldKeys.isEmpty, "Focus loss kept a pending tap")
    }

    static func fixedSteps() throws {
        for key in 49...57 {
            let steps = NativeModernHotkeys.centerGroup(key)
            try check(steps.allSatisfy(\.isOriginalInput), "Group centring uses a foreign channel")
            try check(steps == [.key(vkey: 16, down: true), .key(vkey: key, down: true), .key(vkey: key, down: false),
                                .waitConsumed, .key(vkey: 16, down: false)], "Shift is not held over the consumed digit")
        }
        try check(NativeModernHotkeys.lastAlert.allSatisfy(\.isOriginalInput)
                  && NativeModernHotkeys.lastAlert.compactMap(\.payload).map { $0["vkey"] as? Int } == [88, 88], "Last alert is not the original X")
        try check(!Step.key(vkey: 0x7a, down: true).isOriginalInput && !Step.key(vkey: 0x45, down: true).isOriginalInput
                  && !Step.mouse(msg: 0x206, x: 1, y: 1).isOriginalInput && !Step.mouse(msg: 0x201, x: 800, y: 1).isOriginalInput,
                  "Foreign input accepted")
        let payload = Step.mouse(msg: 0x201, x: 10, y: 20).payload!
        try check(payload["kind"] as? String == "mouse" && payload["msg"] as? Int == 0x201 && payload["x"] as? Int == 10
                  && payload["y"] as? Int == 20 && payload["wParam"] as? Int == 0 && payload.count == 5, "Payload differs from the view's")
    }

    /// Independent transcription of player_input.rs `minimap_center`.
    static func originalMinimapCenter(_ s: Snapshot, _ x: Int, _ y: Int) -> (Int, Int) {
        let (w, h, mw, mh, vw, vh) = (Int32(s.mapW), Int32(s.mapH), Int32(s.minimapW), Int32(s.minimapH), Int32(s.viewW), Int32(s.viewH))
        var bx = (Int32(x) - (vw * mw / w) / 2) * w / mw
        var dy = (Int32(y) - (vh * mh / h) / 2) * h / mh
        if Int16(truncatingIfNeeded: bx) < 0 { bx = 0 }
        if Int16(truncatingIfNeeded: dy) < 0 { dy = 0 }
        if Int32(Int16(truncatingIfNeeded: bx)) + vw > w { bx = w - vw }
        if UInt32(bitPattern: vh + Int32(Int16(truncatingIfNeeded: dy))) > UInt32(h) { dy = h - vh }
        return (Int(UInt16(truncatingIfNeeded: bx)), Int(UInt16(truncatingIfNeeded: dy)))
    }

    nonisolated static func world() -> Snapshot {
        var s = Snapshot()
        s.localPlayer = 1
        (s.viewX, s.viewY, s.viewW, s.viewH) = (10, 10, 25, 17)
        (s.mapW, s.mapH) = (128, 96)
        (s.minimapX, s.minimapY, s.minimapW, s.minimapH) = (660, 400, 128, 96)
        s.zones = [.init(id: 1, x0: 0, y0: 0, x1: 1, y1: 1), .init(id: 2, x0: 2, y0: 0, x1: 797, y1: 1),
                   .init(id: 4, x0: 798, y0: 2, x1: 799, y1: 597), .init(id: 6, x0: 2, y0: 598, x1: 797, y1: 599),
                   .init(id: 8, x0: 0, y0: 2, x1: 2, y1: 597), .init(id: 10, x0: 660, y0: 400, x1: 788, y1: 496),
                   .init(id: 11, x0: 0, y0: 544, x1: 799, y1: 599)]
        s.units = [
            .init(index: 3, owner: 1, kind: 5, state: 7, cell: 12 << 8 | 12, onMap: true),   // idle worker, on screen
            .init(index: 4, owner: 1, kind: 5, state: 5, cell: 13 << 8 | 13, onMap: true),   // busy worker
            .init(index: 7, owner: 2, kind: 6, state: 7, cell: 14 << 8 | 14, onMap: true),   // enemy idle worker
            .init(index: 9, owner: 1, kind: 7, state: 7, cell: 80 << 8 | 100, onMap: true),  // idle worker, off screen
            .init(index: 11, owner: 1, kind: 13, state: 7, cell: 15 << 8 | 15, onMap: false),// housed
            .init(index: 20, owner: 1, kind: 2, state: 7, cell: 20 << 8 | 20, onMap: true),  // idle army
            .init(index: 21, owner: 1, kind: 9, state: 7, cell: 21 << 8 | 21, onMap: true),  // colonist: neither
        ]
        s.structures = [.init(index: 5, owner: 1, type: 2, state: 5, cells: [30 << 8 | 40, 30 << 8 | 41])]
        return s
    }

    static func snapshotGeometry() throws {
        var s = world()
        for (mw, mh, w, h, vw, vh) in [(128, 96, 128, 96, 25, 17), (128, 128, 64, 200, 25, 13), (100, 77, 255, 256, 20, 13)] {
            (s.minimapW, s.minimapH, s.mapW, s.mapH, s.viewW, s.viewH) = (mw, mh, w, h, vw, vh)
            for x in stride(from: 0, to: mw + 3, by: 1) {
                for y in stride(from: 0, to: mh + 3, by: 7) {
                    let expected = originalMinimapCenter(s, x, y), actual = s.minimapOrigin(dx: x, dy: y)
                    try check(actual.x == expected.0 && actual.y == expected.1, "minimap_center replica differs at \(x),\(y)")
                }
            }
        }
        s = world()
        try check(s.clickPixel(cell: 12 << 8 | 12)! == (80, 80), "Cell centre pixel")
        try check(s.clickPixel(cell: 9 << 8 | 12) == nil && s.clickPixel(cell: 12 << 8 | 9) == nil, "Cell left of the view clicked")
        try check(s.clickPixel(cell: 10 << 8 | 35) == nil && s.clickPixel(cell: 27 << 8 | 12) == nil, "Cell right/below the view clicked")
        try check(s.clickPixel(cell: 23 << 8 | 31) == nil, "Cell under the minimap clicked")
        s.margin = 1
        try check(s.clickPixel(cell: 10 << 8 | 12) == nil && s.clickPixel(cell: 11 << 8 | 12) != nil, "Margin ignored")
        let d = s.deselectPixel()!
        try check(s.zone(atX: d.x, y: d.y) == 0 && d.y < 17 * 32, "Deselect pixel outside the map zone")
        s.minimapVisible = false
        try check(s.minimapPress(centering: 80 << 8 | 100) == nil, "Hidden minimap pressed")
    }

    static func planner() throws {
        let s = world()
        var p = Planner()
        try check(p.choose(.nextIdleWorker, in: s) == .unit(3) && p.choose(.nextIdleWorker, in: s) == .unit(9)
                  && p.choose(.nextIdleWorker, in: s) == .unit(3), "Idle worker cycle")
        try check(p.choose(.nextIdleMilitary, in: s) == .unit(20) && p.choose(.nextIdleMilitary, in: s) == .unit(20), "Idle army cycle")
        try check(p.choose(.mainStructure, in: s) == .structure(5), "Sanctuary")
        try check(p.choose(.lastAlert, in: s) == nil && p.choose(.centerGroup(49), in: s) == nil, "Key commands have no target")
        var empty = s; empty.units = []; empty.structures = []
        try check(p.choose(.nextIdleWorker, in: empty) == nil && p.choose(.mainStructure, in: empty) == nil, "Invented target")

        var progress = Planner.Progress()
        try check(Planner.next(.unit(3), in: s, progress: &progress) == .finish([.mouse(msg: 0x201, x: 80, y: 80), .mouse(msg: 0x202, x: 80, y: 80)]),
                  "Visible target is one click")
        var selected = s; selected.selectedUnits = 2
        progress = .init()
        guard case .deliver(let steps) = Planner.next(.unit(3), in: selected, progress: &progress),
              case .mouse(0x204, let x, let y) = steps.first, steps.count == 1 else { throw Failure(message: "Selection not cleared first") }
        try check(selected.zone(atX: x, y: y) == 0, "Deselect outside zone 0")
        try check(Planner.next(.unit(3), in: selected, progress: &progress) == .abort, "A failed deselect must not be retried blindly")
        progress = .init()
        guard case .deliver(let press) = Planner.next(.unit(9), in: s, progress: &progress),
              case .mouse(0x201, let mx, let my) = press.first else { throw Failure(message: "Off-screen target not recentred") }
        try check(s.zone(atX: mx, y: my) == 10, "Minimap press outside zone 10")
        let origin = s.minimapOrigin(dx: mx - s.minimapX, dy: my - s.minimapY)
        try check(abs(origin.x - (100 - 12)) <= 1 && abs(origin.y - (80 - 8)) <= 1, "Recentre misses the target: \(origin)")
        try check(Planner.next(.unit(9), in: s, progress: &progress) == .abort, "Second recentre attempted")
        var hidden = s; hidden.minimapVisible = false
        progress = .init()
        try check(Planner.next(.unit(9), in: hidden, progress: &progress) == .abort, "Hidden minimap: must do nothing")
        progress = .init()
        try check(Planner.next(.unit(7), in: s, progress: &progress) == .abort, "Enemy unit targeted")
        var outOfMission = s; outOfMission.inMission = false
        try check(Planner.next(.unit(3), in: outOfMission, progress: &progress) == .abort, "Acted outside a mission")
    }

    /// Driver-shaped fake: frame stamping of macos_live_event (keys next frame; mouse
    /// k/k+1(/k+3) gestures in sequence), with the original effects the planner relies on.
    final class FakeGame {
        var snapshot = world()
        var frames: UInt64 = 1000
        var queue: [(frame: UInt64, payload: [String: Any])] = []
        var mouseUntil: UInt64 = 0, leftDown: UInt64 = 0
        var held = Set<Int>()
        var digitsWithShift: [Int: Bool] = [:]
        var log: [(UInt64, [String: Any])] = []
        var cursor = (x: 400, y: 300)
        var press: (x: Int, y: Int)?

        func deliver(_ p: [String: Any]) -> Bool {
            log.append((frames, p))
            var frame = frames + 1
            if p["kind"] as? String == "key" { queue.append((frame, p)); return true }
            let msg = p["msg"] as! Int
            if frame <= mouseUntil { frame = mouseUntil + 1 }
            switch msg {
            case 0x204: // driver: 0x200 at k, 0x204 at k+1, its own 0x205 at k+3
                queue.append((frame + 1, p)); queue.append((frame + 3, ["kind": "mouse", "msg": 0x205, "x": p["x"]!, "y": p["y"]!])); mouseUntil = frame + 3
            case 0x201: queue.append((frame + 1, p)); mouseUntil = frame + 1; leftDown = frame + 1
            case 0x202: frame = max(frame, leftDown + 2); queue.append((frame, p)); mouseUntil = frame
            default: queue.append((frame, p)); mouseUntil = frame
            }
            return true
        }
        func advance() {
            frames += 1
            for item in queue where item.frame <= frames { apply(item.payload) }
            queue.removeAll { $0.frame <= frames }
        }
        func apply(_ p: [String: Any]) {
            if p["kind"] as? String == "key" {
                let vkey = p["vkey"] as! Int, down = p["down"] as! Bool
                if down { held.insert(vkey); if (49...57).contains(vkey) { digitsWithShift[vkey] = held.contains(16) } } else { held.remove(vkey) }
                return
            }
            let (msg, x, y) = (p["msg"] as! Int, p["x"] as! Int, p["y"] as! Int)
            cursor = (x, y)
            let s = snapshot
            switch msg {
            case 0x204 where s.zone(atX: x, y: y) != 11: snapshot.selectedUnits = 0; snapshot.selectedStructure = 0xffff
            case 0x201: press = (x, y)
                if s.zone(atX: x, y: y) == 10, !s.hasSelection {
                    let o = s.minimapOrigin(dx: x - s.minimapX, dy: y - s.minimapY); (snapshot.viewX, snapshot.viewY) = (o.x, o.y)
                }
            case 0x202:
                guard let start = press, s.zone(atX: start.x, y: start.y) == 0 else { break }
                let cell = (y / 32 + s.viewY) << 8 | (x / 32 + s.viewX)
                if let unit = s.units.first(where: { $0.cell == cell && $0.owner == s.localPlayer && $0.onMap }) {
                    snapshot.selectedUnits = 1; snapshot.selectedStructure = 0xffff; selectedUnit = unit.index
                } else if let structure = s.structures.first(where: { $0.cells.contains(cell) }) {
                    snapshot.selectedStructure = structure.index
                }
                press = nil
            default: break
            }
        }
        var selectedUnit = -1
        var host: NativeModernJob.Host {
            .init(deliver: { self.deliver($0) }, pendingInput: { UInt32(self.queue.count) }, frames: { self.frames },
                  snapshot: { self.snapshot }, pointer: { (123, 234) })
        }
    }

    static func run(_ job: NativeModernJob, _ game: FakeGame) {
        job.pump(game.host)
        for _ in 0..<400 where job.state == .running { game.advance(); job.pump(game.host) }
        for _ in 0..<5 { game.advance() }
    }

    static func jobs() throws {
        // Group centring: Shift is held on the frame that consumes the digit, then released.
        var game = FakeGame()
        var job = NativeModernJob(steps: NativeModernHotkeys.centerGroup(52))
        run(job, game)
        try check(job.state == .finished && game.digitsWithShift[52] == true && game.held.isEmpty, "Shift not held over the digit")
        try check(job.delivered.map(\.step).allSatisfy(\.isOriginalInput), "Foreign input delivered")

        // Off-screen idle worker with units selected: deselect, recentre, click, return cursor.
        game = FakeGame(); game.snapshot.selectedUnits = 3
        job = NativeModernJob(target: .unit(9))
        run(job, game)
        try check(job.state == .finished, "Job did not finish: \(job.state)")
        try check(game.selectedUnit == 9 && game.snapshot.selectedUnits == 1, "Target not selected")
        try check(game.cursor == (123, 234), "Cursor not returned to the player")
        let msgs = job.delivered.compactMap { if case .mouse(let m, _, _) = $0.step { return m } else { return nil } }
        try check(msgs == [0x204, 0x201, 0x202, 0x201, 0x202, 0x200], "Unexpected input sequence \(msgs)")
        try check(job.delivered.map(\.step).allSatisfy(\.isOriginalInput), "Foreign input delivered")
        // Each stage starts only after the previous one was consumed.
        let frames = job.delivered.map(\.frame)
        try check(frames[1] > frames[0] + 3 && frames[3] > frames[2], "Stages interleaved")

        // Sanctuary: one click on a solid cell after recentring.
        game = FakeGame()
        job = NativeModernJob(target: .structure(5))
        run(job, game)
        try check(job.state == .finished && game.snapshot.selectedStructure == 5, "Sanctuary not selected")

        // Hidden minimap and an off-screen target: nothing is delivered.
        game = FakeGame(); game.snapshot.minimapVisible = false
        job = NativeModernJob(target: .unit(9))
        run(job, game)
        try check(job.state == .aborted && job.delivered.isEmpty && game.log.isEmpty, "Hidden minimap delivered input")

        // A target that disappears mid-command aborts and still returns the cursor.
        game = FakeGame(); game.snapshot.selectedUnits = 1
        job = NativeModernJob(target: .unit(9))
        job.pump(game.host)
        game.snapshot.units.removeAll { $0.index == 9 }
        run(job, game)
        try check(job.state == .aborted && game.cursor == (123, 234), "Vanished target: no cursor return")

        // Budget: a game that never consumes input cannot keep a job alive.
        game = FakeGame()
        job = NativeModernJob(steps: NativeModernHotkeys.centerGroup(49))
        let stuck = NativeModernJob.Host(deliver: { game.deliver($0) }, pendingInput: { 1 }, frames: { game.frames },
                                         snapshot: { game.snapshot }, pointer: { (0, 0) })
        job.pump(stuck)
        for _ in 0...NativeModernJob.frameBudget + 1 { game.frames += 1; job.pump(stuck) }
        try check(job.state == .aborted && job.delivered.last?.step == .key(vkey: 16, down: false), "Budget abort kept Shift held")
    }

    static func snapshotReader() throws {
        // Byte-level reader over a fake projection: addresses/sizes exactly as requested.
        var memory: [UInt32: [UInt8]] = [:]
        func put(_ address: UInt32, _ bytes: [UInt8]) { memory[address] = bytes }
        func le16(_ v: Int) -> [UInt8] { [UInt8(v & 255), UInt8(v >> 8 & 255)] }
        func le32(_ v: Int) -> [UInt8] { (0..<4).map { UInt8(truncatingIfNeeded: v >> (8 * $0)) } }
        put(0x4e2f78, [0x12]); put(0x508041, [1]); put(0x508720, le16(10) + le16(11) + le16(25) + le16(17))
        put(0x497168, le16(128) + le16(96)); put(0x508f90, [0]); put(0x497285, [1])
        put(0x507d68, le16(660) + le16(400)); put(0x496fe0, le16(128) + le16(96)); put(0x50a9b4, le16(9) + le16(11))
        put(0x4e19a0, le32(0x700000)); put(0x4e0f38, le32(0x800000))
        var zones: [UInt8] = []
        for (x0, y0, x1, y1) in [(0, 0, 0, 0), (660, 400, 788, 496), (0, 544, 799, 599)] { zones += le32(x0) + le32(y0) + le32(x1) + le32(y1) }
        put(0x5099b4 + 16 * 9, zones); put(0x508728 + 512, le16(2)); put(0x495be0 + 2, le16(0xffff))
        var units = [UInt8](repeating: 0, count: 800 * 276)
        units[276 * 3] = 1; units[276 * 3 + 1] = 5; units[276 * 3 + 2] = 7; units[276 * 3 + 8] = 12; units[276 * 3 + 9] = 12; units[276 * 3 + 0x72] = 1
        put(0x700000, units)
        var structures = [UInt8](repeating: 0, count: 1000 * 276)
        let o = 276 * 5; structures[o + 2] = 40; structures[o + 3] = 30; structures[o + 4] = 1; structures[o + 5] = 5; structures[o + 8] = 2
        put(0x49a0c8, structures)
        put(0x495d8c + 104, le16(2) + le16(1))
        var grid = [UInt8](repeating: 0, count: 2 * 256 * 96)
        let word = 0x8000 | 5 << 5
        for cell in [30 << 8 | 41] { grid[2 * cell] = UInt8(word & 255); grid[2 * cell + 1] = UInt8(word >> 8) }
        put(0x800000, grid)
        var requests = 0
        let snapshot = Snapshot.read(concatenated: { addresses, sizes in
            requests += 1
            var out: [UInt8] = []
            for (a, n) in zip(addresses, sizes) {
                guard let bytes = memory[a], bytes.count >= Int(n) else { return nil }
                out += bytes.prefix(Int(n))
            }
            return out
        })
        guard let s = snapshot else { throw Failure(message: "Snapshot unreadable") }
        try check(requests == 3, "One projection per stage of the reader")
        try check(s.inMission && s.localPlayer == 1 && (s.viewX, s.viewY, s.viewW, s.viewH) == (10, 11, 25, 17), "Scalars")
        try check(s.zones.map(\.id) == [9, 10, 11] && s.zone(atX: 700, y: 450) == 10 && s.zone(atX: 100, y: 100) == 0, "Zones")
        try check(s.selectedUnits == 2 && s.selectedStructure == 0xffff, "Selection")
        try check(s.units == [.init(index: 3, owner: 1, kind: 5, state: 7, cell: 12 << 8 | 12, onMap: true)], "Units")
        try check(s.structures == [.init(index: 5, owner: 1, type: 2, state: 5, cells: [30 << 8 | 41])], "Sanctuary solid cells")
        memory[0x4e2f78] = [0x16]
        let menu = Snapshot.read(concatenated: { addresses, sizes in
            zip(addresses, sizes).flatMap { memory[$0]!.prefix(Int($1)) }
        })
        try check(menu?.inMission == false, "Non-mission state read as a mission")
    }

    /// Order queue markers: selected units' waypoints in view pixels, one read.
    static func orderQueueMarkers() throws {
        var bytes = [UInt8](repeating: 0, count: 1 + 1 + 8 + 2048)
        bytes[0] = 0x12; bytes[1] = 2
        for (o, v) in [(2, 10), (4, 20), (6, 25), (8, 17)] { bytes[o] = UInt8(v) }
        let list = 10 + 512 * 2
        bytes[list] = 3; bytes[list + 2] = 7; bytes[list + 4] = 9; bytes[list + 6] = 0x10; bytes[list + 7] = 0x01
        var reads = 0
        let queues: [Int: [UInt16]] = [7: [21 << 8 | 11, 30 << 8 | 40], 0x110: [21 << 8 | 10]]
        let peek: ([UInt32], [UInt32]) -> [UInt8]? = { addresses, sizes in
            reads += 1
            return addresses == NativeOrderQueueMarkers.addresses && sizes == NativeOrderQueueMarkers.sizes ? bytes : nil
        }
        let m = NativeOrderQueueMarkers.read(peek: peek, queue: { queues[$0] ?? [] })
        try check(reads == 1, "One snapshot per refresh")
        try check(m?.mapWidth == 800 && m?.mapHeight == 544, "Map region from the view size")
        try check(m?.paths == [[.init(x: 48, y: 48), .init(x: 976, y: 336)], [.init(x: 16, y: 48)]],
                  "Waypoint pixels per selected unit (unit 9 has none)")
        bytes[0] = 0x16
        try check(NativeOrderQueueMarkers.read(peek: peek, queue: { queues[$0] ?? [] }) == .empty, "No markers outside a mission")
        try check(NativeOrderQueueMarkers.read(peek: { _, _ in nil }, queue: { _ in [] }) == nil, "Refused peek")
        try check(NativeModernHotkeys.orderQueueRule == 0x2, "RL_REGRA_FILA_ORDENS")
    }

    /// "Caminhos melhores": off by default, bit 0, combined with the order queue bit.
    static func engineRules() throws {
        let suite = "RisingLandsModernPathsTests-\(ProcessInfo.processInfo.processIdentifier)"
        guard let defaults = UserDefaults(suiteName: suite) else { throw Failure(message: "defaults suite") }
        defer { defaults.removePersistentDomain(forName: suite) }
        try check(NativeModernPaths.preferenceKey == "RisingLandsModernPaths", "Key shared with the Options screen")
        try check(NativeModernPaths.rule == 0x1, "RL_REGRA_CAMINHOS_MELHORES")
        try check(!NativeModernPaths.isEnabled(defaults), "Caminhos melhores off by default")
        try check(NativeEngineRules.current(defaults) == 0x2, "Default: only the order queue (Atalhos modernos on)")
        NativeModernPaths.store(true, defaults)
        try check(NativeEngineRules.current(defaults) == 0x3, "Both rules")
        NativeModernHotkeys.store(false, defaults)
        try check(NativeEngineRules.current(defaults) == 0x1, "Modern paths alone")
        NativeModernPaths.store(false, defaults)
        try check(NativeEngineRules.current(defaults) == 0, "Original game")
    }

    static func main() throws {
        try routerCommands(); print("PASS router: modern keys, exact defaults, double tap only for original groups 1-9")
        try fixedSteps(); print("PASS fixed steps: original Shift+N and X only, foreign channels refused")
        try snapshotGeometry(); print("PASS geometry: minimap_center replica, zone-0 click pixels, margin, hidden minimap")
        try planner(); print("PASS planner: idle worker/army/sanctuary cycle, verified deselect, single recentre")
        try jobs(); print("PASS jobs: Shift over the consumed digit, staged delivery, cursor return, aborts, budget")
        try snapshotReader(); print("PASS snapshot reader: one concatenated projection per stage")
        try orderQueueMarkers(); print("PASS order queue markers: selected units' waypoints, one snapshot, map region")
        try engineRules(); print("PASS engine rules: Caminhos melhores off by default, bit 0 with the order queue bit 1")
        print("ModernHotkeysTests: 7 groups passed; no engine, no window")
    }
}
