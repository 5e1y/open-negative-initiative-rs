//! The HDR file: the SDR JPEG exactly as written without it, plus a flat gain map and the
//! metadata two families of readers look for. Bytes laid in a stated order; nothing decodes here.

use crate::jpeg;

/// The lift every pixel gets on an HDR screen, as a ratio: 4 is two stops, the ceiling
/// Instagram's clients show and the point past which a post reads as a flash.
pub const LIFT: f32 = 4.0;

pub const SUBSAMPLING: usize = 2;

/// The base, then the container's two segments after its Exif, then the gain-map JPEG.
pub fn file(base: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
    let map = with_xmp(&map_jpeg(width, height)?, &map_xmp());
    let xmp = app1(&container_xmp(map.len()));
    let at = insertion_point(base);
    // The MPF index states the primary's own size, which includes the index itself.
    let index_length = mpf(0, 0, 0).len();
    let primary_size = base.len() + xmp.len() + index_length;
    // Offsets are relative to the MP header, 8 bytes into the segment: marker, length, "MPF\0".
    let header = at + xmp.len() + 8;
    let index = mpf(primary_size, map.len(), primary_size - header);
    let mut out = Vec::with_capacity(primary_size + map.len());
    out.extend(&base[..at]);
    out.extend(&xmp);
    out.extend(&index);
    out.extend(&base[at..]);
    out.extend(&map);
    Some(out)
}

/// The Ultra HDR directory in the primary: what an Android or Chrome reader opens first.
fn container_xmp(map_length: usize) -> String {
    format!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
         <rdf:Description rdf:about=\"\" xmlns:Container=\"http://ns.google.com/photos/1.0/container/\" \
         xmlns:Item=\"http://ns.google.com/photos/1.0/container/item/\" \
         xmlns:hdrgm=\"http://ns.adobe.com/hdr-gain-map/1.0/\" hdrgm:Version=\"1.0\">\
         <Container:Directory><rdf:Seq>\
         <rdf:li rdf:parseType=\"Resource\"><Container:Item Item:Semantic=\"Primary\" Item:Mime=\"image/jpeg\"/></rdf:li>\
         <rdf:li rdf:parseType=\"Resource\"><Container:Item Item:Semantic=\"GainMap\" Item:Mime=\"image/jpeg\" \
         Item:Length=\"{map_length}\"/></rdf:li>\
         </rdf:Seq></Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>"
    )
}

/// The map's own metadata, one description: the Ultra HDR attributes, the ISO headrooms, and
/// the Apple auxiliary type; one namespace serves both, which is what lets a single packet do.
fn map_xmp() -> String {
    let stops = format!("{:.6}", LIFT.log2());
    format!(
        "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
         <rdf:Description rdf:about=\"\" xmlns:apdi=\"http://ns.apple.com/pixeldatainfo/1.0/\" \
         xmlns:hdrgm=\"http://ns.adobe.com/hdr-gain-map/1.0/\" hdrgm:Version=\"1.0\" \
         hdrgm:GainMapMin=\"0.000000\" hdrgm:GainMapMax=\"{stops}\" hdrgm:Gamma=\"1.000000\" \
         hdrgm:OffsetSDR=\"0.000000\" hdrgm:OffsetHDR=\"0.000000\" \
         hdrgm:HDRCapacityMin=\"0.000000\" hdrgm:HDRCapacityMax=\"{stops}\" hdrgm:BaseRenditionIsHDR=\"False\" \
         hdrgm:BaseHeadroom=\"0.000000\" hdrgm:AlternateHeadroom=\"{stops}\" \
         apdi:NativeFormat=\"1278226488\" apdi:AuxiliaryImageType=\"urn:com:apple:photo:2020:aux:hdrgainmap\" \
         apdi:StoredFormat=\"1278226488\"/></rdf:RDF></x:xmpmeta>"
    )
}

/// A flat grey JPEG: every pixel at the top of the map's scale, so every pixel takes the full
/// lift. Grey, since a brightness ratio is one channel.
fn map_jpeg(width: usize, height: usize) -> Option<Vec<u8>> {
    let w = (width / SUBSAMPLING).max(1);
    let h = (height / SUBSAMPLING).max(1);
    jpeg::grey(w, h, 255, 0.9)
}

