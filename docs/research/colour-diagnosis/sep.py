import numpy as np
ROWS = {"kodakC41": [[1.0000, -0.2379, -0.0092], [-0.0346, 1.0000, -0.0136], [-0.0327, -0.0790, 1.0000]],
        "genericC41": [[1.0000, -0.2706, -0.0076], [-0.0517, 1.0000, -0.0137], [-0.0228, -0.0723, 1.0000]],
        "e6": [[1.0000, -0.3584, 0.0067], [-0.3109, 1.0000, -0.0588], [-0.0269, -0.2385, 1.0000]]}
def separation(family, s):
    full = np.array(ROWS[family]); full = full / full.sum(1, keepdims=True)
    m = np.eye(3) + s * (full - np.eye(3))
    u = np.array([1, -1, 0]) / np.sqrt(2); v = np.array([1, 1, -2]) / np.sqrt(6)
    a, b, c, d = u @ m @ u, u @ m @ v, v @ m @ u, v @ m @ v
    g = np.sqrt(abs(a * d - b * c))
    return m / g + (1 / 3) * (1 - 1 / g)
