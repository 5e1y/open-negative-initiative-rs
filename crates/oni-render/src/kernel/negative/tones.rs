//! The tones Lighting lays on the negative's density, each read on its tone share: Exposure held at the window's ends,
//! then the points, the zones and the mids.

use super::*;

impl Negative {
    /// Exposure on `density`'s tone share in the development's sense, its odds taken by `odds`: the window's black and
    /// white stay, the tones by each scale about it as by a gain, monotone however far; past the ends, none.
    pub fn exposed(&self, density: V3) -> V3 {
        if self.odds == 0.0 {
            return density;
        }
        let k = self.odds.exp();
        map(density, |c, d| {
            let u = self.origin + self.sense * d / self.span[c];
            if !(u > 0.0 && u < 1.0) {
                return d;
            }
            self.sense * (k * u / (1.0 + (k - 1.0) * u) - self.origin) * self.span[c]
        })
    }

    /// Whether the points, zones or mids lay any density.
    pub fn tones(&self) -> bool {
        self.points_black != [0.0; 3] || self.points_white != [0.0; 3] || self.zones != [0.0; 2] || self.mids != [0.0; 3]
    }

    /// The density above the bare film Lighting's tones leave, each read on its tone share.
    pub fn toned(&self, density: V3) -> V3 {
        if !self.tones() {
            return density;
        }
        let u = map(density, |c, d| (self.origin + d / self.span[c] - self.points_black[c]) / (1.0 - self.points_black[c] - self.points_white[c]));
        let tone = (u[0] + u[1] + u[2]) / 3.0;
        let lift = if self.zones != [0.0; 2] && (0.0..=1.0).contains(&tone) {
            quartered(quartered(tone, self.zones[0], false), self.zones[1], true) - tone
        } else {
            0.0
        };
        map(u, |c, v| (midded(v + lift, self.mids[c]) - self.origin) * self.span[c])
    }
}
