import AppKit
import Metal
import QuartzCore
import CryptoKit

// Engine publishes to three persistent slots. The display queue owns drawables;
// a slot cannot be overwritten until every GPU reader has completed.
final class NativePlayPresenter {
    struct Slot { let image:MTLTexture;let palette:MTLBuffer;var readers=0 }
    let layer:CAMetalLayer,commands:MTLCommandQueue,pipeline:MTLRenderPipelineState
    let queue=DispatchQueue(label:"risinglands.native.display",qos:.userInteractive)
    let lock=NSLock()
    var slots=[Slot](),latest:Int?,timer:DispatchSourceTimer?
    var lastTexture:MTLTexture?,failure:String?
    var published=0,dropped=0,submitted=0
    var uploadSeconds=0.0,submitSeconds=0.0,gpuSeconds=0.0,drawableWaitSeconds=0.0
    var presented=[Double]()
    var presentedCount=0,epoch=0
    var measurementStart=0.0
    var uniqueGameFrames=0,lastPresentedFrame:UInt64?
    var latestFrame:UInt64=0
    var smoothFilter:UInt32=0
    init(layer:CAMetalLayer,device:MTLDevice,commands:MTLCommandQueue,pipeline:MTLRenderPipelineState) throws {
        self.layer=layer;self.commands=commands;self.pipeline=pipeline;presented.reserveCapacity(16384)
        let descriptor=MTLTextureDescriptor.texture2DDescriptor(pixelFormat:.r8Uint,width:800,height:600,mipmapped:false)
        descriptor.storageMode = .shared;descriptor.usage = .shaderRead
        for _ in 0..<3 {
            guard let image=device.makeTexture(descriptor:descriptor),let palette=device.makeBuffer(length:768,options:.storageModeShared) else{throw NSError(domain:"MetalPlaySlotAllocation",code:1)}
            slots.append(Slot(image:image,palette:palette))
        }
    }
    func publish(_ pixels:UnsafePointer<UInt8>,_ rgb:UnsafePointer<UInt8>,frame:UInt64) {
        let started=ProcessInfo.processInfo.systemUptime
        lock.lock()
        guard let index=slots.indices.first(where:{$0 != latest && slots[$0].readers==0}) else{dropped+=1;lock.unlock();return}
        slots[index].readers = -1 // reserved by the producer; never selected by display
        let slot=slots[index];lock.unlock()
        slot.image.replace(region:MTLRegionMake2D(0,0,800,600),mipmapLevel:0,withBytes:pixels,bytesPerRow:800)
        slot.palette.contents().copyMemory(from:rgb,byteCount:768)
        lock.lock();slots[index].readers=0;latest=index;latestFrame=frame;published+=1
        uploadSeconds+=ProcessInfo.processInfo.systemUptime-started;lock.unlock()
    }
    func start(at time:Double) {
        stop()
        lock.lock();epoch &+= 1;measurementStart=time;presentedCount=0;presented.removeAll(keepingCapacity:true);uniqueGameFrames=0;lastPresentedFrame=nil
        published=0;dropped=0;submitted=0;uploadSeconds=0;submitSeconds=0;gpuSeconds=0;drawableWaitSeconds=0;failure=nil;lock.unlock()
        let timer=DispatchSource.makeTimerSource(queue:queue)
        timer.schedule(deadline:.now(),repeating:1.0/120,leeway:.microseconds(100))
        timer.setEventHandler{[weak self] in autoreleasepool{self?.draw()}}
        self.timer=timer;timer.resume()
    }
    func stop() {
        timer?.cancel();timer=nil
        queue.sync{} // never called by the display queue
        // Drain GPU readers only at pause/reset/capture, never per game frame.
        if let barrier=commands.makeCommandBuffer(){barrier.commit();barrier.waitUntilCompleted()}
    }
    func draw() {
        lock.lock()
        guard let index=latest,failure==nil else{lock.unlock();return}
        slots[index].readers+=1;let slot=slots[index],frame=latestFrame,epoch=self.epoch;var smooth=smoothFilter;lock.unlock()
        let started=ProcessInfo.processInfo.systemUptime
        guard let drawable=layer.nextDrawable(),let command=commands.makeCommandBuffer() else{
            lock.lock();slots[index].readers-=1;failure="MetalDrawableUnavailable";lock.unlock();return
        }
        let acquired=ProcessInfo.processInfo.systemUptime
        let pass=MTLRenderPassDescriptor();pass.colorAttachments[0].texture=drawable.texture
        pass.colorAttachments[0].loadAction = .dontCare;pass.colorAttachments[0].storeAction = .store
        guard let encoder=command.makeRenderCommandEncoder(descriptor:pass) else{
            lock.lock();slots[index].readers-=1;failure="MetalEncoderUnavailable";lock.unlock();return
        }
        encoder.setRenderPipelineState(pipeline);encoder.setFragmentTexture(slot.image,index:0);encoder.setFragmentBuffer(slot.palette,offset:0,index:0);encoder.setFragmentBytes(&smooth,length:4,index:1)
        encoder.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:3);encoder.endEncoding()
        drawable.addPresentedHandler{[weak self] drawable in
            guard let self=self else{return};self.lock.lock()
            guard self.epoch==epoch,drawable.presentedTime>=self.measurementStart else{self.lock.unlock();return}
            self.presentedCount+=1;self.presented.append(drawable.presentedTime);if self.presented.count>16384{self.presented.removeFirst(8192)};if self.lastPresentedFrame != frame{self.uniqueGameFrames+=1;self.lastPresentedFrame=frame};self.lock.unlock()
        }
        command.addCompletedHandler{[weak self] result in
            guard let self=self else{return};self.lock.lock();self.slots[index].readers-=1
            if result.status != .completed{self.failure="MetalCommandFailed: \(result.error?.localizedDescription ?? "GPU command failed")"}
            self.gpuSeconds+=max(0,result.gpuEndTime-result.gpuStartTime);self.lock.unlock()
        }
        command.present(drawable);command.commit()
        lock.lock();lastTexture=drawable.texture;submitted+=1;drawableWaitSeconds+=acquired-started;submitSeconds+=ProcessInfo.processInfo.systemUptime-acquired;lock.unlock()
    }
    func clear(){lock.lock();latest=nil;lock.unlock()}
    func setSmoothFilter(_ on:Bool){lock.lock();smoothFilter=on ? 1:0;lock.unlock()}
    /// Owner queue only (the game worker), like start/stop.
    var isPresenting:Bool{timer != nil}
    func currentFailure()->String?{lock.lock();defer{lock.unlock()};return failure}
    func metrics(start:Double,end:Double,detailed:Bool=false)->[String:Any] {
        lock.lock();defer{lock.unlock()}
        // Actual CAMetalDrawable presentation timestamps, not timer/submit counts.
        let times=presented.filter{$0>=start && $0<=end},seconds=max(0.000001,end-start)
        let firstBin=presentedCount>presented.count ? max(0,Int((presented.first ?? start)-start)+1):0
        let count=presentedCount-presented.filter{$0>end}.count
        let bins=(firstBin..<(detailed ? max(firstBin,Int(seconds)):firstBin)).map{b in times.filter{$0>=start+Double(b) && $0<start+Double(b+1)}.count}
        return ["presented":count,"displayFPS":Double(count)/seconds,"minOneSecondFPS":bins.min() ?? 0,"oneSecondPresentations":bins,"traceFirstSecond":firstBin,
                "uniqueGameFramesPresented":uniqueGameFrames,"published":published,"publicationDrops":dropped,"submitted":submitted,
                "uploadSeconds":uploadSeconds,"submitSeconds":submitSeconds,"drawableWaitSeconds":drawableWaitSeconds,"gpuSeconds":gpuSeconds,"displayError":failure as Any? ?? NSNull()]
    }
}

