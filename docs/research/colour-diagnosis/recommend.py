"""The two recommended prints, on any reading of the negative: (a) the paper itself after the printing-density
matrix; (b) ONI's print with greys balanced on the whole ramp, the paper's tone and the effective matrix."""
from steps import *
g = cmy(np.array([[0.184] * 3]))[0]
inv_paper = lambda y: np.stack([np.interp(np.log(np.maximum(y[..., k], 1e-9)), np.log(YC), STOPS) for k in range(3)], -1)
LR = np.log10(spk_ramp)

def J(f, at=g, h=0.01):
    return np.array([(f(np.array([at + h * np.eye(3)[k]]))[0] - f(np.array([at - h * np.eye(3)[k]]))[0]) / (2 * h) for k in range(3)]).T

def matrices(reader):
    ramp = reader(c_ramp)
    Jr = J(lambda x: equivalent(reader(x), ramp))
    Js = J(lambda x: equivalent(read_spk(x), spk_ramp))
    Jc = J(lambda x: inv_paper(spk.run(cpipe, x, inject="cmy_film")))
    Mread = Js @ np.linalg.inv(Jr); Mnet = Jc @ np.linalg.inv(Jr)
    return ramp, Mread / Mread.sum(1, keepdims=True), Mnet / Mnet.sum(1, keepdims=True)

def physical(light, ramp, Mread):
    e = mat(Mread, equivalent(light, ramp))
    q = np.stack([np.interp(e[..., k], STOPS, LR[:, k]) for k in range(3)], -1)
    return spk.run(cpipe, q, inject="log_e_print")

def oni_model(light, ramp, Mnet):
    return tone_paper(mat(Mnet, equivalent(light, ramp)))
