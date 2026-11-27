# derive-couplers.py <stocks dir> <out.json> [--write]: each colour negative's inhibition between layers (model S), from its own
# data sheet only. The layers exposed by the sheet's sensitivities to measured spectra (derive-couplers-training.npz: ColorChecker
# BabelColor Average 24 and CIE D skies at 7500, 10000, 15000 and 25000 K) under the film's light (D55, or 3200 K for a
# tungsten film); developed on the sheet's Status M curves with K (six terms between layers, 0 to 1.5, a neutral kept on
# the sheet); each channel read between D-min and a neutral at the sensor's saturation; the pure inversion. K minimises the
# Oklab (a, b) error of the final positive against the scene's colour at the model's own lightness (the scene's light times
# the ratio the model gives a neutral of the same luminance). With --write, each file's `film.couplers` takes its K: a start for a
# new colour negative with no reference, the shipped ones fitted otherwise (`crates/oni-settings/stocks/README.md`).
import sys, json, glob, os, warnings; warnings.filterwarnings('ignore')
import numpy as np, colour
from scipy.optimize import least_squares
HERE=os.path.dirname(os.path.abspath(__file__)); W=np.arange(380,781,5.0)
CMF=np.stack([np.interp(W,colour.MSDS_CMFS['CIE 1931 2 Degree Standard Observer'].wavelengths,colour.MSDS_CMFS['CIE 1931 2 Degree Standard Observer'].values[:,k]) for k in range(3)],1)
D55=np.interp(W,colour.SDS_ILLUMINANTS['D55'].wavelengths,colour.SDS_ILLUMINANTS['D55'].values)
def planck(k): return 1/((W*1e-9)**5*(np.exp(1.438776877e-2/(W*1e-9*k))-1))
SAT=1.394; GREY=0.184; E_SAT=np.log10(SAT/GREY); LUMA=np.array([0.2627,0.6780,0.0593]); NEUTRAL=6                # the chart's last six patches are its greys
def xyz_xy(xy): return np.array([xy[0]/xy[1],1,(1-xy[0]-xy[1])/xy[1]])
P=np.array([xyz_xy(p) for p in ([0.708,0.292],[0.170,0.797],[0.131,0.046])]).T; D65=xyz_xy([0.3127,0.3290]); REC=P*(np.linalg.inv(P)@D65)
CAT16=np.array([[0.401288,0.650173,-0.051461],[-0.250268,1.204414,0.045854],[-0.002079,0.048952,0.953127]])
def adapt(a,b): return np.linalg.inv(CAT16)@np.diag((CAT16@b)/(CAT16@a))@CAT16
R2S=np.array([[1.6605,-0.5876,-0.0728],[-0.1246,1.1329,-0.0083],[-0.0182,-0.1006,1.1187]])
def eotf(c): return np.where(c<=0.04045,c/12.92,((np.maximum(c,0)+0.055)/1.055)**2.4)
def oklab(lin_srgb):
    lin=np.clip(lin_srgb,0,1); M1=np.array([[0.4122214708,0.5363325363,0.0514459929],[0.2119034982,0.6806995451,0.1073969566],[0.0883024619,0.2817188376,0.6299787005]])
    M2=np.array([[0.2104542553,0.7936177850,-0.0040720468],[1.9779984951,-2.4285922050,0.4505937099],[0.0259040371,0.7827717662,-0.8086757660]]); return np.cbrt(lin@M1.T)@M2.T
def held(a):
    a=np.array([np.nan if v is None else v for v in a],float); i=np.arange(len(a)); ok=~np.isnan(a); return np.interp(i,i[ok],a[ok])
class Film:
    def __init__(s,entry):
        d=entry['film']['profile']['data']; s.LE=np.array(d['log_exposure'],float); S=np.array(d['sheet_density_curves'],float); s.S=S-S.min(0)
        s.SENS=10**np.stack([held([r[k] for r in d['log_sensitivity']]) for k in range(3)],1); s.light=planck(3200) if entry['film'].get('balance')=='tungsten' else D55
    def logh(s,R): return np.log10(np.maximum((R*s.light[None])@s.SENS/(s.light@s.SENS)/GREY,1e-10))
    def model(s,K):
        before=[s.LE-s.S@K[:,c] for c in range(3)]
        if not all(np.all(np.diff(b)>0) for b in before): return None
        def dens(h):
            first=np.stack([np.interp(h[:,c],s.LE,s.S[:,c]) for c in range(3)],1); inh=h-first@K; return np.stack([np.interp(inh[:,c],before[c],s.S[:,c]) for c in range(3)],1)
        white=dens(np.full((1,3),E_SAT))[0]; return lambda h: eotf(np.clip(dens(h)/white,0,1))@R2S.T        # linear sRGB out
def scene_lin(film,R):
    xyz=(R*film.light[None])@CMF/(film.light@CMF[:,1]); w=(film.light@CMF)/(film.light@CMF[:,1]); return xyz@(np.linalg.inv(REC)@adapt(w,D65)).T@R2S.T
def residuals(film,R,K):
    out=film.model(K)
    if out is None: return None
    lin=scene_lin(film,R); Y=lin@np.array([0.2126,0.7152,0.0722]); yo=out(np.repeat(np.log10(np.maximum(Y,1e-6)/GREY)[:,None],3,1))@np.array([0.2126,0.7152,0.0722])
    ref=oklab(lin*(yo/np.maximum(Y,1e-9))[:,None]); o=oklab(out(film.logh(R))); return np.r_[o[:,1]-ref[:,1],o[:,2]-ref[:,2]]
def derive(entry,R):
    film=Film(entry); off=~np.eye(3,dtype=bool)
    def res(v):
        K=np.zeros((3,3)); K[off]=v; r=residuals(film,R,K); return np.full(2*len(R),1.0) if r is None else r
    best=None
    for v0 in (np.zeros(6),np.full(6,0.3)):
        q=least_squares(res,v0,bounds=(0,1.5),diff_step=1e-3)
        if best is None or q.cost<best.cost: best=q
    K=np.zeros((3,3)); K[off]=best.x; return K,float(np.sqrt(np.mean(res(best.x)**2)))*100
if __name__=='__main__':
    tr=np.load(os.path.join(HERE,'derive-couplers-training.npz')); R=tr['R']; chroma=[i for i in range(len(R)) if not (18<=i<24)]   # the ColorChecker's greys out
    out={}
    for f in sorted(glob.glob(os.path.join(sys.argv[1],'*.json'))):
        e=json.load(open(f)); d=(e.get('film') or {}).get('profile') or {}
        if e.get('type') not in ('colour-negative','cine-negative') or not isinstance(d,dict) or 'sheet_density_curves' not in d.get('data',{}): continue
        K,err=derive(e,R[chroma]); out[e['key']]={'couplers':np.round(K,4).tolist(),'error_oklab_x100':round(err,3),'bound':bool(np.any(np.isclose(K[~np.eye(3,dtype=bool)],1.5,atol=1e-3)))}
        print('%-34s K %s erreur %.2f%s'%(e['key'],np.round(K[~np.eye(3,dtype=bool)],3),err,' BORNE' if out[e['key']]['bound'] else ''),flush=True)
        if '--write' in sys.argv: e['film']['couplers']=out[e['key']]['couplers']; json.dump(e,open(f,'w'),indent=1,ensure_ascii=False)
    json.dump(out,open(sys.argv[2],'w'),indent=1)