final class NativeMetalGameView: NSView {
    let device: MTLDevice
    let commands: MTLCommandQueue
    let pipeline: MTLRenderPipelineState
    let indexed: MTLTexture
    let palette: MTLBuffer
    var presentedFrames=0
    var playPresenter:NativePlayPresenter?
    var testPresented=[Double]()
    let testLock=NSLock()
    var lastTexture: MTLTexture?
    var inputEvent: (([String:Any])->Void)?
    var inputCancelled: (()->Void)?
    var tracking: NSTrackingArea?
    lazy var hotkeys: NativeHotkeyInputRouter = {
        let router=NativeHotkeyInputRouter { [weak self] event in self?.inputEvent?(event.payload) }
        router.modernEnabled=NativeModernHotkeys.isEnabled()
        router.modernSink={ [weak self] command in self?.requestModern(command) }
        return router
    }()
    /// "Atalhos modernos": the window controller turns a command into ordinary input.
    var modernCommand: ((NativeHotkeyInputRouter.ModernCommand)->Void)?
    /// Player pointer for the end of a synthetic command (read on the work queue).
    let pointer=NativeModernPointer()
    /// While a synthetic mouse command runs, the player's mouse only moves the
    /// remembered pointer, so no player event interleaves with it. Expires on its own.
    var syntheticUntil:TimeInterval=0
    var syntheticActive:Bool { ProcessInfo.processInfo.systemUptime<syntheticUntil }
    private var pendingModern:NativeHotkeyInputRouter.ModernCommand?
    /// A command waits for the player's held button to be released.
    func requestModern(_ command:NativeHotkeyInputRouter.ModernCommand){
        if leftHeld{pendingModern=command}else{modernCommand?(command)}
    }
    var heldKeys: Set<Int> { hotkeys.heldKeys }
    var viewport: NativeViewportPolicy? {
        try? NativeViewportPolicy(contentRect: bounds, backingScale: window?.backingScaleFactor ?? 1,
                                  mode: .continuousFit, windowYAxis: .up)
    }
    func activateKeys() { hotkeys.setActive(window?.isKeyWindow == true && window?.firstResponder === self) }
    override func becomeFirstResponder() -> Bool { let accepted=super.becomeFirstResponder();if accepted { hotkeys.setActive(window?.isKeyWindow == true) };return accepted }
    override var acceptsFirstResponder: Bool { true }
    override func updateTrackingAreas() {
        super.updateTrackingAreas();if let area=tracking{removeTrackingArea(area)}
        let area=NSTrackingArea(rect:.zero,options:[.mouseMoved,.mouseEnteredAndExited,.activeInKeyWindow,.inVisibleRect],owner:self,userInfo:nil)
        tracking=area;addTrackingArea(area)
    }
    // A left press that started on the game surface is held until its release
    // (continuous drag selection); its drag and release clamp to the surface edge.
    private(set) var leftHeld=false
    var lastPixel=NativeViewportPolicy.Pixel(x:400,y:300)
    // A plain move over the letterbox bars, or out of the window, also clamps: the
    // original's fullscreen pointer could not leave 800x600, and its edge-scroll
    // zones 1..8 (InitGameZone 0x471830) are only 2 px wide, so the edge pixel is
    // what it saw at the screen border. Clicks outside the surface stay ignored.
    // A drag with no held press (it started outside the surface) stays rejected.
    func mouse(_ event:NSEvent,_ message:Int,held:Bool=false,hover:Bool=false) {
        let p=convert(event.locationInWindow,from:nil)
        guard let pixel=held || hover ? viewport?.clampedOriginalPixel(atWindowPoint:p):viewport?.originalPixel(atWindowPoint:p) else{return}
        if syntheticActive{lastPixel=pixel;pointer.set(pixel.x,pixel.y);return}
        if message != 0x200{window?.makeFirstResponder(self)}
        activateKeys();synchronizeModifiers(event.modifierFlags)
        if message==0x201 || message==0x203{leftHeld=true}else if message==0x202{leftHeld=false}
        lastPixel=pixel;pointer.set(pixel.x,pixel.y)
        let w=(event.modifierFlags.contains(.shift) ? 4:0)|(event.modifierFlags.contains(.control) ? 8:0)
        inputEvent?(["kind":"mouse","msg":message,"x":pixel.x,"y":pixel.y,"wParam":w])
        if message==0x202,let command=pendingModern{pendingModern=nil;modernCommand?(command)}
    }
    /// Focus loss, sheets and window close end a held press at its last pixel,
    /// through the same recorded input path as a physical release.
    func releaseMouse(){
        guard leftHeld else{return}
        leftHeld=false;pendingModern=nil
        inputEvent?(["kind":"mouse","msg":0x202,"x":lastPixel.x,"y":lastPixel.y,"wParam":0,"cancel":true])
    }
    func cancelInput(){releaseMouse();inputCancelled?()}
    override func mouseMoved(with event:NSEvent){mouse(event,0x200,hover:true)}
    /// In fullscreen only, leaving the view through a side (the menu bar or Dock
    /// reveal) keeps the pointer on that edge, as with the original's pinned
    /// pointer. In a window the map scrolls only while the cursor is inside it
    /// (user decision, 2026-11-10); moving onto the host bar never scrolls.
    override func mouseExited(with event:NSEvent){
        guard window?.isKeyWindow==true,window?.styleMask.contains(.fullScreen)==true,let content=window?.contentView,
              !content.bounds.contains(content.convert(event.locationInWindow,from:nil)) else{return}
        mouse(event,0x200,held:leftHeld,hover:true)
    }
    /// Focus loss pulls a pointer resting in an edge-scroll zone 2 px inward, so the
    /// map does not keep scrolling while the player is in another app.
    func leaveScrollEdge(){
        let x=min(797,max(2,lastPixel.x)),y=min(597,max(2,lastPixel.y))
        guard x != lastPixel.x || y != lastPixel.y else{return}
        lastPixel=NativeViewportPolicy.Pixel(x:x,y:y);pointer.set(x,y)
        inputEvent?(["kind":"mouse","msg":0x200,"x":x,"y":y,"wParam":0])
    }
    override func mouseDragged(with event:NSEvent){mouse(event,0x200,held:leftHeld)}
    override func rightMouseDragged(with event:NSEvent){mouse(event,0x200,held:leftHeld)}
    override func mouseDown(with event:NSEvent){mouse(event,event.clickCount>1 && event.clickCount%2==0 ? 0x203:0x201)}
    override func mouseUp(with event:NSEvent){mouse(event,0x202,held:leftHeld)}
    override func rightMouseDown(with event:NSEvent){mouse(event,0x204)}
    override func rightMouseUp(with event:NSEvent){mouse(event,0x205)}
    /// Host conveniences for the original page arrows (objective/briefing): the
    /// scroll wheel and PageUp/PageDown become one page each; the worker maps them
    /// onto a click on the original arrow zone. Gameplay ignores them.
    var wheelAccumulator=0.0
    override func scrollWheel(with event:NSEvent){
        guard event.momentumPhase.isEmpty else{return}
        if event.phase.contains(.began) || event.phase.contains(.ended){wheelAccumulator=0}
        let step=event.hasPreciseScrollingDeltas ? 24.0:1.0
        wheelAccumulator+=Double(event.scrollingDeltaY)
        while abs(wheelAccumulator)>=step {
            let previous=wheelAccumulator>0;wheelAccumulator+=previous ? -step:step
            inputEvent?(["kind":"page","next":!previous])
        }
    }
    override func keyDown(with event:NSEvent){
        activateKeys()
        if event.keyCode==116 || event.keyCode==121 { if !event.isARepeat{inputEvent?(["kind":"page","next":event.keyCode==121])};return }
        if hotkeys.keyDown(keyCode:event.keyCode,modifiers:NativeHotkeyLabels.modifiers(event.modifierFlags),isRepeat:event.isARepeat) != .handled { super.keyDown(with:event) }
    }
    override func keyUp(with event:NSEvent){
        if event.keyCode==116 || event.keyCode==121 {return}
        if hotkeys.keyUp(keyCode:event.keyCode,modifiers:NativeHotkeyLabels.modifiers(event.modifierFlags)) != .handled { super.keyUp(with:event) }
    }
    func synchronizeModifiers(_ flags:NSEvent.ModifierFlags){hotkeys.synchronizeModifiers(NativeHotkeyLabels.modifiers(flags))}
    override func flagsChanged(with event:NSEvent){activateKeys();synchronizeModifiers(event.modifierFlags)}
    override func resignFirstResponder()->Bool{cancelInput();hotkeys.setActive(false);return super.resignFirstResponder()}
    func releaseKeys(){cancelInput();hotkeys.releaseAllHeldKeys()}
    // The game draws its own pointer into the surface; while it runs, the system
    // arrow is hidden over the game so only the original cursor is visible.
    static let invisibleCursor:NSCursor={
        let image=NSImage(size:NSSize(width:16,height:16));image.lockFocus();NSColor.clear.set();NSRect(x:0,y:0,width:16,height:16).fill();image.unlockFocus()
        return NSCursor(image:image,hotSpot:.zero)
    }()
    /// Removing the invisible cursor rect alone leaves the invisible cursor set
    /// until the pointer next crosses a rect: a stopped game would then show
    /// neither cursor. Showing the arrow is therefore explicit.
    var hidesSystemCursor=false { didSet {
        guard oldValue != hidesSystemCursor else{return}
        window?.invalidateCursorRects(for:self)
        if !hidesSystemCursor { NSCursor.arrow.set() }
        else if let window, bounds.contains(convert(window.mouseLocationOutsideOfEventStream,from:nil)) { Self.invisibleCursor.set() }
    } }
    override func resetCursorRects(){ if hidesSystemCursor { addCursorRect(bounds,cursor:Self.invisibleCursor) } }
    override var wantsUpdateLayer: Bool { true }
    override func makeBackingLayer() -> CALayer { CAMetalLayer() }
    /// Optional bilinear presentation. Off, every drawable pixel is one exact palette color.
    var smoothFilter=false { didSet { playPresenter?.setSmoothFilter(smoothFilter) } }
    init(nativeFrame:NSRect) throws {
        guard let device=MTLCreateSystemDefaultDevice(),let commands=device.makeCommandQueue() else { throw NSError(domain:"MetalUnavailable",code:1) }
        self.device=device;self.commands=commands
        let source="""
        #include <metal_stdlib>
        using namespace metal;
        struct V { float4 position [[position]]; float2 uv; };
        vertex V vert(uint id [[vertex_id]]) {
            float2 p[3]={float2(-1,-1),float2(3,-1),float2(-1,3)};
            V o;o.position=float4(p[id],0,1);o.uv=float2(p[id].x*.5+.5,.5-p[id].y*.5);return o;
        }
        float3 color(texture2d<uint> image,device const uchar *palette,uint2 p) {
            uint i=image.read(p).r*3;return float3(palette[i],palette[i+1],palette[i+2])/255.0;
        }
        fragment float4 frag(V v [[stage_in]],texture2d<uint> image [[texture(0)]],device const uchar *palette [[buffer(0)]],constant uint &smooth [[buffer(1)]]) {
            float2 surface=clamp(v.uv,0.0,1.0)*float2(800,600);
            if(smooth==0) return float4(color(image,palette,min(uint2(surface),uint2(799,599))),1);
            // Colors blend after the palette lookup: indices cannot be interpolated.
            float2 cell=max(surface-0.5,float2(0));
            uint2 a=min(uint2(cell),uint2(799,599)),b=min(a+uint2(1),uint2(799,599));
            float2 t=fract(cell);
            float3 top=mix(color(image,palette,a),color(image,palette,uint2(b.x,a.y)),t.x);
            float3 bottom=mix(color(image,palette,uint2(a.x,b.y)),color(image,palette,b),t.x);
            return float4(mix(top,bottom,t.y),1);
        }
        """
        let library=try device.makeLibrary(source:source,options:nil)
        let descriptor=MTLRenderPipelineDescriptor();descriptor.vertexFunction=library.makeFunction(name:"vert");descriptor.fragmentFunction=library.makeFunction(name:"frag");descriptor.colorAttachments[0].pixelFormat = .bgra8Unorm
        pipeline=try device.makeRenderPipelineState(descriptor:descriptor)
        let texture=MTLTextureDescriptor.texture2DDescriptor(pixelFormat:.r8Uint,width:800,height:600,mipmapped:false);texture.storageMode = .shared;texture.usage = .shaderRead
        guard let indexed=device.makeTexture(descriptor:texture),let palette=device.makeBuffer(length:768,options:.storageModeShared) else { throw NSError(domain:"MetalAllocationFailed",code:1) }
        self.indexed=indexed;self.palette=palette
        super.init(frame:nativeFrame);wantsLayer=true;isHidden=true
        guard let metal=layer as? CAMetalLayer else { throw NSError(domain:"MetalLayerUnavailable",code:1) }
        metal.device=device;metal.pixelFormat = .bgra8Unorm;metal.framebufferOnly=false;metal.displaySyncEnabled=true
    }
    override func layout(){super.layout();updateDrawableSize()}
    override func viewDidChangeBackingProperties(){super.viewDidChangeBackingProperties();updateDrawableSize()}
    func updateDrawableSize(){guard let metal=layer as? CAMetalLayer else{return};let scale=window?.backingScaleFactor ?? 1;metal.drawableSize=CGSize(width:max(1,bounds.width*scale),height:max(1,bounds.height*scale))}
    func makePlayPresenter() throws -> NativePlayPresenter {
        if let presenter=playPresenter{return presenter}
        guard let layer=layer as? CAMetalLayer else{throw NSError(domain:"MetalLayerUnavailable",code:1)}
        let presenter=try NativePlayPresenter(layer:layer,device:device,commands:commands,pipeline:pipeline);presenter.setSmoothFilter(smoothFilter);playPresenter=presenter;return presenter
    }
    required init?(coder:NSCoder){fatalError("init(coder:) unavailable")}
    func present(_ pixels:UnsafePointer<UInt8>,_ rgb:UnsafePointer<UInt8>) throws {
        guard let metal=layer as? CAMetalLayer else { throw NSError(domain:"MetalLayerUnavailable",code:1) }
        let scale=window?.backingScaleFactor ?? 1
        metal.drawableSize=CGSize(width:max(1,bounds.width*scale),height:max(1,bounds.height*scale))
        guard let drawable=metal.nextDrawable(),let command=commands.makeCommandBuffer() else { throw NSError(domain:"MetalDrawableUnavailable",code:1) }
        indexed.replace(region:MTLRegionMake2D(0,0,800,600),mipmapLevel:0,withBytes:pixels,bytesPerRow:800)
        palette.contents().copyMemory(from:rgb,byteCount:768)
        let pass=MTLRenderPassDescriptor();pass.colorAttachments[0].texture=drawable.texture;pass.colorAttachments[0].loadAction = .clear;pass.colorAttachments[0].storeAction = .store
        guard let encoder=command.makeRenderCommandEncoder(descriptor:pass) else { throw NSError(domain:"MetalEncoderUnavailable",code:1) }
        var smooth:UInt32=smoothFilter ? 1:0;encoder.setRenderPipelineState(pipeline);encoder.setFragmentTexture(indexed,index:0);encoder.setFragmentBuffer(palette,offset:0,index:0);encoder.setFragmentBytes(&smooth,length:4,index:1);encoder.drawPrimitives(type:.triangle,vertexStart:0,vertexCount:3);encoder.endEncoding()
        drawable.addPresentedHandler{[weak self] d in guard let self=self else{return};self.testLock.lock();self.testPresented.append(d.presentedTime);self.testLock.unlock()}
        command.present(drawable);command.commit();command.waitUntilCompleted()
        guard command.status == .completed else { throw NSError(domain:"MetalCommandFailed",code:1,userInfo:[NSLocalizedDescriptionKey:command.error?.localizedDescription ?? "GPU command failed"]) }
        lastTexture=drawable.texture;presentedFrames+=1
    }
    func testMetrics(start:Double,end:Double,detailed:Bool)->[String:Any]{
        testLock.lock();defer{testLock.unlock()}
        let seconds=max(0.000001,end-start),times=testPresented.filter{$0>=start && $0<=end}
        let bins=(0..<(detailed ? max(0,Int(seconds)):0)).map{b in times.filter{$0>=start+Double(b) && $0<start+Double(b+1)}.count}
        return ["presented":times.count,"displayFPS":Double(times.count)/seconds,"minOneSecondFPS":bins.min() ?? 0,"oneSecondPresentations":bins]
    }
    // Actual GPU readback, only after pausing; never part of per-frame transport.
    func capture(_ url:URL,pixels:UnsafePointer<UInt8>,palette:UnsafePointer<UInt8>) throws -> [String:Any] {
        guard let texture=lastTexture,let command=commands.makeCommandBuffer(),let encoder=command.makeBlitCommandEncoder() else { throw NSError(domain:"MetalReadbackUnavailable",code:1) }
        let row=((texture.width*4+255)/256)*256
        guard let buffer=device.makeBuffer(length:row*texture.height,options:.storageModeShared) else { throw NSError(domain:"MetalReadbackAllocation",code:1) }
        encoder.copy(from:texture,sourceSlice:0,sourceLevel:0,sourceOrigin:MTLOrigin(x:0,y:0,z:0),sourceSize:MTLSize(width:texture.width,height:texture.height,depth:1),to:buffer,destinationOffset:0,destinationBytesPerRow:row,destinationBytesPerImage:row*texture.height);encoder.endEncoding();command.commit();command.waitUntilCompleted()
        guard command.status == .completed else { throw NSError(domain:"MetalReadbackFailed",code:1) }
        let actual=buffer.contents().assumingMemoryBound(to:UInt8.self)
        var checked=0
        for y in 0..<texture.height { for x in 0..<texture.width {
            let sx=min(799,Int((Double(x)+0.5)*800/Double(texture.width))),sy=min(599,Int((Double(y)+0.5)*600/Double(texture.height)))
            let i=Int(pixels[sy*800+sx])*3,a=y*row+x*4
            guard actual[a]==palette[i+2] && actual[a+1]==palette[i+1] && actual[a+2]==palette[i] && actual[a+3]==255 else { throw NSError(domain:"MetalReadbackPixelMismatch",code:a) }
            checked+=1
        }}
        guard let bitmap=NSBitmapImageRep(bitmapDataPlanes:nil,pixelsWide:texture.width,pixelsHigh:texture.height,bitsPerSample:8,samplesPerPixel:4,hasAlpha:true,isPlanar:false,colorSpaceName:.deviceRGB,bytesPerRow:texture.width*4,bitsPerPixel:32),let out=bitmap.bitmapData else { throw NSError(domain:"MetalEvidenceAllocation",code:1) }
        for y in 0..<texture.height { for x in 0..<texture.width { let a=y*row+x*4,b=(y*texture.width+x)*4;out[b]=actual[a+2];out[b+1]=actual[a+1];out[b+2]=actual[a];out[b+3]=255 }}
        guard let data=bitmap.representation(using:.png,properties:[:]) else { throw NSError(domain:"MetalEvidenceEncoding",code:1) }
        try data.write(to:url,options:.atomic)
        return ["pixelsChecked":checked,"path":url.path,"sha256":SHA256.hash(data:data).map{String(format:"%02x",$0)}.joined()]
    }
}

