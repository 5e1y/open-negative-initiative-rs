import CoreImage
import Foundation

/// Probe-only instruments for the Rust port's parity checks; never part of the Swift app itself.
enum ProbeTools {
    /// Width, height as Int32, then RGBA floats, top row first, as `RenderDump` writes.
    static func dump(_ image: CIImage, to url: URL, bounds: CGRect? = nil) -> Bool {
        let extent = (bounds ?? image.extent).integral
        let (w, h) = (Int(extent.width), Int(extent.height))
        guard w > 0, h > 0 else { return false }
        var raw = [Float](repeating: 0, count: w * h * 4)
        Pipeline.measureContext.render(image, toBitmap: &raw, rowBytes: w * 16, bounds: extent,
                                       format: .RGBAf, colorSpace: RawDecode.workingSpace)
        var data = Data()
        withUnsafeBytes(of: Int32(w)) { data.append(contentsOf: $0) }
        withUnsafeBytes(of: Int32(h)) { data.append(contentsOf: $0) }
        raw.withUnsafeBytes { data.append(contentsOf: $0) }
        return (try? data.write(to: url)) != nil
    }

    static func lines(_ list: String) -> [String] {
        ((try? String(contentsOfFile: list, encoding: .utf8)) ?? "").split(separator: "\n").map(String.init).filter { !$0.isEmpty }
    }

    /// `--probe-decode <list> <folder> [side]`: `RawDecode.linear` of each file, held or converted as its sidecar says.
    static func decode(list: String, folder: String, side: CGFloat?) -> Int32 {
        try? FileManager.default.createDirectory(atPath: folder, withIntermediateDirectories: true)
        var written = 0
        for (i, path) in lines(list).enumerated() {
            guard let linear = RawDecode.linear(URL(fileURLWithPath: path), longestSide: side) else { print("unread \(path)"); continue }
            if dump(linear, to: URL(fileURLWithPath: folder).appendingPathComponent(String(format: "%05d.f32", i))) { written += 1 }
        }
        print("decoded \(written) of \(lines(list).count)")
        return written > 0 ? 0 : 1
    }