fn app1(xml: &str) -> Vec<u8> {
    let mut payload = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    payload.extend(xml.as_bytes());
    segment(0xE1, &payload)
}

fn segment(marker: u8, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0xFF, marker];
    out.extend(be16(payload.len() + 2));
    out.extend(payload);
    out
}

/// The MPF index (CIPA DC-007) for two images: the primary at offset 0, the map after it.
fn mpf(primary_size: usize, map_size: usize, map_offset: usize) -> Vec<u8> {
    let mut tiff: Vec<u8> = vec![0x4D, 0x4D, 0x00, 0x2A];
    tiff.extend(be32(8));
    tiff.extend(be16(3));
    tiff.extend(be16(0xB000).into_iter().chain(be16(7)).chain(be32(4)).chain([0x30, 0x31, 0x30, 0x30]));
    tiff.extend(be16(0xB001).into_iter().chain(be16(4)).chain(be32(1)).chain(be32(2)));
    tiff.extend(be16(0xB002).into_iter().chain(be16(7)).chain(be32(32)).chain(be32(8 + 2 + 3 * 12 + 4)));
    tiff.extend(be32(0));
    tiff.extend(be32(0x030000).into_iter().chain(be32(primary_size)).chain(be32(0)).chain(be16(0)).chain(be16(0)));
    tiff.extend(be32(0).into_iter().chain(be32(map_size)).chain(be32(map_offset)).chain(be16(0)).chain(be16(0)));
    let mut payload = b"MPF\0".to_vec();
    payload.extend(tiff);
    segment(0xE2, &payload)
}

/// Where the container's segments go: after the JFIF and Exif segments, before anything else.
pub fn insertion_point(jpeg: &[u8]) -> usize {
    let b = jpeg;
    let mut i = 2;
    while i + 4 < b.len() && b[i] == 0xFF && (0xE0..=0xEF).contains(&b[i + 1]) {
        let length = (b[i + 2] as usize) << 8 | b[i + 3] as usize;
        let exif = b[i + 1] == 0xE1 && i + 5 < b.len() && b[i + 4] == 0x45 && b[i + 5] == 0x78;
        if !(b[i + 1] == 0xE0 || exif) {
            break;
        }
        i += 2 + length;
    }
    i
}

fn with_xmp(jpeg: &[u8], xml: &str) -> Vec<u8> {
    let at = insertion_point(jpeg);
    let mut out = jpeg[..at].to_vec();
    out.extend(app1(xml));
    out.extend(&jpeg[at..]);
    out
}

fn be32(v: usize) -> [u8; 4] {
    (v as u32).to_be_bytes()
}

fn be16(v: usize) -> [u8; 2] {
    (v as u16).to_be_bytes()
}

// MARK: - Reading back

/// The first image's entropy-coded scan, from its SOS to its EOI.
pub fn scan(file: &[u8]) -> Option<&[u8]> {
    let start = (2..file.len().saturating_sub(1)).find(|&i| file[i] == 0xFF && file[i + 1] == 0xDA)?;
    let end = (start..file.len() - 1).find(|&i| file[i] == 0xFF && file[i + 1] == 0xD9)?;
    Some(&file[start..end])
}

/// The bytes of the second JPEG in a file, or nothing when there is only one. Read past the first
/// image's end: its metadata can carry an FF D8 by chance, the MPF index's own sizes included.
pub fn second_image(file: &[u8]) -> Option<&[u8]> {
    let start = (2..file.len().saturating_sub(1)).find(|&i| file[i] == 0xFF && file[i + 1] == 0xDA)?;
    let end = (start..file.len() - 1).find(|&i| file[i] == 0xFF && file[i + 1] == 0xD9)?;
    let first = (end + 2..file.len().saturating_sub(1)).find(|&i| file[i] == 0xFF && file[i + 1] == 0xD8)?;
    Some(&file[first..])
}
