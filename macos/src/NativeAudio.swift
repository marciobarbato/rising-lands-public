import AVFoundation
import Foundation

// Play-mode sound. The game worker drains the engine's ordered sound requests
// (macos_live_sound_events) into the C mapper, which reproduces the original
// initialized queue and emits device commands. Commands run on a separate audio
// queue; AVAudioEngine renders on its own thread. Nothing here reads or writes
// game state, and validation mode never enables the transport.
final class NativeAudioPlayback {
    let directory:URL
    let queue=DispatchQueue(label:"risinglands.native.audio",qos:.userInitiated)
    let format=AVAudioFormat(commonFormat:.pcmFormatFloat32,sampleRate:22050,channels:1,interleaved:false)!
    private var mapper:OpaquePointer?
    private var heds=[(UInt32,Data)]()
    private let events=UnsafeMutablePointer<MacOSSoundEvent>.allocate(capacity:256)
    private let commands=UnsafeMutablePointer<MacOSAudioCommand>.allocate(capacity:256)
    private let lock=NSLock()
    private var finished=[UInt32]()          // device answers, guarded by lock
    // Audio queue state.
    private let engine=AVAudioEngine()
    private var nodes=[AVAudioPlayerNode](),idle=[Int]()
    private var buffers=[UInt32:(node:Int,pcm:AVAudioPCMBuffer?)]()
    private var playGenerations=[UInt32:UInt64]()  // audio queue; monotonic per buffer id
    private var cache=[String:AVAudioPCMBuffer]()
    private var deviceAvailable=false
    private(set) var availability:String
    // Worker counters (read by snapshots on the worker).
    private(set) var requests:UInt64=0,commandCount:UInt64=0
    private var playsStarted:UInt64=0,missingSamples:UInt64=0  // written on the audio queue, guarded by lock

    init(directory:URL) {
        self.directory=directory
        var loaded=[(UInt32,Data)]()
        for (selector,letter) in [(UInt32(0x41),"A"),(0x42,"B"),(0x50,"P")] {
            if let data=try? Data(contentsOf:directory.appendingPathComponent("\(letter).HED")){loaded.append((selector,data))}
        }
        heds=loaded
        availability=loaded.count==3 && FileManager.default.fileExists(atPath:directory.appendingPathComponent("P").path) ? "pacote":"ausente"
        if availability=="pacote" {
            queue.sync {
                for _ in 0..<24 {let node=AVAudioPlayerNode();engine.attach(node);engine.connect(node,to:engine.mainMixerNode,format:format);idle.append(nodes.count);nodes.append(node)}
                engine.prepare()
                do{try engine.start();deviceAvailable=true}catch{deviceAvailable=false}
            }
            if !deviceAvailable{availability="sem dispositivo"}
        }
        reset()
    }
    deinit {if let m=mapper{macos_audio_free(m)};events.deallocate();commands.deallocate()}

    var enabled:Bool{availability != "ausente"}
    /// Effects level of the Áudio menu (mute = 0), on the mixer output: the original
    /// per-buffer volumes stay as the mapper sends them. Any queue.
    private(set) var outputGain:Float=1
    func setOutputGain(_ value:Float) {
        let gain=value.isFinite ? min(1,max(0,value)):1
        outputGain=gain
        queue.async{[weak self] in self?.engine.mainMixerNode.outputVolume=gain}
    }

    /// Worker queue: a fresh mapper for a fresh world; silences the device.
    func reset() {
        if let m=mapper{macos_audio_free(m)};mapper=nil
        if enabled,let m=macos_audio_create() {
            var ok=true
            for (selector,data) in heds {ok = ok && data.withUnsafeBytes{macos_audio_load_bank(m,selector,$0.bindMemory(to:UInt8.self).baseAddress,UInt32(data.count))==0}}
            if ok{mapper=m}else{macos_audio_free(m);availability="ausente"}
        }
        lock.lock();finished.removeAll();lock.unlock()
        queue.async{[weak self] in self?.silence()}
    }

