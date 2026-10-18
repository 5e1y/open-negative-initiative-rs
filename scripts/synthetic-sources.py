"""Writes the synthetic sources the decode parity check reads: a linear DNG (LinearRaw, ColorMatrix1)
and 16-bit PNG passes, all made of smooth ramps so a resampling difference shows as a small gap."""
import math, struct, sys, zlib, os

def ramp(w, h, phase):
    for y in range(h):
        for x in range(w):
            r = 0.5 + 0.45 * math.sin(x / w * 6.3 + phase)
            g = 0.5 + 0.45 * math.cos(y / h * 4.1 + 2 * phase)
            b = (x + y) / (w + h)
            yield (r, g, b)

def png16(path, w, h, phase):
    rows = b''
    px = list(ramp(w, h, phase))
    for y in range(h):
        rows += b'\x00' + b''.join(struct.pack('>HHH', *(int(v * 65535) for v in px[y * w + x])) for x in range(w))
    def chunk(t, d):
        return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    data = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 16, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(rows)) + chunk(b'IEND', b'')
    open(path, 'wb').write(data)

def dng(path, w, h):
    samples = b''.join(struct.pack('<HHH', *(int(v * 60000) for v in p)) for p in ramp(w, h, 0.7))
    cm = [3.2406, -1.5372, -0.4986, -0.9689, 1.8758, 0.0415, 0.0557, -0.2040, 1.0570]
    entries = []
    extra = b''
    base = 8
    n_tags = 15
    ifd_size = 2 + n_tags * 12 + 4
    data_at = base + ifd_size
    def put(blob):
        nonlocal extra
        off = data_at + len(extra)
        extra += blob
        if len(extra) % 2:
            extra += b'\x00'
        return off
    bps = put(struct.pack('<HHH', 16, 16, 16))
    cm_off = put(b''.join(struct.pack('<ii', int(round(v * 10000)), 10000) for v in cm))
    strip = put(samples)
    tags = [
        (254, 4, 1, 0), (256, 4, 1, w), (257, 4, 1, h), (258, 3, 3, bps), (259, 3, 1, 1), (262, 3, 1, 34892),
        (273, 4, 1, strip), (277, 3, 1, 3), (278, 4, 1, h), (279, 4, 1, len(samples)), (284, 3, 1, 1),
        (50706, 1, 4, int.from_bytes(bytes([1, 4, 0, 0]), 'little')), (50717, 4, 1, 60000), (50721, 10, 9, cm_off), (50778, 3, 1, 21),
    ]
    assert len(tags) == n_tags
    ifd = struct.pack('<H', n_tags) + b''.join(struct.pack('<HHII', t, ty, c, v) for t, ty, c, v in sorted(tags)) + struct.pack('<I', 0)
    open(path, 'wb').write(b'II*\x00' + struct.pack('<I', base) + ifd + extra)

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
dng(os.path.join(out, 'linear.dng'), 300, 200)
for i, name in enumerate(['pass-r.png', 'pass-g.png', 'pass-b.png']):
    png16(os.path.join(out, name), 300, 200, i * 1.3)
png16(os.path.join(out, 'plain16.png'), 300, 200, 0.4)
print('written')