final class NativeGameLayout:NSView {
    let game:NativeMetalGameView
    var stopPanel:NSView?
    var scaleMode:NativeViewportPolicy.Mode = .continuousFit { didSet { needsLayout=true } }
    /// Fullscreen shows only the game on black; host controls return with a stop
    /// report (or on leaving fullscreen) and stay reachable from the menus.
    var immersive=false { didSet { needsLayout=true } }
    /// Player edition: no host control bar at all; only a stop report takes room.
    var player=false { didSet { needsLayout=true } }
    var controlsHidden:Bool { player || (immersive && stopPanel?.isHidden != false) }
    init(_ game:NativeMetalGameView){self.game=game;super.init(frame:NSRect(x:0,y:0,width:800,height:705));addSubview(game)}
    required init?(coder:NSCoder){fatalError("init(coder:) unavailable")}
    override func layout(){
        super.layout()
        let hide=controlsHidden
        for view in subviews where view !== game && view !== stopPanel { view.isHidden=hide }
        wantsLayer=true;layer?.backgroundColor=immersive || player ? NSColor.black.cgColor:nil
        let reporting=stopPanel?.isHidden == false
        let bottom:CGFloat = player ? (reporting ? 175:0) : hide ? 0:reporting ? 275:105
        let content=CGRect(x:0,y:bottom,width:bounds.width,height:max(1,bounds.height-bottom))
        let backing=window?.backingScaleFactor ?? 1
        // An undersized screen uses explicit aspect fit until 1x fits again.
        let policy=(try? NativeViewportPolicy(contentRect:content,backingScale:backing,mode:scaleMode,windowYAxis:.up))
            ?? (try? NativeViewportPolicy(contentRect:content,backingScale:backing,mode:.continuousFit,windowYAxis:.up))
        if let policy { game.frame=policy.viewportInWindow;game.updateDrawableSize() }
        stopPanel?.frame=NSRect(x:15,y:player ? 10:105,width:bounds.width-30,height:160)
    }
}