    /// Worker queue, after every play-mode advance. Constant work when idle.
    func pump(_ driver:OpaquePointer?) {
        guard let mapper=mapper,let driver=driver else{return}
        lock.lock()
        if !finished.isEmpty{for id in finished{macos_audio_device_stopped(mapper,id)};finished.removeAll(keepingCapacity:true)}
        lock.unlock()
        while true {
            let n=macos_live_sound_events(driver,events,256)
            for i in 0..<Int(n){_=macos_audio_request(mapper,events+i)}
            requests+=UInt64(n)
            if n<256{break}
        }
        drain(mapper)
    }

    /// Worker queue. One modal-event display tick of EvenementAffiche
    /// (0x45827f): with the latch clear and the FLC counter still below its
    /// frame count, while the start sound's shadow reference 0x496cd0 is held,
    /// post the event loop PostSound(0x41, 0x40200, 0x496cd4, 1) at volume 0x100.
    /// The first post starts sample 4/2 looping; later ones only refresh its
    /// volume; the end path's StopAllSound stops it.
    func eventTick(frame:UInt32) {
        guard let mapper=mapper,macos_audio_reference(mapper,0x496cd0) != UInt32.max else{return}
        var event=MacOSSoundEvent(kind:1,frame:frame,phase:3,site:0x45827f,eax:0x41,edx:0x40200,ebx:0x496cd4,ecx:1,volume:0x100,has_volume:1)
        _=macos_audio_request(mapper,&event);requests+=1;eventTicks+=1
        drain(mapper)
    }
    private(set) var eventTicks:UInt64=0

    private func drain(_ mapper:OpaquePointer) {
        while true {
            let n=Int(macos_audio_commands(mapper,commands,256))
            if n==0{return}
            commandCount+=UInt64(n)
            var batch=[(MacOSAudioCommand,String?)]();batch.reserveCapacity(n)
            for i in 0..<n {
                let c=commands[i];var path:String?
                if c.op==UInt32(MACOS_AUDIO_CREATE){
                    var bank:UInt32=0,index:UInt32=0,variant:UInt32=0
                    if macos_audio_sample(mapper,c.source,&bank,&index,&variant)==0{path=String(format:"%@/%04u-v%02u.wav",["A","B","P"][Int(bank)],index,variant)}
                }
                batch.append((c,path))
            }
            queue.async{[weak self] in self?.apply(batch)}
        }
    }

    func metrics()->[String:Any] {
        var stats=MacOSAudioStats();if let m=mapper{macos_audio_stats(m,&stats)}
        // Never waits for the audio queue (it may be decoding a batch): the game
        // worker calls this; the two counters are read under the small lock.
        lock.lock();let played=(playsStarted,missingSamples);lock.unlock()
        return ["audio":availability,"audioOutputGain":Double(outputGain),"audioRequests":requests,"audioPosts":stats.posts,"audioStops":stats.stops,"audioManages":stats.manages,
                "audioCommands":commandCount,"audioPlays":stats.plays,"audioPlaysStarted":played.0,"audioMissingSamples":played.1,
                "audioDomainFaults":stats.domain_faults,"audioDroppedCommands":stats.dropped_commands,"audioStopAlls":stats.stop_alls,"audioEventTicks":eventTicks]
    }

