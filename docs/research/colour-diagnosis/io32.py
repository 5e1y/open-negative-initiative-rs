import numpy as np
def read(p):
    with open(p, "rb") as f:
        w, h = map(int, f.readline().split())
        return np.frombuffer(f.read(), dtype="<f4").reshape(h, w, 3).astype(np.float64)
def write(p, a):
    a = np.asarray(a, dtype="<f4"); h, w = a.shape[:2]
    with open(p, "wb") as f:
        f.write(f"{w} {h}\n".encode()); f.write(a.reshape(-1).tobytes())
