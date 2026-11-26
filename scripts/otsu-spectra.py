# The spectra table model S reads, Otsu 2018 in place of spektrafilm's: the app's 192 × 192 chromaticity square (`square_of_xy`),
# each cell a reflectance Otsu 2018 rebuilds at the cell's chromaticity under D55 (Y = 0.2), lit by D55, scaled so its X + Y + Z
# is the spektrafilm table's median (21.4, a constant every film's balance on its grey takes away). float16, 192 × 192 × 81, 380 to
# 780 nm by 5 nm, the same layout as `irradiance_xy_tc.npy`. A cell Otsu cannot rebuild keeps a flat reflectance.
# Run: PYTHONPATH=<numpy, colour-science 0.4.x> python3 scripts/otsu-spectra.py <out.npy>
import sys, warnings; warnings.filterwarnings('ignore')
import numpy as np, colour
W=np.arange(380,781,5.0); N=192
CMF=colour.MSDS_CMFS['CIE 1931 2 Degree Standard Observer']; D55=colour.SDS_ILLUMINANTS['D55']
cmf=np.stack([np.interp(W,CMF.wavelengths,CMF.values[:,k]) for k in range(3)],1); d55=np.interp(W,D55.wavelengths,D55.values)
def xy_of_square(t): r=np.sqrt(max(t[0],0)); return np.array([1-r,t[1]*r])
SUM=21.4
if __name__=='__main__':
    out=np.zeros((N,N,len(W))); failed=0
    for i in range(N):
        for j in range(N):
            xy=np.clip(xy_of_square(np.array([i/(N-1),j/(N-1)])),1e-4,None)
            if xy.sum()>=1: xy=xy/(xy.sum()+1e-3)
            XYZ=np.array([xy[0]/xy[1],1,(1-xy.sum())/xy[1]])*0.2
            try: sd=colour.recovery.XYZ_to_sd_Otsu2018(XYZ,CMF,D55); R=np.clip(np.interp(W,sd.wavelengths,sd.values),0,1.5)
            except Exception: R=np.full(len(W),0.2); failed+=1
            light=R*d55; out[i,j]=light*SUM/(light@cmf).sum()
    np.save(sys.argv[1],out.astype(np.float16)); print('cases',N*N,'non reconstruites',failed)
