// apple-decode <out> <block> <side, 0 whole> <raw>...: `CIRAWFilter` set as the Swift app's
// `RawDecode.configured`, in linear Rec. 2020 floats, averaged over block squares, as `<name>.apple.f32`.
import CoreImage
import Foundation

let workingSpace = CGColorSpace(name: CGColorSpace.extendedLinearITUR_2020)!

func pinned(_ versions: [CIRAWDecoderVersion]) -> CIRAWDecoderVersion? {
    func number(_ v: CIRAWDecoderVersion) -> Int? { Int(v.rawValue.prefix { $0.isNumber }) }
    return versions.filter { (number($0) ?? .max) <= 8 }.max { (number($0) ?? 0) < (number($1) ?? 0) }
}

func configured(_ raw: CIRAWFilter) -> CIRAWFilter {
    if let version = pinned(raw.supportedDecoderVersions) { raw.decoderVersion = version }
    raw.boostAmount = 0
    raw.boostShadowAmount = 0
    raw.isGamutMappingEnabled = false
    raw.neutralTemperature = 5000
    raw.neutralTint = 0
    raw.shadowBias = 0
    raw.colorNoiseReductionAmount = 1
    raw.luminanceNoiseReductionAmount = 0
    raw.contrastAmount = 0
    raw.detailAmount = 0
    raw.sharpnessAmount = 0
    raw.moireReductionAmount = 0
    raw.localToneMapAmount = 0
    raw.isLensCorrectionEnabled = false
    if #available(macOS 26, *) { raw.isHighlightRecoveryEnabled = false }
    raw.extendedDynamicRangeAmount = 0
    return raw
}

let args = CommandLine.arguments
let out = URL(fileURLWithPath: args[1])
let block = Int(args[2])!
let side = Double(args[3])!
try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
let ctx = CIContext(options: [.workingColorSpace: workingSpace, .workingFormat: CIFormat.RGBAf, .cacheIntermediates: false])
for path in args[4...] {
    let url = URL(fileURLWithPath: path)
    guard let raw = CIRAWFilter(imageURL: url).map(configured) else { print("FAIL \(path)"); continue }
    if side > 0 {
        let native = max(raw.nativeSize.width, raw.nativeSize.height)
        raw.scaleFactor = Float(min(1, side / native))
    }
    guard let img = raw.outputImage else { print("FAIL output \(path)"); continue }
    let e = img.extent.integral
    let (w, h) = (Int(e.width), Int(e.height))
    var px = [Float](repeating: 0, count: w * h * 4)
    ctx.render(img, toBitmap: &px, rowBytes: w * 16, bounds: e, format: .RGBAf, colorSpace: workingSpace)
    let (bw, bh) = (w / block, h / block)
    var red = [Float](repeating: 0, count: bw * bh * 4)
    for y in 0..<bh { for x in 0..<bw {
        var s = [Double](repeating: 0, count: 3)
        for yy in 0..<block { for xx in 0..<block {
            let i = ((y * block + yy) * w + x * block + xx) * 4
            for c in 0..<3 { s[c] += Double(px[i + c]) }
        } }
        let n = Double(block * block)
        for c in 0..<3 { red[(y * bw + x) * 4 + c] = Float(s[c] / n) }
        red[(y * bw + x) * 4 + 3] = 1
    } }
    var data = Data()
    withUnsafeBytes(of: Int32(bw)) { data.append(contentsOf: $0) }
    withUnsafeBytes(of: Int32(bh)) { data.append(contentsOf: $0) }
    red.withUnsafeBytes { data.append(contentsOf: $0) }
    let name = url.lastPathComponent + ".apple.f32"
    try! data.write(to: out.appendingPathComponent(name))
    print("\(url.lastPathComponent): decoder \(raw.decoderVersion.rawValue), \(w)x\(h) -> \(bw)x\(bh), baseline \(raw.baselineExposure)")
}