final class NativeLiveWindow:NSObject,NSWindowDelegate {
    let root:URL
    let work=DispatchQueue(label:"risinglands.native.live",qos:.userInitiated)
    var validationMode:Bool
    // Effective host rate: the selected speed, or a diagnostic override (0 = sem limite).
    var speed=NativeLiveWindow.initialSpeed
    var gameFPS=NativeLiveWindow.initialSpeed.frameRate { didSet { pacer.setRate(gameFPS,now:ProcessInfo.processInfo.systemUptime) } }
    var pacer=NativeFramePacer(rate:NativeLiveWindow.initialSpeed.frameRate,now:0)
    var runID:UInt64=0
    var presenter:NativePlayPresenter?
    var audio:NativeAudioPlayback?
    // Main-thread owned presentation. Never passed to Rust, saves or replay.
    var cdMusic:NativeCDMusicHost?
    var cdEvents=[RlFrameEvent](repeating:RlFrameEvent(),count:8192)
    func pumpCDMusic() {
        var count:UInt32=0
        let code=macos_live_cd_events(driver,&cdEvents,UInt32(cdEvents.count),&count)
        guard code==0 else { DispatchQueue.main.async{self.cdMusic?.transportFailed(code)};return }
        let requests=cdEvents.prefix(Int(count)).filter{$0.kind==2 && $0.id==102}.map{($0.site,Int($0.registers.0),Int($0.registers.1 & 255))}
        guard !requests.isEmpty else{return}
        DispatchQueue.main.async { [weak self] in
            for (site,track,loop) in requests { self?.cdMusic?.request(site:site,track:track,loop:loop) }
        }
    }
    let musicLabel=NSTextField(labelWithString:"")
    @MainActor func configureCDMusic(disc:URL?=nil) {
        let active=cdMusic?.stageActive ?? false
        cdMusic?.close()
        let player:NativeCDMusicPlayer
        if let disc=disc { player=NativeCDMusicPlayer(originalDiscURL:disc) }
        else {
            // An absent catalog stays an explicit, track-specific error.
            do {
                player=try NativeCDMusicPlayer(originalTrackURLs:[
                    2:root.appendingPathComponent("music/TRACK02.wav"),
                    3:root.appendingPathComponent("music/TRACK03.wav")])
            } catch { cdMusic=nil;musicLabel.stringValue=error.localizedDescription;return }
        }
        let host=NativeCDMusicHost(player:player);cdMusic=host
        host.onChange={ [weak self,weak host] in self?.musicLabel.stringValue=host?.notice ?? "" }
        if active{host.beginStage()}
        musicLabel.stringValue=host.notice
    }
    @MainActor func closeCDMusic(){cdMusic?.close()}
    let modeControl=NSSegmentedControl(labels:["Jogar","Validar"],trackingMode:.selectOne,target:nil,action:nil)
    let rateControl=NSPopUpButton(frame:.zero,pullsDown:false)
    let metal:NativeMetalGameView
    let window:NSWindow
    var driver:OpaquePointer?
    var running=false
    var stoppedReason:String?
    var metalSeconds=0.0
    var playStarted=0.0,playEnded=0.0
    var startFrames:UInt64=0,startLogic:UInt64=0,startRender:UInt64=0,startProof:UInt64=0
    var startMetal=0.0
    var lastNotify=0.0
    var update:(([String:Any])->Void)?
    var recordedEvent:(([String:Any])->Void)?
    var recordingAction:(()->Void)?
    var exportAction:(()->Void)?
    var recording=false
    var benchmarkInput=false
    /// In-game FLC events (lane B): the payload's events folder beside native-inputs.
    lazy var eventFLC=NativeEventFLCPlayback(folder:root.appendingPathComponent("events",isDirectory:true))
    /// Main thread sets it; the worker reads it for periodic notifications.
    var periodicSnapshots=true
    var benchmarkOrderTags=[UInt32]()
    var orderBuffer=[UInt32](repeating:0,count:32)
    let recordButton=NSButton(title:"Nova gravação",target:nil,action:nil)
    let resetButton=NSButton(title:"Reiniciar",target:nil,action:nil)
    let stepButton=NSButton(title:"Avançar quadro",target:nil,action:nil)
    let stopPanel=NSView(frame:NSRect(x:15,y:105,width:770,height:160))
    let stopText=NSTextView()
    let copyStopButton=NSButton(title:"Copiar relatório",target:nil,action:nil)
    let continueButton=NSButton(title:"Continuar campanha",target:nil,action:nil)
    let stopTitle=NSTextField(labelWithString:"Jogo interrompido · copie o relatório para informar esta parada")
    let stopGuidance=NSTextField(labelWithString:"A simulação parou. Você pode copiar, inspecionar ou reiniciar esta fase.")
    var campaignEnd:MacOSCampaignEnd?
    var shellHeld=Set<Int>()
    var shellMouseDown=false
    var shellRun:UInt64=0
    let defeatMovie=NativeDefeatMovie()
    var movieActive=false
    var movieStatus=""
    var moviePath=""
    /// The playing movie answers INTRODUCTION, which has no engine completion yet.
    var introductionMovie=false
    var credits:NativeFLC?,creditsRead=0.0
    private(set) var stopReport=""
    var saveNotice=""
    var savesChanged:(()->Void)?
    var campaignSelection:CampaignSelection
    // Diagnostic builds use disposable slots, never the player's saves.
    var saveDirectory:URL {
        let args=CommandLine.arguments
        if let i=args.firstIndex(of:"--save-directory"),args.count>i+1{return URL(fileURLWithPath:args[i+1],isDirectory:true)}
        if Self.diagnosticRun {
            return FileManager.default.temporaryDirectory.appendingPathComponent("risinglands-tests-\(ProcessInfo.processInfo.processIdentifier)",isDirectory:true)
        }
        return FileManager.default.urls(for:.applicationSupportDirectory,in:.userDomainMask)[0].appendingPathComponent("RisingLands-ptbr/Partidas",isDirectory:true)
    }
    /// Smokes, benchmarks and Test B never read or write the player's saves or preferences.
    static let diagnosticRun=CommandLine.arguments.contains(where:{["--smoke-test","--play-smoke","--play-stop-smoke","--native-benchmark","--native-horizon","--test-b-session","--player-smoke","--save-smoke","--campaign-smoke","--defeat-smoke","--menu-smoke","--pause-smoke","--ux-menu-trace"].contains($0)})
    static var initialSpeed:NativeGameSpeed { diagnosticRun ? .normal:NativeGameSpeed.load() }
    static var initialDifficulty:NativeDifficulty { diagnosticRun ? .normal:NativeDifficulty.load() }
    static var initialDisplay:NativeDisplayOptions { diagnosticRun ? NativeDisplayOptions():NativeDisplayOptions.load() }
    func saveURL(_ slot:String)->URL {saveDirectory.appendingPathComponent(slot).appendingPathExtension("rls")}
    func saveError(_ code:Int32)->NSError {
        var text=[CChar](repeating:0,count:512);macos_live_save_error(driver,&text,512)
        return NSError(domain:"Partida",code:Int(code),userInfo:[NSLocalizedDescriptionKey:String(cString:text)])
    }
    // The game queue owns world/file operations. Let scheduled clicks complete
    // before save; paused games report busy instead of losing their input.
    func saveSlot(_ slot:String,completion:@escaping(Result<Void,Error>)->Void) {
        metal.releaseKeys()
        func attempt(_ retries:Int) {
            work.async{[weak self] in
                guard let self=self else{return}
                do {
                    guard !self.validationMode,!self.recording else{throw NSError(domain:"Partida",code:16,userInfo:[NSLocalizedDescriptionKey:"Salve no modo Jogar, fora de uma gravação de entrada."])}
                    try FileManager.default.createDirectory(at:self.saveDirectory,withIntermediateDirectories:true)
                    let path=self.saveURL(slot).path
                    var job:OpaquePointer?
                    let code=macos_live_save_begin(self.driver,path,&job)
                    if code==16 && self.running && retries>0 {self.work.asyncAfter(deadline:.now()+0.08){attempt(retries-1)};return}
                    guard code==0,let job else{throw self.saveError(code)}
                    // Captured here; written off the game queue. Success only after write+fsync.
                    self.commitSave(job,path:path){failure in
                        if let failure{DispatchQueue.main.async{completion(.failure(NSError(domain:"Partida",code:15,userInfo:[NSLocalizedDescriptionKey:failure])))};return}
                        self.saveNotice="Partida salva";self.deliver()
                        DispatchQueue.main.async{self.savesChanged?();completion(.success(()))}
                    }
                }catch{DispatchQueue.main.async{completion(.failure(error))}}
            }
        }
        attempt(30)
    }
    /// Serial utility queue for save files: capture stays on the game worker,
    /// the companion/slot writes, SHA of the old slot and fsync run here.
    let saveIO=DispatchQueue(label:"risinglands.native.save",qos:.utility)
    var pendingSaves=[String:Int]()
    var saveTrace=[[String:Double]]()
    /// Worker. `done` runs on the worker after the files were written and synced.
    func commitSave(_ job:OpaquePointer,path:String,captured:Double=ProcessInfo.processInfo.systemUptime,done:@escaping(String?)->Void){
        pendingSaves[path,default:0]+=1
        saveIO.async{[weak self] in
            let started=ProcessInfo.processInfo.systemUptime
            let code=macos_live_save_commit(job)
            var text=[CChar](repeating:0,count:512);macos_live_save_job_error(job,&text,512);macos_live_save_job_free(job)
            let failure:String?=code==0 ? nil:String(cString:text)
            let written=ProcessInfo.processInfo.systemUptime
            self?.work.async{[weak self] in
                guard let self else{return}
                if let count=self.pendingSaves[path],count>1{self.pendingSaves[path]=count-1}else{self.pendingSaves[path]=nil}
                if NativeLiveWindow.menuTrace{self.saveTrace.append(["captured":captured,"writeStarted":started,"written":written])}
                done(failure)
            }
        }
    }
    /// Worker. A load (or quit) of a slot still being written waits for its file.
    func waitForSaves(_ path:String?=nil){
        if path.map({pendingSaves[$0] != nil}) ?? !pendingSaves.isEmpty {saveIO.sync{}}
    }
    /// Main thread at application exit: never leave a captured save unwritten.
    func flushSaves(){saveIO.sync{}}
    func loadSlot(_ slot:String,completion:@escaping(Result<Void,Error>)->Void) {
        metal.releaseKeys()
        work.async{[weak self] in
            guard let self=self else{return}
            do {
                guard !self.validationMode,!self.recording else{throw NSError(domain:"Partida",code:16,userInfo:[NSLocalizedDescriptionKey:"Carregue no modo Jogar, fora de uma gravação de entrada."])}
                let wasRunning=self.running
                self.waitForSaves(self.saveURL(slot).path)
                self.presenter?.stop()
                let code=macos_live_load(self.driver,self.saveURL(slot).path)
                guard code==0 else{
                    if wasRunning{self.presenter?.start(at:ProcessInfo.processInfo.systemUptime)}
                    throw self.saveError(code)
                }
                self.shellRun &+= 1;self.movieActive=false;self.introductionMovie=false
                DispatchQueue.main.async{self.defeatMovie.cancel()}
                self.runID &+= 1;self.running=false;self.stoppedReason=nil;self.presenter?.clear();self.audio?.reset()
                DispatchQueue.main.async{self.cdMusic?.beginStage()}
                self.campaignEnd=nil;self.refreshSelection()
                self.playStarted=0;self.playEnded=0;self.startFrames=self.status().frames;self.startLogic=0;self.startRender=0;self.startProof=0;self.startMetal=self.metalSeconds
                self.saveNotice="Partida carregada"
                if let pixels=macos_live_pixels(self.driver),let palette=macos_live_palette(self.driver){
                    DispatchQueue.main.sync{self.metal.releaseKeys();self.metal.isHidden=false;do{try self.metal.present(pixels,palette)}catch{self.stoppedReason="Metal: \(error.localizedDescription)"}}
                }
                let frame=self.status().frames
                DispatchQueue.main.async{self.displayStop(nil,frame:frame+1);self.updateTitle();self.window.makeKeyAndOrderFront(nil);completion(.success(()))}
                if self.stoppedReason==nil{self.perform("live-play",completion:nil)}else{self.deliver()}
            }catch{DispatchQueue.main.async{completion(.failure(error))}}
        }
    }
    let label=NSTextField(labelWithString:"MONDE01 · Rust ABI 1.6 · sem entrada aplicada")
    let button=NSButton(title:"Executar",target:nil,action:nil)
    deinit{presenter?.stop();if let driver=driver{macos_live_free(driver)}}
    init(root:URL,validationMode:Bool=false,selection:CampaignSelection=CampaignSelection(mission:"MONDE01.ETA",difficulty:1)) throws {
        campaignSelection=selection
        self.validationMode=validationMode;self.root=root;metal=try NativeMetalGameView(nativeFrame:NSRect(x:0,y:105,width:800,height:600))
        window=NSWindow(contentRect:risingPlayerEdition ? NSRect(x:0,y:0,width:960,height:720):NSRect(x:0,y:0,width:800,height:705),styleMask:[.titled,.closable,.miniaturizable,.resizable],backing:.buffered,defer:false)
        super.init()
        window.collectionBehavior.insert(.fullScreenPrimary)
        loadHotkeys()
        window.acceptsMouseMovedEvents=true;window.title=Self.title(selection);window.minSize=NSSize(width:640,height:550);window.contentView=NativeGameLayout(metal);applyDisplay(Self.initialDisplay);window.delegate=self
        button.target=self;button.action=#selector(toggle);button.frame=NSRect(x:15,y:36,width:100,height:27)
        stepButton.target=self;stepButton.action=#selector(step);stepButton.frame=NSRect(x:125,y:36,width:140,height:27)
        resetButton.target=self;resetButton.action=#selector(reset);resetButton.frame=NSRect(x:275,y:36,width:100,height:27)
        label.frame=NSRect(x:15,y:8,width:770,height:22);label.autoresizingMask = [.width]
        // Research chrome exists only in the development build.
        let chrome=risingPlayerEdition ? nil:window.contentView
        chrome?.addSubview(button);chrome?.addSubview(stepButton);chrome?.addSubview(resetButton);chrome?.addSubview(label)
        recordButton.target=self;recordButton.action=#selector(toggleRecording);recordButton.frame=NSRect(x:385,y:36,width:170,height:27)
        recordButton.toolTip="Gravação de cliques, arrastos e teclas"
        let exportButton=NSButton(title:"Exportar entrada",target:self,action:#selector(exportRecording));exportButton.frame=NSRect(x:565,y:36,width:160,height:27)
        chrome?.addSubview(recordButton);chrome?.addSubview(exportButton)
        stopTitle.font = .boldSystemFont(ofSize:13);stopTitle.textColor = .systemRed
        stopTitle.frame=NSRect(x:0,y:132,width:550,height:22);stopTitle.autoresizingMask = [.width]
        copyStopButton.target=self;copyStopButton.action=#selector(copyStopReport)
        copyStopButton.frame=NSRect(x:570,y:130,width:170,height:27);copyStopButton.autoresizingMask = [.minXMargin]
        let scroll=NSScrollView(frame:NSRect(x:0,y:28,width:770,height:98))
        scroll.autoresizingMask = [.width];scroll.hasVerticalScroller=true;scroll.borderType = .bezelBorder
        stopText.isEditable=false;stopText.isSelectable=true;stopText.font = .monospacedSystemFont(ofSize:12,weight:.regular)
        stopText.isVerticallyResizable=true;stopText.isHorizontallyResizable=false
        stopText.autoresizingMask = [.width];stopText.textContainer?.widthTracksTextView=true
        stopText.frame=scroll.contentView.bounds
        stopText.textContainer?.containerSize=NSSize(width:scroll.contentSize.width,height:CGFloat.greatestFiniteMagnitude)
        scroll.documentView=stopText
        stopGuidance.frame=NSRect(x:0,y:2,width:770,height:22);stopGuidance.autoresizingMask = [.width]
        continueButton.target=self;continueButton.action=#selector(continueCampaign);continueButton.frame=NSRect(x:0,y:0,width:205,height:27);continueButton.isHidden=true
        stopPanel.addSubview(stopTitle);stopPanel.addSubview(copyStopButton);stopPanel.addSubview(scroll);stopPanel.addSubview(stopGuidance);stopPanel.addSubview(continueButton)
        stopPanel.isHidden=true;window.contentView?.addSubview(stopPanel)
        (window.contentView as? NativeGameLayout)?.stopPanel=stopPanel
        metal.inputCancelled={ [weak self] in self?.work.async{[weak self] in self?.shellHeld.removeAll();self?.shellMouseDown=false} }
        metal.inputEvent={ [weak self] event in
            guard let self=self else{return}
            if event["kind"] as? String == "page"{
                self.work.async{[weak self] in guard let self=self,self.stoppedReason==nil,macos_live_shell_mode(self.driver) != 0 else{return};self.shellEvent(event)}
                return
            }
            let shouldRecord=self.recording
            // Worker stamps the next game frame; presentation rate never enters mapping.
            self.work.async{[weak self] in
                guard let self=self,self.status().stopped==0,self.stoppedReason==nil else{return}
                if macos_live_shell_mode(self.driver) != 0{self.shellEvent(event);return}
                let frame=max(129,Int(self.status().frames)+1)
                if !self.validationMode {
                    let kind=event["kind"] as? String
                    let code=macos_live_event(self.driver,kind=="mouse" ? 1:2,UInt32(event["msg"] as? Int ?? 0),Int32(event["x"] as? Int ?? 0),Int32(event["y"] as? Int ?? 0),UInt32(event["wParam"] as? Int ?? 0),UInt32(event["vkey"] as? Int ?? 0),(event["down"] as? Bool ?? false) ? 1:0)
                    if code != 0{self.running=false;self.stoppedReason="Entrada recusada: \(self.message(self.status()))";self.deliver()}
                }
                DispatchQueue.main.async{if shouldRecord{self.recordedEvent?(["frame":frame,"event":event])}}
            }
        }
        metal.modernCommand={ [weak self] command in MainActor.assumeIsolated{self?.startModern(command)} }
        modeControl.target=self;modeControl.action=#selector(changeMode);modeControl.selectedSegment=validationMode ? 1:0
        modeControl.frame=NSRect(x:15,y:66,width:190,height:25)
        rateControl.addItems(withTitles:NativeGameSpeed.allCases.map{"Velocidade \($0.menuTitle)"});rateControl.selectItem(at:speed.rawValue)
        rateControl.toolTip="Velocidade do jogo: 1x = 30 quadros/s; a lógica de cada quadro não muda"
        rateControl.target=self;rateControl.action=#selector(changeRate);rateControl.frame=NSRect(x:220,y:66,width:225,height:25)
        rateControl.isEnabled = !validationMode;chrome?.addSubview(modeControl);chrome?.addSubview(rateControl)
        musicLabel.frame=NSRect(x:455,y:68,width:330,height:22);musicLabel.font = .systemFont(ofSize:10)
        musicLabel.lineBreakMode = .byTruncatingTail;musicLabel.autoresizingMask = [.minXMargin]
        chrome?.addSubview(musicLabel)
        if risingPlayerEdition {
            (window.contentView as? NativeGameLayout)?.player=true
            window.minSize=NSSize(width:640,height:480)
            stopGuidance.stringValue="O jogo parou. Copie o relatório para informar esta parada; você pode carregar uma partida salva pelo menu Jogo."
        }
        MainActor.assumeIsolated {
            let path=ProcessInfo.processInfo.environment["RISING_CD_MUSIC_BIN"]
            configureCDMusic(disc:path.map{URL(fileURLWithPath:$0)})
        }
        presenter=try metal.makePlayPresenter()
        // Host controls never take keyboard focus: Space/arrows/Enter stay game keys.
        let controls:[NSControl]=[button,stepButton,resetButton,recordButton,exportButton,copyStopButton,continueButton,modeControl,rateControl];for control in controls{control.refusesFirstResponder=true}
        window.center();window.makeKeyAndOrderFront(nil);window.makeFirstResponder(metal)
    }
    func benchmarkClick(_ x:Double,_ y:Double,right:Bool=false,clickCount:Int=1,flags:NSEvent.ModifierFlags=[]){
        window.makeKeyAndOrderFront(nil);window.makeFirstResponder(metal)
        guard let local=metal.viewport?.windowPoint(forOriginalPixel:.init(x:Int(x),y:Int(y))) else{return}
        let point=metal.convert(local,to:nil)
        for type in (right ? [NSEvent.EventType.rightMouseDown,.rightMouseUp]:[NSEvent.EventType.leftMouseDown,.leftMouseUp]){if let event=NSEvent.mouseEvent(with:type,location:point,modifierFlags:flags,timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:window.windowNumber,context:nil,eventNumber:1,clickCount:clickCount,pressure:1){window.sendEvent(event)}}
    }
    func benchmarkPointer(_ x:Double,_ y:Double){
        guard let local=metal.viewport?.windowPoint(forOriginalPixel:.init(x:Int(x),y:Int(y))) else{return}
        let point=metal.convert(local,to:nil)
        if let event=NSEvent.mouseEvent(with:.mouseMoved,location:point,modifierFlags:[],timestamp:ProcessInfo.processInfo.systemUptime,windowNumber:window.windowNumber,context:nil,eventNumber:3,clickCount:0,pressure:0){window.sendEvent(event)}
    }
    @objc func changeMode(){metal.releaseKeys();let selected=modeControl.selectedSegment==1;rateControl.isEnabled = !selected;work.async{[weak self] in guard let self=self else{return};self.running=false;self.presenter?.stop();self.validationMode=selected;self.perform("live-reset",completion:nil)}}
    @objc func changeRate(){if let selected=NativeGameSpeed(rawValue:rateControl.indexOfSelectedItem){setSpeed(selected)}}
    /// Main thread. Speed is a host pacing choice; the worker re-anchors its deadline.
    func setSpeed(_ selected:NativeGameSpeed){
        speed=selected;if !Self.diagnosticRun{selected.store()};rateControl.selectItem(at:selected.rawValue)
        work.async{[weak self] in self?.gameFPS=selected.frameRate;self?.deliver()}
    }
    var isRunning:Bool{running}
    @objc func toggleRecording(){inputBarrier{[weak self] frame in guard let self=self else{return};if frame<129 && !self.recording{self.label.stringValue="Gravação disponível após o quadro 128"}else{self.recordingAction?()}}}
    @objc func exportRecording(){exportAction?()}
    @objc func copyStopReport(){
        guard !stopReport.isEmpty else{return}
        NSPasteboard.general.clearContents();NSPasteboard.general.setString(stopReport,forType:.string)
    }
    // Main-thread presentation only. The driver retains the failed world and its
    // original diagnostic until an explicit reset; no retry or engine substitute.
    func displayStop(_ detail:String?,frame:UInt64,result:MacOSCampaignEnd?=nil,originalMode:UInt32=0){
        if let detail=detail {
            if stopReport.isEmpty {
                let revision=Bundle.main.object(forInfoDictionaryKey:"RisingLandsSourceCommit") as? String ?? "desconhecida"
                stopReport="\(campaignSelection.mission) · quadro \(frame) · \(validationMode ? "Validar":"Jogar")\n\(detail)\nVersão: \(revision)"
                stopText.string=stopReport;stopText.scrollRangeToVisible(NSRange(location:0,length:0))
            }
        }else{stopReport="";stopText.string=""}
        let hidden=stopReport.isEmpty && (result == nil || originalMode != 0)
        let visibilityChanged=stopPanel.isHidden != hidden
        stopPanel.isHidden=hidden
        button.isEnabled=hidden && originalMode==0;stepButton.isEnabled=hidden && originalMode==0
        continueButton.isHidden = true
        stopTitle.stringValue="Jogo interrompido · copie o relatório para informar esta parada";stopTitle.textColor = .systemRed
        stopGuidance.stringValue=risingPlayerEdition ? "O jogo parou. Copie o relatório para informar esta parada; você pode carregar uma partida salva pelo menu Jogo.":"A simulação parou. Você pode inspecionar ou reiniciar esta fase."
        if visibilityChanged{window.contentView?.needsLayout=true}
    }
    func setRecording(_ active:Bool){
        let starting=active && !recording
        if starting{metal.synchronizeModifiers(NSEvent.modifierFlags)}
        if active{periodicSnapshots=true}
        recording=active;modeControl.isEnabled = !active;resetButton.isEnabled = !active;recordButton.title=active ? "● Gravando entrada":"Nova gravação";recordButton.contentTintColor=active ? .systemRed:nil
        if starting{window.makeKeyAndOrderFront(nil);window.makeFirstResponder(metal)}
        if starting{let held=metal.heldKeys.sorted();inputBarrier{[weak self] frame in for key in held{self?.recordedEvent?(["frame":frame,"event":["kind":"key","vkey":key,"down":true]])}}}
    }
    func inputBarrier(_ completion:@escaping(Int)->Void){work.async{[weak self] in guard let self=self else{return};let frame=Int(self.status().frames)+1;DispatchQueue.main.async{completion(frame)}}}
    func windowDidBecomeKey(_ notification:Notification){
        if window.firstResponder === window || window.firstResponder == nil {window.makeFirstResponder(metal)}
        metal.activateKeys()
        work.async{[weak self] in
            guard let self=self,!self.movieActive,self.stoppedReason==nil,macos_live_shell_mode(self.driver) != 0 else{return}
            self.startShell();self.showShellPicture()
        }
    }
    func windowDidResignKey(_ notification:Notification){metal.cancelInput();metal.leaveScrollEdge();metal.hotkeys.setActive(false)}
    func windowDidChangeBackingProperties(_ notification:Notification){window.contentView?.needsLayout=true}
    func windowWillEnterFullScreen(_ notification:Notification){(window.contentView as? NativeGameLayout)?.immersive=true}
    func windowWillExitFullScreen(_ notification:Notification){(window.contentView as? NativeGameLayout)?.immersive=false}
    func windowDidExitFullScreen(_ notification:Notification){window.makeFirstResponder(metal);metal.activateKeys()}
    func windowWillClose(_ notification:Notification){
        metal.releaseKeys();hotkeyPreferences?.close()
        work.async{[weak self] in guard let self=self else{return};self.running=false;self.runID &+= 1;self.shellRun &+= 1;self.presenter?.stop();DispatchQueue.main.async{self.cdMusic?.leaveStage()}}
    }
    var hotkeyPreferences:NativeHotkeyPreferencesController?
    var hotkeyURL:URL { saveDirectory.appendingPathComponent("atalhos.json") }
    func loadHotkeys() { MainActor.assumeIsolated {
        guard FileManager.default.fileExists(atPath:hotkeyURL.path) else{return}
        do { var bindings=NativeHotkeyBindings();try bindings.load(from:hotkeyURL);try metal.hotkeys.prepareInstallation(bindings)() }
        catch { label.stringValue=error.localizedDescription }
    }
    }
    @objc func showHotkeys(){ MainActor.assumeIsolated {
        do { try FileManager.default.createDirectory(at:hotkeyURL.deletingLastPathComponent(),withIntermediateDirectories:true) }
        catch { let alert=NSAlert();alert.messageText="Não foi possível abrir os atalhos";alert.informativeText=error.localizedDescription;alert.beginSheetModal(for:self.window);return }
        if hotkeyPreferences == nil {
            hotkeyPreferences=NativeHotkeyPreferencesController(bindings:metal.hotkeys.bindings,configurationURL:hotkeyURL,keyLabel:NativeHotkeyLabels.label) { [weak self] candidate in
                guard let self else { throw NativeHotkeyBindings.BindingError.writeFailed }
                return try self.metal.hotkeys.prepareInstallation(candidate)
            }
        }
        hotkeyPreferences?.showWindow(nil)
    }
    }
    @objc func toggleFullScreen(){window.toggleFullScreen(nil)}
    var scaleMode:NativeViewportPolicy.Mode{(window.contentView as? NativeGameLayout)?.scaleMode ?? .continuousFit}
    var smoothFilter:Bool{metal.smoothFilter}
    var difficulty:NativeDifficulty{NativeDifficulty(rawValue:campaignSelection.difficulty) ?? .normal}
    func applyDisplay(_ options:NativeDisplayOptions){
        (window.contentView as? NativeGameLayout)?.scaleMode=options.integerScale ? .integerFit:.continuousFit
        metal.smoothFilter=options.smoothFilter
    }
    func storeDisplay(){if !Self.diagnosticRun{NativeDisplayOptions(integerScale:scaleMode == .integerFit,smoothFilter:smoothFilter).store()}}
    func setScaleMode(_ mode:NativeViewportPolicy.Mode){(window.contentView as? NativeGameLayout)?.scaleMode=mode;storeDisplay()}
    /// Main thread. A paused game or an original shell screen has no presenter
    /// running, so the frame on screen is repainted once with the new filter.
    func setSmoothFilter(_ on:Bool){
        metal.smoothFilter=on;storeDisplay()
        work.async{[weak self] in
            guard let self=self,self.presenter?.isPresenting != true,!self.movieActive,self.stoppedReason==nil,let pixels=macos_live_pixels(self.driver),let palette=macos_live_palette(self.driver) else{return}
            DispatchQueue.main.sync{if !self.metal.isHidden{try? self.metal.present(pixels,palette)}}
        }
    }
    /// Main thread. Applies from the next stage start; the driver keeps it across the session.
    func setDifficulty(_ selected:NativeDifficulty){
        if !Self.diagnosticRun{selected.store()}
        campaignSelection=CampaignSelection(mission:campaignSelection.mission,difficulty:selected.rawValue)
        work.async{[weak self] in if let self=self{_ = macos_live_set_difficulty(self.driver,selected.rawValue)}}
    }

    func create() throws {
        guard macos_live_abi_version()==1,macos_live_status_size()==MemoryLayout<MacOSLiveStatus>.size else { throw NSError(domain:"NativeDriverVersionMismatch",code:1) }
        let directory=root.appendingPathComponent("native-inputs")
        let historical=validationMode || CommandLine.arguments.contains("--debug-s0")
        if historical && !validationMode && (campaignSelection.mission != "MONDE01.ETA" || campaignSelection.difficulty != 1){
            throw NSError(domain:"Depuração S0",code:1,userInfo:[NSLocalizedDescriptionKey:"A opção --debug-s0 exige MONDE01 na dificuldade Normal."])
        }
        let manifest=try JSONSerialization.jsonObject(with:Data(contentsOf:directory.appendingPathComponent("manifest.json"))) as! [String:Any]
        for (name,expected) in manifest["files"] as! [String:String] {
            if !historical && ["s0.bin","entries.bin","spans.bin","states.bin","surfaces.bin"].contains(name){continue}
            let data=try Data(contentsOf:directory.appendingPathComponent(name))
            guard SHA256.hash(data:data).map({String(format:"%02x",$0)}).joined()==expected else { throw NSError(domain:"NativeInputHashMismatch",code:1,userInfo:[NSLocalizedDescriptionKey:name]) }
        }
        var error=[CChar](repeating:0,count:512)
        let library=root.appendingPathComponent("librising_engine.dylib").path
        let candidate:OpaquePointer?
        if historical {
            candidate=macos_live_create(library,directory.path,&error,512)
        } else {
            guard let logical=Bundle.main.object(forInfoDictionaryKey:"RisingLandsEventsDirectory") as? String else {
                throw NSError(domain:"EVENTSHostContract",code:1,userInfo:[NSLocalizedDescriptionKey:"Mapeamento de EVENTS ausente no aplicativo. Recompile esta versão."])
            }
            let bytes=Array(logical.utf8)
            // The original main menu (MenuInit 2) is the normal start; a stage
            // name starts that stage directly (chooser, saves, diagnostics).
            candidate=bytes.withUnsafeBufferPointer {
                campaignSelection.isMenu
                    ? macos_live_create_menu_host_v1(library,directory.path,campaignSelection.difficulty,$0.baseAddress,UInt32($0.count),&error,512)
                    : macos_live_create_mission_host_v1(library,directory.path,campaignSelection.mission,campaignSelection.difficulty,$0.baseAddress,UInt32($0.count),&error,512)
            }
        }
        guard let candidate=candidate else { throw NSError(domain:"NativeInitialization",code:1,userInfo:[NSLocalizedDescriptionKey:String(cString:error)]) }
        guard macos_live_validation(candidate,validationMode ? 1:0)==0 else{
            var failed=MacOSLiveStatus();macos_live_status(candidate,&failed)
            let detail=message(failed);macos_live_free(candidate)
            throw NSError(domain:"NativeValidationModeRejected",code:1,userInfo:[NSLocalizedDescriptionKey:detail])
        }
        // Sound is a play-mode consumer only; validation never enables the transport.
        if !validationMode && audio==nil{audio=NativeAudioPlayback(directory:root.appendingPathComponent("audio"))}
        audio?.reset()
        if !validationMode,audio?.enabled==true,macos_live_sound_capture(candidate,1) != 0{macos_live_free(candidate);throw NSError(domain:"NativeSoundTransportRejected",code:1)}
        let previous=driver;driver=candidate;saveNotice="";if let previous=previous{macos_live_free(previous)}
        DispatchQueue.main.async{if self.validationMode{self.cdMusic?.leaveStage()}else{self.cdMusic?.beginStage()}}
    }
    func status()->MacOSLiveStatus { var value=MacOSLiveStatus();macos_live_status(driver,&value);return value }
    func refreshSelection(){
        var name=[CChar](repeating:0,count:16),difficulty:UInt32=0
        if macos_live_selection(driver,&name,16,&difficulty)==0{campaignSelection=CampaignSelection(mission:String(cString:name),difficulty:difficulty)}
    }
    static func title(_ selection:CampaignSelection)->String{risingPlayerEdition ? "Rising Lands":selection.isMenu ? "Rising Lands · menu original · Rust e Metal":"\(selection.mission.replacingOccurrences(of:".ETA",with:"")) · \(selection.difficultyTitle) · Rust e Metal"}
    func updateTitle(){window.title=Self.title(campaignSelection)}
    func message(_ value:MacOSLiveStatus)->String { var s=value;return withUnsafePointer(to:&s.message){$0.withMemoryRebound(to:CChar.self,capacity:512){String(cString:$0)}} }
    func snapshot()->[String:Any] {
        let s=status(),index=s.phase_index==UInt32.max ? -1:Int(s.phase_index)
        let name=index<0 ? "state_s0":index==0 ? "load_tail":"frame\(s.frames)"
        let elapsed=max(0.000001,(running ? ProcessInfo.processInfo.systemUptime:playEnded)-playStarted)
        let frames=s.frames>=startFrames ? s.frames-startFrames:0
        var timing:[String:Any]=["frames":frames,"seconds":elapsed,"fps":playStarted>0 ? Double(frames)/elapsed:0,"rustSeconds":Double(s.logic_ns-startLogic)/1e9,"renderSeconds":Double(s.render_ns-startRender)/1e9,"proofSeconds":Double(s.proof_ns-startProof)/1e9,"metalSeconds":metalSeconds-startMetal]
        timing["humanEvents"]=macos_live_human_events(driver);timing["declaredFrames"]=macos_live_declared_frames(driver)
        timing["gameFPS"]=validationMode ? 0:gameFPS;timing["gameSpeed"]=NativeGameSpeed(frameRate:gameFPS)?.label ?? NSNull();timing["executionMode"]=validationMode ? "test":"play"
        let end=running ? ProcessInfo.processInfo.systemUptime:playEnded
        if !validationMode,let presenter=presenter{timing.merge(presenter.metrics(start:playStarted,end:end,detailed:!running)){_,new in new}}
        if !validationMode,let audio=audio{timing.merge(audio.metrics()){_,new in new}}
        else{timing.merge(metal.testMetrics(start:playStarted,end:end,detailed:!running)){_,new in new}}
        let stop:Any = s.stopped != 0 || stoppedReason != nil ? ["type":"NativeStop","phase":"frame\(s.frames+1)","detail":stoppedReason ?? message(s)]:NSNull()
        let equal:Any = s.stopped==0 && stoppedReason==nil && s.frames>0 && s.frames<=9200 && s.surfaces_equal>=s.frames ? true:NSNull()
        let sha=validationMode ? withUnsafeBytes(of:s.state_sha){$0.map{String(format:"%02x",$0)}.joined()}:""
        var phases=validationMode ? ["load_tail","between_frames","frame1"]:[String]()
        if validationMode{for i in 2...128{phases += ["between_frames\(i)","frame\(i)"]}}
        if s.frames>128{phases.append(name)}
        return ["mode":"live-rust-no-input","mission":campaignSelection.mission,"difficulty":campaignSelection.difficulty,"campaign_outcome":campaignEnd?.outcome ?? 0,"original_shell_mode":macos_live_shell_mode(driver),"movie_status":movieStatus,"movie_path":moviePath,"scenario":validationMode ? "MONDE01 · validação sem entrada":"\(campaignSelection.mission) · \(campaignSelection.difficultyTitle) · ordens ao vivo","phase":name,"phase_index":index,"phases":phases,
                "summary_sha256":index>=0 && index<=256 ? sha:"", "summary_verified":validationMode && s.stopped==0 && index>=0 && index<=256,
                "checkpoint_verified":false,"verified_phases":s.phases_equal,"verified_checkpoints":s.checkpoints_equal,"verified_surfaces":s.surfaces_equal,
                "observed":["counter":["address":"0x508f7c","bytes":4,"value":s.counter]],"blocks":0,"bytes":0,"memory":[],"input_policy":validationMode ? "record-only":"live-orders","live_stop":stop,
                "benchmark_order_tags":benchmarkOrderTags,"execution_mode":validationMode ? "test":"play","native_metal":true,"native_engine_binding":macos_live_engine_binding()==1 ? "static-abi16":"dynamic-abi16","native_running":running,"native_metrics":timing,
                "render":["status":s.disposition==2 ? "partial":s.frames==0 ? "undrawn":"full","hud":true,"version":[1,9],"backend":"Rust ABI 1.6 + Metal","equals_native_phase_end":equal,"native_phase_end_sha256":NSNull()]]
    }
    func deliver(_ completion:(([String:Any])->Void)?=nil,periodic:Bool=false){
        // Periodic notifications build a full snapshot only for a consumer (the
        // shown inspector, a recording, inspection/test modes); direct play and
        // the player edition refresh only the stop state and caption.
        let value:[String:Any]=periodic && !periodicSnapshots && completion==nil ? [:]:snapshot()
        let s=status(),isRunning=running,result=campaignEnd,originalMode=macos_live_shell_mode(driver)
        let detail:String? = stoppedReason ?? (s.stopped != 0 ? message(s):nil)
        let caption="\(validationMode ? "Validar":"Jogar · \(gameFPS==0 ? "sem limite":NativeGameSpeed(frameRate:gameFPS).map{"\($0.label) (\(Int($0.frameRate)) q/s)"} ?? String(format:"%.0f q/s",gameFPS))") · Quadro \(s.frames) · \(detail != nil ? "interrompido":isRunning ? "executando":"pausado")" + (detail != nil ? " · relatório abaixo":validationMode ? " · sem entrada aplicada":" · ordens após q128 · som: \(audio?.availability ?? "—") · texto: PT-BR")
        DispatchQueue.main.async { [weak self] in
            guard let self=self else{return};var value=value;value["native_window_visible"]=self.window.isVisible
            value["cd_music"]=self.cdMusic?.metrics
            self.button.title=isRunning ? "Pausar":"Executar"
            self.metal.hidesSystemCursor=isRunning && !self.validationMode
            self.displayStop(detail,frame:s.frames+1,result:result,originalMode:originalMode)
            let fps=(value["native_metrics"] as? [String:Any])?["displayFPS"] as? Double ?? 0
            self.label.stringValue=caption+(isRunning && !value.isEmpty ? String(format:" · tela %.1f fps",fps):"")+(self.saveNotice.isEmpty ? "":" · \(self.saveNotice)")
            if let completion=completion{completion(value)}else if !value.isEmpty{self.update?(value)}
        }
    }
    func advance(){
        let stageStart = !validationMode && status().phase_index==UInt32.max
        let code=macos_live_advance(driver,nil,nil),s=status()
        if stageStart && code==0 {
            do {
                try FileManager.default.createDirectory(at:saveDirectory,withIntermediateDirectories:true)
                let saved=macos_live_autosave(driver,saveURL("inicio-fase").path)
                guard saved==0 else{throw saveError(saved)}
                DispatchQueue.main.async{self.savesChanged?()}
            }catch{
                running=false
                DispatchQueue.main.async{let alert=NSAlert();alert.messageText="Falha ao salvar automaticamente";alert.informativeText=error.localizedDescription;alert.beginSheetModal(for:self.window)}
            }
        }
        if !validationMode{audio?.pump(driver);if code==0{pumpCDMusic()}}
        if !validationMode && code==0 {
            var screen:UInt32=0
            if macos_live_campaign_screen(driver,&screen)==0 && screen==UInt32(RL_SHELL_PAUSE) {
                // Original in-game pause (MenuInit 16 ran in this frame): the prepared session takes input.
                running=false;playEnded=ProcessInfo.processInfo.systemUptime
                let shell=macos_live_shell_begin(driver)
                if shell==0{startShell()}else{stoppedReason=saveError(shell).localizedDescription}
            }
            else if macos_live_campaign_screen(driver,&screen)==0 && screen==19 {
                running=false;playEnded=ProcessInfo.processInfo.systemUptime
                var result=MacOSCampaignEnd()
                let query=macos_live_campaign_end(driver,&result)
                if query==0 {
                    campaignEnd=result
                    let shell=macos_live_shell_begin(driver)
                    if shell==0{startShell()}else{stoppedReason=saveError(shell).localizedDescription}
                }
                else {stoppedReason=saveError(query).localizedDescription}
                DispatchQueue.main.async{self.cdMusic?.result()}
            }
        }
        if benchmarkInput && code==0 && s.frames>128 {
            let count=macos_live_order_tags(driver,&orderBuffer,32)
            if count==UInt32.max{stoppedReason="Diagnóstico de entrada: \(message(status()))";running=false}
            else if count>0{benchmarkOrderTags.append(contentsOf:orderBuffer.prefix(Int(count)))}
        }
        if code != 0 {
            running=false;playEnded=ProcessInfo.processInfo.systemUptime
            let detail=message(s)
            if !validationMode && detail.contains("MenuInit mission end") && (detail.contains("0x0042c931") || detail.contains("0x42c931")) {
                var result=MacOSCampaignEnd()
                let query=macos_live_campaign_end(driver,&result)
                if query==0 && result.outcome != 0{campaignEnd=result}
                else if query != 0{stoppedReason=detail+"\n"+saveError(query).localizedDescription}
            }
            let result=campaignEnd != nil
            DispatchQueue.main.async{if result{self.cdMusic?.result()}else{self.cdMusic?.leaveStage()}}
        }
        if macos_live_shell_mode(driver) != 0{showShellPicture();return}
        if s.phase_index != UInt32.max,let pixels=macos_live_pixels(driver),let palette=macos_live_palette(driver) {
            if !validationMode {
                var state=MacOSEventFlcState()
                if macos_live_event_flc_state(driver,&state)==0,eventFLC.update(state:state,now:ProcessInfo.processInfo.systemUptime,name:{[weak self] index in
                    guard let self=self else{return nil};var name=[CChar](repeating:0,count:16)
                    return macos_live_event_flc_name(self.driver,index,&name,16)==0 ? String(cString:name):nil
                },enginePixels:pixels,finish:{[weak self] in macos_live_event_flc_finish(self?.driver)}),let flc=eventFLC.flc {
                    eventFLC.surface.withUnsafeBufferPointer{surface in flc.palette.withUnsafeBufferPointer{rgb in presenter?.publish(surface.baseAddress!,rgb.baseAddress!,frame:s.frames)}}
                } else {
                presenter?.publish(pixels,palette,frame:s.frames)
                }
                if let error=presenter?.currentFailure(){stoppedReason=error;running=false;playEnded=ProcessInfo.processInfo.systemUptime}
            } else {
                let started=ProcessInfo.processInfo.systemUptime
                DispatchQueue.main.sync { [weak self] in
                    guard let self=self else{return}
                    do {self.metal.isHidden=false;try self.metal.present(pixels,palette)}catch{self.stoppedReason="Metal: \(error.localizedDescription)";self.running=false;self.playEnded=ProcessInfo.processInfo.systemUptime}
                }
                metalSeconds+=ProcessInfo.processInfo.systemUptime-started
            }
        }
    }
    func pump(_ run:UInt64){
        guard running,run==runID else{return}
        let now=ProcessInfo.processInfo.systemUptime
        if !validationMode{let wait=pacer.delay(now:now);if wait>0{work.asyncAfter(deadline:.now()+wait){[weak self] in self?.pump(run)};return}}
        advance()
        stepModern()
        let finished=ProcessInfo.processInfo.systemUptime
        if finished-lastNotify>=0.1 || !running{lastNotify=finished;deliver(periodic:running)}
        if running{work.async{[weak self] in self?.pump(run)}}else{
            if macos_live_shell_mode(driver)==0 || movieActive{presenter?.stop()}
            if stoppedReason != nil && campaignEnd==nil{DispatchQueue.main.async{self.cdMusic?.leaveStage()}}
        }
    }
    // MARK: "Atalhos modernos" (NativeModernHotkeys.swift)
    /// Work queue only.
    var modernJob:NativeModernJob?
    var modernPlanner=NativeModernPlanner()
    /// Ignored in validation, menus/briefing/pause (shell), while stopped or not running.
    func modernAllowed()->Bool{
        running && !validationMode && stoppedReason==nil && status().stopped==0 && macos_live_shell_mode(driver)==0
    }
    @MainActor func startModern(_ command:NativeHotkeyInputRouter.ModernCommand){
        let mouse:Bool
        switch command{case .centerGroup,.lastAlert:mouse=false;default:mouse=true}
        if mouse{metal.syntheticUntil=ProcessInfo.processInfo.systemUptime+3}
        work.async{[weak self] in
            guard let self=self else{return}
            var job:NativeModernJob?
            if self.modernJob==nil,self.modernAllowed(){
                switch command{
                case .centerGroup(let key):job=NativeModernJob(steps:NativeModernHotkeys.centerGroup(key))
                case .lastAlert:job=NativeModernJob(steps:NativeModernHotkeys.lastAlert)
                default:
                    if let snapshot=self.modernSnapshot(),let target=self.modernPlanner.choose(command,in:snapshot){job=NativeModernJob(target:target)}
                }
            }
            guard let job=job else{if mouse{DispatchQueue.main.async{self.metal.syntheticUntil=0}};return}
            self.modernJob=job;self.stepModern()
        }
    }
    /// After every advance: deliver the next stage of the running command.
    func stepModern(){
        guard let job=modernJob else{return}
        let allowed=modernAllowed()
        if allowed{job.pump(modernHost())}
        // Menus, validation, a stop or a pause end the command; nothing more is delivered.
        guard !allowed || job.state != .running else{return}
        modernJob=nil
        if job.usesMouse{DispatchQueue.main.async{self.metal.syntheticUntil=0}}
    }
    func modernHost()->NativeModernJob.Host{
        NativeModernJob.Host(deliver:{[unowned self] in self.deliverSynthetic($0)},
                             pendingInput:{[unowned self] in macos_live_pending_input(self.driver)},
                             frames:{[unowned self] in self.status().frames},
                             snapshot:{[unowned self] in self.modernSnapshot()},
                             pointer:{[unowned self] in self.metal.pointer.get()})
    }
    /// One read-only projection per call, between frames, on the work queue.
    func modernSnapshot()->NativeModernSnapshot?{
        NativeModernSnapshot.read(concatenated:{addresses,sizes in
            var bytes=[UInt8](repeating:0,count:sizes.reduce(0){$0+Int($1)})
            guard !bytes.isEmpty,macos_live_peek(self.driver,addresses,sizes,UInt32(addresses.count),&bytes)==0 else{return nil}
            return bytes
        })
    }
    /// The player's own input path (macos_live_event, then recording), for synthetic input.
    func deliverSynthetic(_ event:[String:Any])->Bool{
        guard modernAllowed() else{return false}
        let frame=max(129,Int(status().frames)+1),kind=event["kind"] as? String
        let code=macos_live_event(driver,kind=="mouse" ? 1:2,UInt32(event["msg"] as? Int ?? 0),Int32(event["x"] as? Int ?? 0),Int32(event["y"] as? Int ?? 0),UInt32(event["wParam"] as? Int ?? 0),UInt32(event["vkey"] as? Int ?? 0),(event["down"] as? Bool ?? false) ? 1:0)
        if code != 0{running=false;stoppedReason="Entrada recusada: \(message(status()))";deliver();return false}
        DispatchQueue.main.async{if self.recording{self.recordedEvent?(["frame":frame,"event":event])}}
        return true
    }
    @MainActor func setModernHotkeys(_ enabled:Bool){
        NativeModernHotkeys.store(enabled);metal.releaseKeys();metal.hotkeys.modernEnabled=enabled
    }
    @objc func toggle(){perform("live-play",completion:nil)}
    @objc func step(){perform("live-step",completion:nil)}
    @objc func reset(){metal.releaseKeys();perform("live-reset",completion:nil)}
    @objc func continueCampaign(){
        work.async{[weak self] in self?.sendShell(keys:1)}
    }
    // The worker owns the prepared session and host menu counters. Gameplay is
    // suspended; only real shell input/draw APIs can change the research choice.
    func startShell(){
        presenter?.stop();shellHeld.removeAll();shellMouseDown=false;hostRequest=false;shellRun &+= 1;shellPendingMove=nil;shellZones.removeAll()
        let run=shellRun
        DispatchQueue.main.async{self.metal.releaseKeys();self.window.makeFirstResponder(self.metal)}
        work.asyncAfter(deadline:.now()+0.05){[weak self] in self?.shellTick(run)}
    }
    /// Worker. Menus publish into the play presenter, which shows the latest
    /// picture at the display refresh on its own queue (no main-thread wait, no
    /// waitUntilCompleted); the shell's own clock/animation stay at the 20 Hz tick.
    /// A movie keeps the one-shot synchronous present underneath it.
    var shellPicture:UInt64=0
    func showShellPicture(){
        guard let pixels=macos_live_pixels(driver),let palette=macos_live_palette(driver) else{presenter?.stop();return}
        guard !movieActive,!validationMode,let presenter else{
            presenter?.stop()
            DispatchQueue.main.sync{do{metal.isHidden=false;try metal.present(pixels,palette)}catch{stoppedReason="Metal: \(error.localizedDescription)"}}
            return
        }
        shellPicture &+= 1;presenter.publish(pixels,palette,frame:shellPicture)
        if !presenter.isPresenting{
            presenter.start(at:ProcessInfo.processInfo.systemUptime)
            DispatchQueue.main.async{self.metal.isHidden=false}
        }
        if let error=presenter.currentFailure(){stoppedReason=error;presenter.stop()}
    }
    /// Pointer moves in menus are coalesced: only the latest is applied, at most
    /// once per display refresh (or with the next 20 Hz tick). Press, release and
    /// keys are never coalesced.
    var shellPendingMove:(x:UInt32,y:UInt32)?
    var shellMoveScheduled=false
    var shellLastMove=0.0
    var shellMovesReceived=0,shellMovesApplied=0
    var shellZones=[UInt32:[Int32]]()
    func scheduleShellMove(){
        guard !shellMoveScheduled else{return}
        shellMoveScheduled=true
        let run=shellRun,delay=max(0,shellLastMove+1.0/60-ProcessInfo.processInfo.systemUptime)
        work.asyncAfter(deadline:.now()+delay){[weak self] in
            guard let self else{return}
            self.shellMoveScheduled=false
            guard run==self.shellRun else{self.shellPendingMove=nil;return}
            self.flushShellMove()
        }
    }
    func flushShellMove(){
        guard let move=shellPendingMove else{return}
        shellPendingMove=nil;shellLastMove=ProcessInfo.processInfo.systemUptime;shellMovesApplied+=1
        var input=RlShellInput();guard macos_live_shell_state(driver,&input)==0 else{return}
        input.mouse_x=move.x;input.mouse_y=move.y;input.mouse_af=0;input.mouse_b0=0
        input.mouse_ae=shellMouseDown ? 1:0
        // A held button must not turn a second page at an already used clock.
        if shellMouseDown{input.clock_7c=macos_live_shell_click_clock(macos_live_shell_mode(driver),input.clock_7c,0)}
        sendShell(input:input)
    }
    /// Worker. Up/Down, PageUp/PageDown and the wheel on the objective/briefing
    /// pages: a click on the original arrow zone 6/7 (rectangle from the session's
    /// MouseZone table) at an aligned clock, then the pointer returns. Same engine
    /// path as a real click; no new game logic.
    func pageObjectives(next:Bool){
        let mode=macos_live_shell_mode(driver)
        guard mode==UInt32(RL_SHELL_BRIEFING) || mode==UInt32(RL_SHELL_OBJECTIVES),!shellMouseDown else{return}
        let zone:UInt32=next ? 7:6
        var rect=shellZones[zone] ?? [Int32](repeating:0,count:4)
        if shellZones[zone]==nil{
            let code=macos_live_shell_zone(driver,zone,&rect)
            guard code==0 else{stoppedReason=saveError(code).localizedDescription;showShellPicture();deliver();return}
            shellZones[zone]=rect
        }
        guard rect[2]>rect[0],rect[3]>rect[1],rect[0]>=0,rect[1]>=0 else{return}
        var input=RlShellInput();guard macos_live_shell_state(driver,&input)==0 else{return}
        let pointer=shellPendingMove ?? (input.mouse_x,input.mouse_y);shellPendingMove=nil
        input.mouse_x=UInt32((rect[0]+rect[2])/2);input.mouse_y=UInt32((rect[1]+rect[3])/2)
        input.mouse_ae=1;input.mouse_af=0;input.mouse_b0=0
        input.clock_7c=macos_live_shell_click_clock(mode,input.clock_7c,1)
        sendShell(input:input)
        guard macos_live_shell_mode(driver)==mode,macos_live_shell_state(driver,&input)==0 else{return}
        input.mouse_x=pointer.0;input.mouse_y=pointer.1;input.mouse_ae=0;input.mouse_af=0;input.mouse_b0=0
        sendShell(input:input)
    }
    func shellEvent(_ event:[String:Any]){
        // PlayVideo 0x47c82c ends a movie only on WM_KEYUP VK_ESCAPE (0x47c96e); other input is ignored.
        if movieActive{
            if event["kind"] as? String == "key",event["vkey"] as? Int == 27,event["down"] as? Bool != true{
                DispatchQueue.main.async{self.defeatMovie.skip()}
            }
            return
        }
        guard !hostRequest else{return}
        // An interrupted press is not a menu click in the original screens.
        if event["cancel"] as? Bool == true{shellPendingMove=nil;shellMouseDown=false;return}
        let kind=event["kind"] as? String
        let mode=macos_live_shell_mode(driver)
        let pages=mode==UInt32(RL_SHELL_BRIEFING) || mode==UInt32(RL_SHELL_OBJECTIVES)
        if kind=="page"{pageObjectives(next:event["next"] as? Bool ?? true);return}
        if kind=="key"{
            let key=event["vkey"] as? Int ?? 0
            // The original pages only with its arrows; Up/Down are host conveniences there.
            let arrow:Bool?=pages ? [38:false,40:true][key]:nil
            if event["down"] as? Bool == true{
                if shellHeld.insert(key).inserted,let next=arrow{pageObjectives(next:next)}
                return
            }
            guard shellHeld.remove(key) != nil,arrow==nil else{return}
            let keys:UInt32 = [13:1,38:2,40:4,27:8][key] ?? 0
            if keys != 0{sendShell(keys:keys)}
        }else{
            let message=event["msg"] as? Int ?? 0
            if message==0x200{
                shellMovesReceived+=1
                shellPendingMove=(UInt32(event["x"] as? Int ?? 0),UInt32(event["y"] as? Int ?? 0));scheduleShellMove();return
            }
            shellPendingMove=nil // the press/release carries the newer position
            var input=RlShellInput();guard macos_live_shell_state(driver,&input)==0 else{return}
            input.mouse_x=UInt32(event["x"] as? Int ?? 0);input.mouse_y=UInt32(event["y"] as? Int ?? 0)
            input.mouse_af=0;input.mouse_b0=0
            let press=message==0x201 || message==0x203
            if press{shellMouseDown=true}
            input.mouse_ae=shellMouseDown ? 1:0
            if message==0x202{input.mouse_af=shellMouseDown ? 1:0;shellMouseDown=false}
            // Objective/briefing arrows: one page per click (see macos_live_shell_click_clock).
            if press || input.mouse_ae != 0{input.clock_7c=macos_live_shell_click_clock(mode,input.clock_7c,press ? 1:0)}
            sendShell(input:input)
        }
    }
    /// Worker. Captions of the modern saves posicao-1..10 for the original lists
    /// (save1.sav..save9.sav,save0.sav); the engine never reads save files here.
    func publishSlots()->Int32{
        let names=(try? PropertyListSerialization.propertyList(from:Data(contentsOf:saveDirectory.appendingPathComponent("nomes.plist")),format:nil)) as? [String:String] ?? [:]
        let formatter=DateFormatter();formatter.dateFormat="dd/MM HH:mm"
        var table=[UInt8](repeating:0,count:330)
        for slot in 0..<10 {
            let url=saveURL("posicao-\(slot+1)")
            guard let date=(try? FileManager.default.attributesOfItem(atPath:url.path))?[.modificationDate] as? Date else{continue}
            let caption="\(names[String(slot+1)] ?? "Posição \(slot+1)") · \(formatter.string(from:date))"
            let bytes=Array((caption.data(using:.windowsCP1252,allowLossyConversion:true) ?? Data()).prefix(31))
            table[33*slot]=1;for (i,b) in bytes.enumerated(){table[33*slot+1+i]=b}
        }
        return macos_live_shell_slots(driver,table,UInt32(table.count))
    }
    /// The pending LOAD_SLOT request is the host boundary: the modern load replaces
    /// the world (and the session); a failure reopens the original main menu.
    var hostRequest=false
    func beginSlotLoad(_ slot:Int){
        hostRequest=true;shellRun &+= 1;shellHeld.removeAll();shellMouseDown=false;deliver()
        DispatchQueue.main.async{ [weak self] in
            guard let self else{return}
            self.metal.releaseKeys()
            self.loadSlot("posicao-\(slot+1)"){result in
                guard case .failure(let error)=result else{self.savesChanged?();return}
                let alert=NSAlert();alert.messageText="Não foi possível carregar a partida";alert.informativeText=error.localizedDescription
                alert.beginSheetModal(for:self.window){_ in self.reopenMenu()}
            }
        }
    }
    /// Worker. SAVE_SLOT completed: the game is already saved (or the failure is
    /// reported) and resumed; play continues like after "Voltar ao jogo".
    func finishSlotSave(_ saved:Int32,job:OpaquePointer?,path:String,clicked:Double){
        if saved==0,let job{
            commitSave(job,path:path,captured:clicked){[weak self] failure in self?.reportSlotSave(failure)}
        }else{
            reportSlotSave(saveError(saved).localizedDescription)
        }
        finishShellResume()
    }
    /// Worker. Reported only after the files were written and synced (or failed;
    /// the previous save of that slot is then intact).
    func reportSlotSave(_ failure:String?){
        if failure==nil{saveNotice="Partida salva"}
        deliver()
        DispatchQueue.main.async{ [weak self] in
            guard let self else{return}
            self.savesChanged?()
            if let failure{let alert=NSAlert();alert.messageText="Não foi possível salvar a partida";alert.informativeText=failure;alert.beginSheetModal(for:self.window)}
        }
    }
    func reopenMenu(){
        campaignSelection=CampaignSelection(mission:CampaignSelection.menu,difficulty:campaignSelection.difficulty)
        updateTitle();perform("live-reset",completion:nil)
    }
    func shellTick(_ run:UInt64){
        guard run==shellRun,!movieActive,!hostRequest,macos_live_shell_mode(driver) != 0,stoppedReason==nil else{return}
        var input=RlShellInput();guard macos_live_shell_state(driver,&input)==0 else{return}
        input.clock_7c &+= 1;input.animation_84 &+= 1
        input.mouse_ae=shellMouseDown ? 1:0;input.mouse_af=0
        if let move=shellPendingMove{input.mouse_x=move.x;input.mouse_y=move.y;shellPendingMove=nil;shellMovesApplied+=1;shellLastMove=ProcessInfo.processInfo.systemUptime}
        sendShell(input:input)
        // AffCredits advances by the host clock (71 ms per CREDITS.FLC frame): tick finer there.
        work.asyncAfter(deadline:.now()+(macos_live_shell_mode(driver)==UInt32(RL_SHELL_CREDITS_SCREEN) ? 0.01:0.05)){[weak self] in self?.shellTick(run)}
    }
    func sendShell(keys:UInt32){
        var input=RlShellInput();guard macos_live_shell_state(driver,&input)==0 else{return}
        input.keys=keys;input.mouse_ae=0;input.mouse_af=0
        sendShell(input:input)
    }
    func sendShell(input supplied:RlShellInput){
        guard !movieActive,!hostRequest,macos_live_shell_mode(driver) != 0,stoppedReason==nil else{return}
        var input=supplied,event=RlShellEvent()
        var code=macos_live_shell_input(driver,&input,&event)
        if code==0 && event.kind != UInt32(RL_SHELL_NONE){shellZones.removeAll()}
        if code==0{
            switch event.kind{
            case UInt32(RL_SHELL_NONE):
                if macos_live_shell_mode(driver)==UInt32(RL_SHELL_CREDITS_SCREEN){
                    var changed=false
                    code=creditsFrame(changed:&changed)
                    if code==0 && !changed{return}
                }else{code=macos_live_shell_draw(driver,input.animation_84)}
            case UInt32(RL_SHELL_SHOW):
                credits=nil;code=publishSlots();if code==0{code=macos_live_shell_show(driver)};shellHeld.removeAll();shellMouseDown=false
            case UInt32(RL_SHELL_QUIT):
                shellRun &+= 1;waitForSaves()
                DispatchQueue.main.async{self.defeatMovie.cancel();self.closeCDMusic();NSApp.terminate(nil)}
                return
            case UInt32(RL_SHELL_MISSION_NEXT):
                code=macos_live_shell_prepare(driver);shellHeld.removeAll();shellMouseDown=false
            case UInt32(RL_SHELL_DEFEAT_MOVIE),UInt32(RL_SHELL_CAMPAIGN_MOVIE),UInt32(RL_SHELL_INTRODUCTION):
                // Defeat (Over_fr.avi), campaign end (extrofr.avi) and Introdução (introfr.avi) share the original completion.
                beginDefeatMovie(event);return
            case UInt32(RL_SHELL_SAVE_LOAD):
                code=publishSlots();if code==0{code=macos_live_shell_show(driver)};shellHeld.removeAll();shellMouseDown=false
            case UInt32(RL_SHELL_RESUME):
                // Pause "continuar"/Escape: original in-game MenuInit(18), same clock and frame.
                code=macos_live_shell_request(driver)
                if code==0{finishShellResume();return}
            case UInt32(RL_SHELL_RESET_CAMPAIGN):
                // Single app entry for "Novo jogo"/"Carregar jogo"; the driver owns the engine call.
                code=publishSlots();if code==0{code=macos_live_shell_request(driver)};shellHeld.removeAll();shellMouseDown=false
            case UInt32(RL_SHELL_LOAD_SLOT):
                beginSlotLoad(Int(event.reserved));return
            case UInt32(RL_SHELL_SAVE_SLOT):
                // Modern save of the paused game first, then the original MenuInit(18) resume.
                var saved:Int32=0;var job:OpaquePointer?
                let clicked=ProcessInfo.processInfo.systemUptime
                try? FileManager.default.createDirectory(at:saveDirectory,withIntermediateDirectories:true)
                let path=saveURL("posicao-\(Int(event.reserved)+1)").path
                code=macos_live_shell_save_slot_begin(driver,path,&saved,&job)
                if code==0{finishSlotSave(saved,job:job,path:path,clicked:clicked);return}
            case UInt32(RL_SHELL_START_MISSION):
                code=macos_live_shell_load(driver)
                if code==0{shellRun &+= 1;finishShellCampaign();return}
            case UInt32(RL_SHELL_CREDITS):
                code=beginCredits();shellHeld.removeAll();shellMouseDown=false
            case UInt32(RL_SHELL_MULTIPLAYER):
                unavailableMenuItem("O jogo em rede não faz parte desta versão",
                    detail:"O menu principal será reaberto do início.");return
            default:
                stoppedReason="Pedido da tela original ainda não disponível: \(event.kind)"
            }
        }
        if code != 0{stoppedReason=saveError(code).localizedDescription}
        showShellPicture();deliver()
    }
    /// CREDITS (MenuInit 10): opens the host FLC decoder before the engine's MenuInit. An absent
    /// file is the original's failed open (the engine then ends the screen by itself).
    func beginCredits()->Int32{
        let url=root.appendingPathComponent("events/CREDITS.FLC")
        do{credits=FileManager.default.fileExists(atPath:url.path) ? try NativeFLC(contentsOf:url):nil}
        catch{stoppedReason="CREDITS.FLC: \(error)";return 0}
        let code=macos_live_shell_show(driver);creditsRead=ProcessInfo.processInfo.systemUptime
        if code==0{DispatchQueue.main.async{self.cdMusic?.leaveStage()}}
        return code
    }
    /// Worker. One AffCredits frame: the engine owns the counter, the host the decoder and clock
    /// (elapsed = ms since the last FLC frame read, as ReadFli's stored time). changed = new picture.
    func creditsFrame(changed:inout Bool)->Int32{
        var out=RlShellCredits()
        let code=macos_live_shell_credits_frame(driver,UInt32(max(0,ProcessInfo.processInfo.systemUptime-creditsRead)*1000),&out)
        guard code==0 else{return code}
        if out.state==UInt32(RL_SHELL_CREDITS_ENDED){credits=nil;changed=true;return 0}
        guard out.state==UInt32(RL_SHELL_CREDITS_DRAW) else{return 0}
        guard let flc=credits,flc.width==Int(out.width),flc.height==Int(out.height) else{stoppedReason="CREDITS.FLC: cabeçalho diferente do declarado";changed=true;return 0}
        guard flc.shown<=Int(out.frame) else{return 0}
        do{while flc.shown<=Int(out.frame){try flc.next()}}catch{stoppedReason="CREDITS.FLC: \(error)";changed=true;return 0}
        creditsRead=ProcessInfo.processInfo.systemUptime;changed=true
        return flc.pixels.withUnsafeBufferPointer{p in flc.palette.withUnsafeBufferPointer{c in macos_live_shell_credits_present(driver,&out,p.baseAddress,c.baseAddress)}}
    }
    /// A main-menu item whose request the engine cannot complete yet: report it and
    /// boot the main menu again as at launch (the pending session is discarded).
    func unavailableMenuItem(_ title:String,detail:String){
        hostRequest=true;shellRun &+= 1;shellHeld.removeAll();shellMouseDown=false;deliver()
        DispatchQueue.main.async{ [weak self] in
            guard let self else{return}
            self.metal.releaseKeys()
            let alert=NSAlert();alert.messageText=title;alert.informativeText=detail
            alert.beginSheetModal(for:self.window){_ in self.reopenMenu()}
        }
    }
    func beginDefeatMovie(_ request:RlShellEvent){
        var request=request
        moviePath=withUnsafeBytes(of:&request.path){String(data:Data($0.prefix(Int(request.path_length))),encoding:.windowsCP1252) ?? ""}
        movieActive=true;introductionMovie=request.kind==UInt32(RL_SHELL_INTRODUCTION);movieStatus="Pedido original pendente";shellRun &+= 1
        let run=shellRun
        shellHeld.removeAll();shellMouseDown=false;showShellPicture();deliver()
        DispatchQueue.main.async{
            self.metal.releaseKeys();self.cdMusic?.leaveStage()
            // Packaged original movies keep their uppercase CD names; an unpackaged one is unavailable.
            let name=(self.moviePath.split(whereSeparator:{$0=="\\" || $0=="/"}).last.map(String.init) ?? "").uppercased()
            self.defeatMovie.play(self.root.appendingPathComponent("movies/\(name)"),over:self.metal){[weak self] completion,detail in
                self?.work.async{[weak self] in
                    guard let self=self,self.shellRun==run,self.movieActive else{return}
                    self.movieActive=false;self.movieStatus=detail
                    let status=self.movieCompletion(completion == .finished,file:name)
                    let code=macos_live_shell_movie_finish(self.driver,status)
                    if code != 0{self.stoppedReason=self.saveError(code).localizedDescription}
                    else{self.startShell()}
                    self.showShellPicture();self.deliver()
                }
            }
        }
    }
    /// Worker. Introdução: FINISHED whenever the file exists, played or not (the original
    /// would have called PlayVideo); UNAVAILABLE only for an absent file. Other movies keep theirs.
    func movieCompletion(_ played:Bool,file:String)->UInt32{
        let exists=introductionMovie && FileManager.default.fileExists(atPath:root.appendingPathComponent("movies/\(file)").path)
        introductionMovie=false
        return played || exists ? UInt32(RL_SHELL_MOVIE_FINISHED):UInt32(RL_SHELL_MOVIE_UNAVAILABLE)
    }
    /// Worker. The resumed world's first frame events (cursor/picture/music
    /// boundaries 101/102) are consumed before play continues at the same speed.
    func finishShellResume(){
        shellRun &+= 1;shellHeld.removeAll();shellMouseDown=false
        pumpCDMusic();showShellPicture()
        DispatchQueue.main.async{self.metal.releaseKeys();self.window.makeFirstResponder(self.metal)}
        perform("live-play",completion:nil)
    }
    func finishShellCampaign(){
            runID &+= 1;running=false;stoppedReason=nil;campaignEnd=nil;presenter?.clear();audio?.reset();refreshSelection()
            DispatchQueue.main.async{self.cdMusic?.beginStage()}
            // Continue already ran the next load tail. Consume its original
            // entry request before the first game frame replaces the event log.
            pumpCDMusic()
            playStarted=0;playEnded=0;startFrames=0;startLogic=0;startRender=0;startProof=0;startMetal=metalSeconds
            do {
                try FileManager.default.createDirectory(at:saveDirectory,withIntermediateDirectories:true)
                let saved=macos_live_autosave(driver,saveURL("inicio-fase").path)
                guard saved==0 else{throw saveError(saved)}
                DispatchQueue.main.async{self.metal.releaseKeys();self.displayStop(nil,frame:1);self.updateTitle();self.savesChanged?()}
                perform("live-play",completion:nil)
            }catch{
                saveNotice="Falha ao salvar automaticamente";deliver()
                DispatchQueue.main.async{let alert=NSAlert();alert.messageText="Falha ao salvar automaticamente";alert.informativeText=error.localizedDescription;alert.beginSheetModal(for:self.window)}
            }
    }
    func perform(_ operation:String,completion:(([String:Any])->Void)?){
        work.async { [weak self] in
            guard let self=self else{return}
            switch operation {
            case "live-play":
                if self.running{self.running=false;self.playEnded=ProcessInfo.processInfo.systemUptime}
                else if self.status().stopped==0 && self.stoppedReason==nil && macos_live_shell_mode(self.driver)==0{
                    let s=self.status();self.startFrames=s.frames;self.startLogic=s.logic_ns;self.startRender=s.render_ns;self.startProof=s.proof_ns;self.startMetal=self.metalSeconds;self.playStarted=ProcessInfo.processInfo.systemUptime;self.playEnded=self.playStarted;self.running=true
                    if !self.validationMode{self.presenter?.start(at:self.playStarted);DispatchQueue.main.async{self.metal.isHidden=false;self.window.makeFirstResponder(self.metal);if self.cdMusic?.stageActive==false{self.cdMusic?.beginStage()}}}
                    self.runID &+= 1;let run=self.runID;self.pacer.restart(now:self.playStarted);self.work.async{[weak self] in self?.pump(run)}
                }
            case "live-pause":self.running=false;self.playEnded=ProcessInfo.processInfo.systemUptime
            case "live-menu":
                if self.movieActive{
                    self.shellRun &+= 1;self.movieActive=false;self.movieStatus="Reprodução interrompida"
                    DispatchQueue.main.async{self.defeatMovie.cancel()}
                    let code=macos_live_shell_movie_finish(self.driver,self.movieCompletion(false,file:"INTROFR.AVI"))
                    if code != 0{self.stoppedReason=self.saveError(code).localizedDescription}
                }
                self.running=false;self.playEnded=ProcessInfo.processInfo.systemUptime
                DispatchQueue.main.async{self.cdMusic?.leaveStage()}
            case "live-step":if !self.running && self.status().stopped==0 && self.stoppedReason==nil && macos_live_shell_mode(self.driver)==0{self.advance();if !self.validationMode,let pixels=macos_live_pixels(self.driver),let palette=macos_live_palette(self.driver){DispatchQueue.main.sync{do{self.metal.isHidden=false;try self.metal.present(pixels,palette)}catch{self.stoppedReason="Metal: \(error.localizedDescription)"}}}}
            case "live-reset":
                self.shellRun &+= 1;self.movieActive=false;self.introductionMovie=false
                DispatchQueue.main.async{self.defeatMovie.cancel()}
                self.running=false;self.presenter?.stop();self.presenter?.clear()
                DispatchQueue.main.async{self.cdMusic?.leaveStage()}
                do{try self.create();self.campaignEnd=nil;DispatchQueue.main.async{self.metal.isHidden=true};self.stoppedReason=nil;self.playStarted=0;self.playEnded=0;self.startFrames=0;self.startLogic=0;self.startRender=0;self.startProof=0;self.startMetal=self.metalSeconds
                    if macos_live_shell_mode(self.driver) != 0{self.hostRequest=false;self.startShell();self.showShellPicture()}
                }catch{self.stoppedReason=error.localizedDescription}
            default:break
            }
            if !self.running && (macos_live_shell_mode(self.driver)==0 || self.movieActive){self.presenter?.stop()}
            if self.stoppedReason != nil && self.campaignEnd==nil{DispatchQueue.main.async{self.cdMusic?.leaveStage()}}
            self.deliver(completion)
        }
    }
    func read(_ address:UInt32,_ size:Int,completion:@escaping([String:Any])->Void){
        work.async{[weak self] in guard let self=self else{return};guard self.validationMode else{DispatchQueue.main.async{completion(["error":"Leitura de memória disponível no modo Validar."])};return};var data=[UInt8](repeating:0,count:size);let code=macos_live_read(self.driver,address,UInt32(size),&data);DispatchQueue.main.async{completion(code==0 ? ["read_hex":data.map{String(format:"%02x",$0)}.joined()]:["error":"Leitura nativa recusada: \(code)"])}}
    }
    func capture(_ url:URL,completion:@escaping([String:Any])->Void){
        work.async{[weak self] in
            guard let self=self else{return};self.running=false;self.playEnded=ProcessInfo.processInfo.systemUptime;self.presenter?.stop()
            guard let pixels=macos_live_pixels(self.driver),let palette=macos_live_palette(self.driver) else{DispatchQueue.main.async{completion(["error":"NativePixelsUnavailable"])};return}
            if !self.validationMode,let presenter=self.presenter{
                // Capture the actual play texture/upload/shader path. Draining makes a
                // free slot available even if the last timed publication was dropped.
                presenter.publish(pixels,palette,frame:self.status().frames)
                presenter.queue.sync{presenter.draw()};presenter.stop()
            }
            DispatchQueue.main.sync {
                do{
                    if !self.validationMode{
                        guard let texture=self.presenter?.lastTexture else{throw NSError(domain:"MetalPlayReadbackUnavailable",code:1)}
                        self.metal.lastTexture=texture
                    }
                    var result=try self.metal.capture(url,pixels:pixels,palette:palette)
                    result["transport"]=self.validationMode ? "validation-texture":"async-slot-texture";completion(result)
                }catch{completion(["error":error.localizedDescription])}
            }
        }
    }
    func windowShouldClose(_ sender:NSWindow)->Bool {perform("live-menu",completion:nil);window.orderOut(nil);return false}
}

/// `--ux-menu-trace` support (AppDelegate.startUXMenuTrace in main.swift).
extension NativeLiveWindow {
    /// Any thread. Presented-drawable timestamps of both presentation paths.
    func uxPresentedTimes(since start:Double)->[Double]{
        metal.testLock.lock();var times=metal.testPresented.filter{$0>=start};metal.testLock.unlock()
        if let presenter{presenter.lock.lock();times+=presenter.presented.filter{$0>=start};presenter.lock.unlock()}
        return times.sorted()
    }
    func uxCounters(_ completion:@escaping([String:Any])->Void){
        work.async{[weak self] in
            guard let self else{return}
            let value:[String:Any]=["movesReceived":self.shellMovesReceived,"movesApplied":self.shellMovesApplied,"shellMode":macos_live_shell_mode(self.driver),
                                    "running":self.running,"saveTrace":self.saveTrace,"saveNotice":self.saveNotice]
            DispatchQueue.main.async{completion(value)}
        }
    }
    static let menuTrace=CommandLine.arguments.contains("--ux-menu-trace")
}

