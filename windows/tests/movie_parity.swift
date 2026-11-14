// Movie decoding parity, macOS side: NativeDefeatMovie.swift (read-only) decodes
// every frame and the whole audio of each AVI; one line per movie:
// "<NAME> <frames> <pcm samples> <sha256 of every RGBX frame in order> <sha256 of the PCM>".
// audio_parity.sh compiles it with swiftc and compares with audio_parity's "movies" mode.
import Foundation
import CryptoKit
for path in CommandLine.arguments.dropFirst() {
    let movie = try NativeAVIMovie(data: Data(contentsOf: URL(fileURLWithPath: path), options: .alwaysMapped))
    let decoder = NativeCinepakDecoder(width: movie.width, height: movie.height)
    var frameHash = SHA256()
    for chunk in movie.videoChunks { try decoder.decode(movie.data[chunk]); decoder.pixels.withUnsafeBytes { frameHash.update(bufferPointer: $0) } }
    let pcm = try movie.pcm()
    let pcmHash = pcm.withUnsafeBytes { SHA256.hash(data: Data($0)) }
    print(path.split(separator: "/").last!, movie.frameCount, pcm.count, frameHash.finalize().map { String(format: "%02x", $0) }.joined(), pcmHash.map { String(format: "%02x", $0) }.joined())
}
