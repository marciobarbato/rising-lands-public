import Foundation

/// Physical input lifetime only. The host retains menu dispatch, mouse mapping,
/// frame stamping, recording and engine injection. Call on the main actor;
/// the synchronous, infallible sink must not reenter this owner.
@MainActor
final class NativeHotkeyInputRouter {
    typealias Bindings = NativeHotkeyBindings
    typealias Modifiers = Bindings.Modifiers
    typealias Commit = @MainActor () -> Void

    struct KeyEvent: Equatable, Sendable {
        let virtualKey: Int
        let down: Bool

        /// The existing NativeMetalGameView.inputEvent payload, without a frame.
        var payload: [String: Any] {
            ["kind": "key", "vkey": virtualKey, "down": down]
        }
    }

    enum Disposition: Equatable {
        case handled, unhandled, nativeMenu
    }

    /// "Atalhos modernos" requests. The host turns each into ordinary input the
    /// original already accepts (NativeModernHotkeys.swift); nothing here emits it.
    enum ModernCommand: Equatable, Sendable {
        /// Second tap of digit N: the original's Shift+N (select and centre). vkey 49...57.
        case centerGroup(Int)
        case lastAlert, nextIdleWorker, nextIdleMilitary, mainStructure
    }

    /// Two taps of the same digit within this interval centre on the group.
    static let doubleTapInterval: TimeInterval = 0.35

    private struct Press {
        // Remember unmatched downs too: a modifier change cannot reinterpret one.
        let action: Bindings.Action?
        /// Consumed by a modern command: its release emits nothing.
        var modern = false
    }

    private(set) var bindings: Bindings
    private(set) var isActive = false
    private(set) var heldKeys = Set<Int>()
    private var presses: [UInt16: Press] = [:]
    private var references: [Bindings.Action: Int] = [:]
    private let sink: @MainActor (KeyEvent) -> Void
    /// Off by default in the component; the app sets it from the stored preference.
    var modernEnabled = false
    var modernSink: (@MainActor (ModernCommand) -> Void)?
    private let clock: @MainActor () -> TimeInterval
    private var lastDigitTap: (key: Int, time: TimeInterval)?

    init(bindings: Bindings = Bindings(), clock: @escaping @MainActor () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
         sink: @escaping @MainActor (KeyEvent) -> Void) {
        self.bindings = bindings
        self.clock = clock
        self.sink = sink
    }

    /// Activate only for the active game window with the game view as responder.
    /// Deactivation flushes before the host detaches/replaces the old input sink.
    /// Reactivation never synthesizes downs for keys still physically held.
    func setActive(_ active: Bool) {
        if !active { releaseAllHeldKeys() }
        isActive = active
    }

    @discardableResult
    func keyDown(keyCode: UInt16, modifiers: Modifiers, isRepeat: Bool = false) -> Disposition {
        // Never steal even an unassigned Command shortcut from the native host.
        guard !modifiers.contains(.command) else { return .nativeMenu }
        guard isActive else { return .unhandled }
        if let press = presses[keyCode] {
            return press.action == nil && !press.modern ? .unhandled : .handled
        }
        // Includes repeats arriving after focus loss or a successful rebind flush.
        guard !isRepeat else { return .unhandled }
        synchronizeModifiers(modifiers)
        let action = bindings.action(keyCode: keyCode, modifiers: modifiers)
        if let action, action.isModern {
            guard modernEnabled, let command = Self.command(for: action) else {
                presses[keyCode] = Press(action: nil)
                return .unhandled
            }
            lastDigitTap = nil
            presses[keyCode] = Press(action: nil, modern: true)
            modernSink?(command)
            return .handled
        }
        if let action, let key = action.virtualKey, modernEnabled, (49...57).contains(key), modifiers.isEmpty {
            let now = clock()
            if let last = lastDigitTap, last.key == key, now - last.time <= Self.doubleTapInterval {
                lastDigitTap = nil
                presses[keyCode] = Press(action: nil, modern: true)
                modernSink?(.centerGroup(key))
                return .handled
            }
            lastDigitTap = (key, now)
        } else if action != nil {
            lastDigitTap = nil
        }
        presses[keyCode] = Press(action: action)
        guard let action, let key = action.virtualKey else { return .unhandled }
        let count = references[action, default: 0]
        references[action] = count + 1
        if count == 0 { emit(key, down: true) }
        return .handled
    }

    @discardableResult
    func keyUp(keyCode: UInt16, modifiers: Modifiers) -> Disposition {
        guard isActive else {
            return modifiers.contains(.command) ? .nativeMenu : .unhandled
        }
        // Preserve the committed host's modifier-before-key order. Command must
        // not prevent releasing an earlier gameplay down with a different chord.
        synchronizeModifiers(modifiers)
        let press = presses.removeValue(forKey: keyCode)
        if press?.modern == true {
            return modifiers.contains(.command) ? .nativeMenu : .handled
        }
        let action = press?.action
        if let action, let key = action.virtualKey {
            let count = references[action, default: 0]
            if count > 1 {
                references[action] = count - 1
            } else {
                references.removeValue(forKey: action)
                emit(key, down: false)
            }
        }
        if modifiers.contains(.command) { return .nativeMenu }
        return action == nil ? .unhandled : .handled
    }

    /// Call for flagsChanged and before the existing mouse event callback.
    /// Command itself has no gameplay channel. Shift/Control/Option remain
    /// independent channels, including when no action chord matches.
    func synchronizeModifiers(_ modifiers: Modifiers) {
        guard isActive else { return }
        for (flag, key): (Modifiers, Int) in [(.shift, 16), (.control, 17), (.option, 18)] {
            let down = modifiers.contains(flag)
            if down != heldKeys.contains(key) { emit(key, down: down) }
        }
    }

    /// Idempotent, deterministic release through the SAME sink as normal keys.
    /// Also clears unmatched physical downs and alias counts. No restore downs.
    func releaseAllHeldKeys() {
        lastDigitTap = nil
        presses.removeAll(keepingCapacity: true)
        references.removeAll(keepingCapacity: true)
        for key in heldKeys.sorted() { emit(key, down: false) }
    }

    /// Directly compatible with NativeHotkeyPreferencesSession.Prepare:
    /// prepare -> candidate.save(to:) -> returned synchronous commit.
    /// Bindings is already validated by its value-type API. All additional host
    /// checks/allocations belong in checkHost and MUST be free of side effects.
    /// On a throw, or if save fails and this closure is discarded, NOTHING here
    /// changes. Serialize the entire transaction; do not retain it across an
    /// await, other installation, sink replacement, or editor lifetime.
    func prepareInstallation(_ candidate: Bindings,
                             checkHost: (Bindings) throws -> Void = { _ in }) throws -> Commit {
        try checkHost(candidate)
        var committed = false
        return {
            guard !committed else { return }
            committed = true
            guard self.bindings != candidate else { return }
            self.releaseAllHeldKeys()
            self.bindings = candidate
        }
    }

    static func command(for action: Bindings.Action) -> ModernCommand? {
        switch action {
        case .lastAlert: return .lastAlert
        case .nextIdleWorker: return .nextIdleWorker
        case .nextIdleMilitary: return .nextIdleMilitary
        case .mainStructure: return .mainStructure
        default: return nil
        }
    }

    private func emit(_ key: Int, down: Bool) {
        if down { heldKeys.insert(key) } else { heldKeys.remove(key) }
        sink(KeyEvent(virtualKey: key, down: down))
    }
}