    /// `--probe-heal <list> <folder>`: every correction of each file generated in order on the full
    /// decode, its patch copied out, its source rectangle and a crop of the source it searched written.
    static func heal(list: String, folder: String) -> Int32 {
        let out = URL(fileURLWithPath: folder)
        try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
        var report: [[String: Any]] = []
        for (i, path) in lines(list).enumerated() {
            let url = URL(fileURLWithPath: path)
            guard let decoded = RawDecode.linear(url), let full = RawDecode.materialisedOnGPU(decoded),
                  let settings = try? Sidecar.read(for: url).settings else { print("unread \(path)"); continue }
            for (k, c) in settings.corrections.enumerated() {
                let source = Pipeline.correctionSource(full, settings: settings, excluding: c.id)
                let extent = source.extent
                let done = DispatchSemaphore(value: 0)
                nonisolated(unsafe) var rect: CGRect?
                Task.detached { rect = await CorrectionRenderer.generate(c, source: source, settings: settings); done.signal() }
                done.wait()
                Pipeline.evictCorrectionCache(c.id)
                let name = String(format: "%05d-%02d", i, k)
                if let data = CorrectionCache.load(c.id) { try? data.write(to: out.appendingPathComponent(name + ".tiff")) }
                // The port's search window, padded: the crop holds every pixel its search may read.
                let tile = CorrectionRenderer.contextRect(for: c, in: extent)
                let side = max(tile.width, tile.height)
                let reach = (side * 1.6 + side) * 1.5 + 16 + 8
                let crop = CGRect(x: tile.midX - reach, y: tile.midY - reach, width: 2 * reach, height: 2 * reach)
                    .intersection(extent).integral
                _ = dump(source, to: out.appendingPathComponent(name + "-source.f32"), bounds: crop)
                let patch = CorrectionRenderer.patchRect(for: c, in: extent)
                // As `applyCorrections` lays it on the 1000 px render: scaled, placed, masked.
                var reduced: [String: Any] = [:]
                if let small = RawDecode.linear(url, longestSide: 1000)?.extent, let data = CorrectionCache.load(c.id),
                   let atFull = CIImage(data: data) {
                    let scale = small.width / extent.width
                    let fullExtent = CGRect(x: small.minX / scale, y: small.minY / scale, width: small.width / scale, height: small.height / scale)
                    let t = CorrectionRenderer.contextRect(for: c, in: fullExtent)
                    let fullTile = atFull.extent.size == t.size ? t : CorrectionRenderer.patchRect(for: c, in: fullExtent)
                    let scaled = atFull.applyingFilter("CILanczosScaleTransform", parameters: [kCIInputScaleKey: scale, kCIInputAspectRatioKey: 1])
                    let tileRect = CGRect(x: (fullTile.minX * scale).rounded(), y: (fullTile.minY * scale).rounded(),
                                          width: scaled.extent.width, height: scaled.extent.height)
                    let mask = CorrectionRenderer.blendMask(for: c, imageExtent: small, tileRect: tileRect, scale: scale)
                    _ = dump(scaled, to: out.appendingPathComponent(name + "-scaled.f32"))
                    _ = dump(mask, to: out.appendingPathComponent(name + "-mask.f32"))
                    let hard = CorrectionRenderer.strokeMask(c, imageExtent: small, tileRect: tileRect, scale: scale)
                    _ = dump(hard, to: out.appendingPathComponent(name + "-hard.f32"))
                    reduced = ["scale": scale, "small": [small.minX, small.minY, small.width, small.height],
                               "scaled": [scaled.extent.minX, scaled.extent.minY, scaled.extent.width, scaled.extent.height],
                               "placed": [tileRect.minX, tileRect.minY, tileRect.width, tileRect.height],
                               "mask": [mask.extent.minX, mask.extent.minY, mask.extent.width, mask.extent.height]]
                }
                report.append([
                    "reduced": reduced,
                    "index": i, "k": k, "id": c.id.uuidString, "file": url.lastPathComponent,
                    "extent": [extent.minX, extent.minY, extent.width, extent.height],
                    "crop": [crop.minX, crop.minY, crop.width, crop.height],
                    "patch": [patch.minX, patch.minY, patch.width, patch.height],
                    "tile": [tile.minX, tile.minY, tile.width, tile.height],
                    "source": rect.map { [$0.minX, $0.minY, $0.width, $0.height] } ?? [],
                ])
            }
        }
        let json = try? JSONSerialization.data(withJSONObject: report, options: [.prettyPrinted, .sortedKeys])
        try? json?.write(to: out.appendingPathComponent("heal.json"))
        print("healed \(report.count) corrections")
        return report.isEmpty ? 1 : 0
    }

    /// `--probe-export <list> <folder>`: each line `path<TAB>format<TAB>space<TAB>longest edge or 0`,
    /// exported through `Export.write` with the frame's sidecar and the scan's own metadata.
    static func export(list: String, folder: String) -> Int32 {
        let out = URL(fileURLWithPath: folder)
        try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
        var written = 0
        for (i, line) in lines(list).enumerated() {
            let f = line.split(separator: "\t").map(String.init)
            guard f.count == 4, let settings = try? Sidecar.read(for: URL(fileURLWithPath: f[0])).settings else { continue }
            var export = Export.Settings()
            export.format = f[1] == "tiff16" ? .tiff16 : .jpeg
            export.space = Export.Space(key: f[2]) ?? .displayP3
            if let edge = Int(f[3]), edge > 0 { export.resize = .longestEdge(edge) }
            let dest = out.appendingPathComponent(String(format: "%05d.%@", i, export.format.fileExtension))
            do {
                try Export.write(source: URL(fileURLWithPath: f[0]), to: dest, settings: settings, export: export,
                                 metadata: Export.Metadata(), dateOverride: nil)
                written += 1
            } catch { print("failed \(line): \(error)") }
        }
        print("exported \(written) of \(lines(list).count)")
        return written > 0 ? 0 : 1
    }
}
