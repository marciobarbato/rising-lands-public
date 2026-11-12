import AppKit
import Metal

@main
@MainActor
struct ControlsConsumerTests {
    // Deterministic consumer test. macOS focus granting is a separate UI smoke
    // (run on the Mac mini). This test never activates the app, never orders a
    // window in and never takes focus from the user's frontmost application.
    final class FocusWindow:NSWindow {
        var focused=true
        override var isKeyWindow:Bool { focused }
    }
    static func main() {
        _ = NSApplication.shared
        NSApp.setActivationPolicy(.prohibited)
        DispatchQueue.main.asyncAfter(deadline:.now()+0.2) {
            do { try run();exit(0) }
            catch { FileHandle.standardError.write(Data("Consumer failed: \(error)\n".utf8));exit(1) }
        }
        NSApp.run()
    }
    static func run() throws {
        let view=try NativeMetalGameView(nativeFrame:.zero)
        let window=FocusWindow(contentRect:NSRect(x:0,y:0,width:1100,height:805),styleMask:[.titled,.resizable],backing:.buffered,defer:false)
        view.isHidden=false
        let layout=NativeGameLayout(view)
        window.contentView=layout
        let frontmost=NSWorkspace.shared.frontmostApplication?.processIdentifier
        RunLoop.current.run(until:Date().addingTimeInterval(0.3))
        checkFocus: do { precondition(window.makeFirstResponder(view),"game responder refused") }
        var events=[[String:Any]]()
        view.inputEvent={events.append($0)}
        func check(_ value:Bool,_ name:String){precondition(value,name)}
        func key(_ code:UInt16,_ down:Bool,_ flags:NSEvent.ModifierFlags=[]) {
            let event=NSEvent.keyEvent(with:down ? .keyDown:.keyUp,location:.zero,modifierFlags:flags,timestamp:0,windowNumber:window.windowNumber,context:nil,characters:"",charactersIgnoringModifiers:"",isARepeat:false,keyCode:code)!
            if down {view.keyDown(with:event)} else {view.keyUp(with:event)}
        }
        key(36,true);key(76,true);key(36,false)
        check(events.count==1 && view.heldKeys==[13],"alias retained until last key up")
        key(76,false);check(events.count==2 && view.heldKeys.isEmpty,"alias released")
        key(123,true,.control);window.focused=false;view.activateKeys()
        check(view.heldKeys.isEmpty && events.suffix(2).allSatisfy{$0["down"] as? Bool == false},"focus releases modifiers and keys through same sink")
        window.focused=true
        var candidate=NativeHotkeyBindings();try candidate.rebind(.left,to:[.init(keyCode:0)])
        try view.hotkeys.prepareInstallation(candidate)()
        key(0,true);check(view.heldKeys==[37],"rebound physical input reaches game virtual key")
        key(0,false,.command);check(view.heldKeys.isEmpty,"Command key up cannot strand rebound key")
        let before=events.count;key(0,true,.command);check(events.count==before,"Command never injected")
        for mode in [NativeViewportPolicy.Mode.continuousFit,.integerFit] {
            layout.scaleMode=mode;layout.layoutSubtreeIfNeeded()
            let policy=try NativeViewportPolicy(contentRect: CGRect(x:0,y:105,width:layout.bounds.width,height:layout.bounds.height-105),backingScale:window.backingScaleFactor,mode:mode,windowYAxis:.up)
            check(view.frame==policy.viewportInWindow,"real layout adopts policy")
            for pixel in [NativeViewportPolicy.Pixel(x:0,y:0),.init(x:799,y:599),.init(x:400,y:272)] {
                let p=view.viewport!.windowPoint(forOriginalPixel:pixel)!
                let event=NSEvent.mouseEvent(with:.mouseMoved,location:view.convert(p,to:nil),modifierFlags:[],timestamp:0,windowNumber:window.windowNumber,context:nil,eventNumber:0,clickCount:0,pressure:0)!
                view.mouseMoved(with:event)
                check(events.last?["x"] as? Int==pixel.x && events.last?["y"] as? Int==pixel.y,"mouse hits rendered original pixel")
            }
            let count=events.count
            let outside=NSEvent.mouseEvent(with:.leftMouseDown,location:view.convert(NSPoint(x:-1,y:1),to:nil),modifierFlags:[],timestamp:0,windowNumber:window.windowNumber,context:nil,eventNumber:0,clickCount:1,pressure:1)!
            view.mouseDown(with:outside);check(events.count==count,"letterbox rejects input")
        }
        // Continuous left drag: the press is held, drags beyond the surface clamp
        // to its edge, the release carries the final pixel and ends the press.
        layout.scaleMode = .continuousFit;layout.layoutSubtreeIfNeeded()
        func mouse(_ type:NSEvent.EventType,_ point:NSPoint,clicks:Int=1)->NSEvent {
            NSEvent.mouseEvent(with:type,location:point,modifierFlags:[],timestamp:0,windowNumber:window.windowNumber,context:nil,eventNumber:0,clickCount:clicks,pressure:1)!
        }
        let start=view.convert(view.viewport!.windowPoint(forOriginalPixel:.init(x:100,y:120))!,to:nil)
        let beyond=view.convert(NSPoint(x:-50,y:view.bounds.height+50),to:nil)
        events.removeAll()
        view.mouseDown(with:mouse(.leftMouseDown,start))
        check(view.leftHeld && events.last?["msg"] as? Int==0x201 && events.last?["x"] as? Int==100,"press starts a held drag")
        view.mouseDragged(with:mouse(.leftMouseDragged,beyond))
        check(events.last?["msg"] as? Int==0x200 && events.last?["x"] as? Int==0 && events.last?["y"] as? Int==0,"held drag clamps to the surface corner")
        view.mouseUp(with:mouse(.leftMouseUp,beyond))
        check(!view.leftHeld && events.last?["msg"] as? Int==0x202 && events.last?["x"] as? Int==0 && events.last?["cancel"]==nil,"release outside ends the press at the edge")
        let count=events.count
        view.mouseDragged(with:mouse(.leftMouseDragged,beyond));view.mouseUp(with:mouse(.leftMouseUp,beyond))
        check(events.count==count,"no held press: outside drag/release stay rejected")
        // A plain move (no button) over the letterbox clamps to the surface edge, as the
        // original's fullscreen pointer rested on the 2-px edge-scroll zones.
        view.mouseMoved(with:mouse(.mouseMoved,beyond,clicks:0))
        check(events.count==count+1 && events.last?["msg"] as? Int==0x200 && events.last?["x"] as? Int==0 && events.last?["y"] as? Int==0,"hover beyond the surface clamps to its edge")
        // Losing the responder ends a held press through the same input sink.
        var cancelled=0;view.inputCancelled={cancelled+=1}
        view.mouseDown(with:mouse(.leftMouseDown,start));view.mouseDragged(with:mouse(.leftMouseDragged,view.convert(view.viewport!.windowPoint(forOriginalPixel:.init(x:300,y:200))!,to:nil)))
        check(window.makeFirstResponder(nil),"responder change")
        check(!view.leftHeld && cancelled==1 && events.last?["msg"] as? Int==0x202 && events.last?["x"] as? Int==300 && events.last?["y"] as? Int==200 && events.last?["cancel"] as? Bool==true,"focus loss releases the held press at its last pixel")
        check(window.makeFirstResponder(view),"responder restored")
        let settled=events.count
        view.cancelInput();check(cancelled==2 && events.count==settled,"cancel without a held press sends no release")
        // Atalhos modernos: a command waits for the player's held button, and while a
        // synthetic mouse command runs the player's mouse only moves the remembered pointer.
        var modern=[NativeHotkeyInputRouter.ModernCommand]();view.modernCommand={modern.append($0)}
        view.hotkeys.modernEnabled=true
        view.mouseDown(with:mouse(.leftMouseDown,start))
        key(47,true);key(47,false)
        check(modern.isEmpty,"modern command delivered during a held press")
        view.mouseUp(with:mouse(.leftMouseUp,start))
        check(modern==[.nextIdleWorker] && events.last?["msg"] as? Int==0x202,"modern command after the release")
        key(115,true);key(115,false);check(modern.last == .lastAlert,"Home is the last-alert command")
        view.syntheticUntil=ProcessInfo.processInfo.systemUptime+30
        let quiet=events.count
        let target=view.convert(view.viewport!.windowPoint(forOriginalPixel:.init(x:222,y:111))!,to:nil)
        view.mouseMoved(with:mouse(.mouseMoved,target,clicks:0));view.mouseDown(with:mouse(.leftMouseDown,target));view.mouseUp(with:mouse(.leftMouseUp,target))
        check(events.count==quiet && !view.leftHeld && view.pointer.get()==(222,111),"player mouse interleaved with a synthetic command")
        view.syntheticUntil=0
        view.mouseMoved(with:mouse(.mouseMoved,target,clicks:0));check(events.count==quiet+1,"mouse returns after the command")
        view.hotkeys.modernEnabled=false
        key(47,true);key(47,false);check(modern.count==2,"disabled modern keys still command")
        view.modernCommand=nil
        // Fullscreen (immersive): only the game, on black; controls return with a stop panel.
        let control=NSButton(title:"Pausar",target:nil,action:nil);control.frame=NSRect(x:15,y:36,width:100,height:27);layout.addSubview(control)
        let panel=NSView(frame:.zero);panel.isHidden=true;layout.addSubview(panel);layout.stopPanel=panel
        layout.immersive=true;layout.layoutSubtreeIfNeeded()
        let full=try NativeViewportPolicy(contentRect:CGRect(x:0,y:0,width:layout.bounds.width,height:layout.bounds.height),backingScale:window.backingScaleFactor,mode:.continuousFit,windowYAxis:.up)
        check(control.isHidden && view.frame==full.viewportInWindow,"immersive layout hides controls and uses the whole content")
        panel.isHidden=false;layout.needsLayout=true;layout.layoutSubtreeIfNeeded()
        check(!control.isHidden && view.frame.minY>=275,"stop report brings controls back in fullscreen")
        panel.isHidden=true;layout.immersive=false;layout.layoutSubtreeIfNeeded()
        check(!control.isHidden && view.frame.minY>=105,"windowed layout restores the control bar")
        // Player edition layout: never a control bar; a stop report takes the bottom.
        layout.player=true;layout.layoutSubtreeIfNeeded()
        check(control.isHidden && view.frame==full.viewportInWindow,"player layout has no control bar and uses the whole content")
        panel.isHidden=false;layout.needsLayout=true;layout.layoutSubtreeIfNeeded()
        check(control.isHidden && view.frame.minY>=175 && panel.frame.minY==10,"player stop report sits below the game without controls")
        panel.isHidden=true;layout.player=false;layout.layoutSubtreeIfNeeded()
        check(!control.isHidden && view.frame.minY>=105,"development layout keeps its control bar")
        // Smooth filter over a hard vertical edge: off, only the two palette colors
        // reach the drawable; on, the edge blends while flat areas stay exact.
        func presentedRow(smooth:Bool) throws -> [UInt8] {
            var pixels=[UInt8](repeating:0,count:480000);for y in 0..<600 {for x in 400..<800 {pixels[y*800+x]=1}}
            var palette=[UInt8](repeating:0,count:768);palette[3]=255;palette[4]=255;palette[5]=255
            view.smoothFilter=smooth;try view.present(pixels,palette)
            let texture=view.lastTexture!,row=((texture.width*4+255)/256)*256
            let buffer=view.device.makeBuffer(length:row*texture.height,options:.storageModeShared)!
            let command=view.commands.makeCommandBuffer()!,blit=command.makeBlitCommandEncoder()!
            blit.copy(from:texture,sourceSlice:0,sourceLevel:0,sourceOrigin:MTLOrigin(x:0,y:0,z:0),sourceSize:MTLSize(width:texture.width,height:texture.height,depth:1),to:buffer,destinationOffset:0,destinationBytesPerRow:row,destinationBytesPerImage:row*texture.height)
            blit.endEncoding();command.commit();command.waitUntilCompleted()
            let bytes=buffer.contents().assumingMemoryBound(to:UInt8.self),middle=texture.height/2
            return (0..<texture.width).map{bytes[middle*row+$0*4]}
        }
        layout.scaleMode = .continuousFit;layout.layoutSubtreeIfNeeded()
        let sharp=try presentedRow(smooth:false),soft=try presentedRow(smooth:true)
        view.smoothFilter=false
        check(view.frame.width*window.backingScaleFactor>800,"the edge test needs an upscaled surface")
        check(Set(sharp)==[0,255],"sharp presentation shows only exact palette colors")
        check(soft.contains{$0>0 && $0<255},"smooth presentation blends the edge")
        check(soft.first==0 && soft.last==255,"smooth presentation keeps flat areas exact")
        view.hidesSystemCursor=true;view.hidesSystemCursor=false
        check(NSCursor.current == NSCursor.arrow,"a stop restores the visible arrow immediately")
        check(!NSApp.isActive && !window.isVisible && NSApp.windows.allSatisfy{!$0.isVisible},"never activates or shows a window")
        check(NSWorkspace.shared.frontmostApplication?.processIdentifier != ProcessInfo.processInfo.processIdentifier && NSWorkspace.shared.frontmostApplication?.processIdentifier == frontmost,"user's frontmost application keeps focus")
        print("ControlsConsumerTests: non-activating, no window on screen")
        print("ControlsConsumerTests: real NSView aliases, controlled focus, rebind, Command, 2 layouts, 6 mouse inverses, letterbox, held drag/clamp/release, focus release and fullscreen layout passed")
    }
}