    // MARK: audio queue
    private func silence() {
        for (_,b) in buffers{nodes[b.node].stop();idle.append(b.node)}
        buffers.removeAll()
    }
    private func gain(_ hundredthsDB:Int32)->Float {hundredthsDB <= -10000 ? 0:Float(pow(10.0,Double(min(0,hundredthsDB))/2000.0))}
    private func sample(_ path:String)->AVAudioPCMBuffer? {
        if let pcm=cache[path]{return pcm}
        guard let data=try? Data(contentsOf:directory.appendingPathComponent(path)),let pcm=NativeAudioPlayback.decode(data,format:format) else{lock.lock();missingSamples+=1;lock.unlock();return nil}
        cache[path]=pcm;return pcm
    }
    /// RIFF/WAVE PCM16 mono 22,050 Hz from the pinned asset release.
    static func decode(_ data:Data,format:AVAudioFormat)->AVAudioPCMBuffer? {
        let bytes=[UInt8](data)
        func u32(_ o:Int)->Int{Int(bytes[o])|Int(bytes[o+1])<<8|Int(bytes[o+2])<<16|Int(bytes[o+3])<<24}
        func u16(_ o:Int)->Int{Int(bytes[o])|Int(bytes[o+1])<<8}
        guard bytes.count>=12,bytes[0..<4].elementsEqual(Array("RIFF".utf8)),bytes[8..<12].elementsEqual(Array("WAVE".utf8)) else{return nil}
        var offset=12,valid=false
        while offset+8<=bytes.count {
            let size=u32(offset+4),body=offset+8
            if bytes[offset..<offset+4].elementsEqual(Array("fmt ".utf8)) {
                guard size>=16,body+16<=bytes.count else{return nil}
                valid=u16(body)==1&&u16(body+2)==1&&u32(body+4)==22050&&u16(body+14)==16
            } else if bytes[offset..<offset+4].elementsEqual(Array("data".utf8)) {
                guard valid,body+size<=bytes.count else{return nil}
                let frames=size/2
                guard frames>0,let pcm=AVAudioPCMBuffer(pcmFormat:format,frameCapacity:AVAudioFrameCount(frames)),let out=pcm.floatChannelData?[0] else{return nil}
                for i in 0..<frames{out[i]=Float(Int16(bitPattern:UInt16(bytes[body+2*i])|UInt16(bytes[body+2*i+1])<<8))/32768}
                pcm.frameLength=AVAudioFrameCount(frames);return pcm
            }
            offset=body+size+(size&1)
        }
        return nil
    }
    private func node(for id:UInt32,pcm:AVAudioPCMBuffer?) {
        if idle.isEmpty {let n=AVAudioPlayerNode();engine.attach(n);engine.connect(n,to:engine.mainMixerNode,format:format);idle.append(nodes.count);nodes.append(n)}
        buffers[id]=(idle.removeLast(),pcm)
    }
    private func ended(_ id:UInt32) {lock.lock();finished.append(id);lock.unlock()}
    private func apply(_ batch:[(MacOSAudioCommand,String?)]) {
        for (c,path) in batch {
            switch Int(c.op) {
            case Int(MACOS_AUDIO_CREATE):node(for:c.buffer,pcm:path.flatMap{sample($0)})
            case Int(MACOS_AUDIO_DUPLICATE):node(for:c.buffer,pcm:buffers[c.source]?.pcm)
            case Int(MACOS_AUDIO_PLAY):
                guard let b=buffers[c.buffer] else{continue}
                let player=nodes[b.node],id=c.buffer
                player.volume=gain(c.volume)
                // Each play of a buffer has a generation: a replaced play's
                // completion never reports the new play as finished.
                let generation=(playGenerations[id] ?? 0) &+ 1;playGenerations[id]=generation
                guard deviceAvailable,let pcm=b.pcm else{if player.isPlaying{player.stop()};ended(id);continue}
                // .interrupts replaces what this node plays without a synchronous
                // stop(); play() only when the node is idle (fewer audio-queue stalls).
                player.scheduleBuffer(pcm,at:nil,options:c.loop != 0 ? [.interrupts,.loops]:[.interrupts],completionCallbackType:.dataPlayedBack){[weak self] _ in
                    self?.queue.async{guard let self=self,self.buffers[id] != nil,self.playGenerations[id]==generation else{return};self.ended(id)}
                }
                if !player.isPlaying{player.play()};lock.lock();playsStarted+=1;lock.unlock()
            case Int(MACOS_AUDIO_VOLUME):if let b=buffers[c.buffer]{nodes[b.node].volume=gain(c.volume)}
            case Int(MACOS_AUDIO_STOP):if let b=buffers[c.buffer]{nodes[b.node].stop()}
            case Int(MACOS_AUDIO_RELEASE):if let b=buffers.removeValue(forKey:c.buffer){nodes[b.node].stop();idle.append(b.node)}
            default:break
            }
        }
    }
}
