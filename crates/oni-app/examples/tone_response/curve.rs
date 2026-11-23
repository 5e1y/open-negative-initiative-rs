//! A relative density curve along log exposure from the grey: a stock's, or a slide's read from a file.

/// A relative curve `m(x)`, density above its least along log exposure from the grey, read from a JSON file.
pub struct Curve {
    pub x: Vec<f32>,
    pub m: Vec<f32>,
}

impl Curve {
    /// The mean of a stock's three printed curves above its least, its `profile` as its file holds it: the density a
    /// scan reads, its dyes' amounts mixed.
    pub fn of_stock(profile: &str) -> Curve {
        let json: serde_json::Value = serde_json::from_str(profile).expect("a profile");
        let numbers = |v: &serde_json::Value| v.as_array().expect("an array").iter().map(|x| x.as_f64().expect("a number")).collect::<Vec<f64>>();
        let rows = json["data"]["sheet_density_curves"].as_array().expect("the sheet's curves");
        let mean: Vec<f32> = rows.iter().map(|r| (numbers(r).iter().sum::<f64>() / 3.0) as f32).collect();
        let least = mean.iter().copied().fold(f32::MAX, f32::min);
        Curve { x: numbers(&json["data"]["log_exposure"]).iter().map(|v| *v as f32).collect(), m: mean.iter().map(|v| v - least).collect() }
    }

    pub fn read(path: &str) -> Curve {
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let get = |k: &str| v[k].as_array().unwrap().iter().map(|n| n.as_f64().unwrap() as f32).collect();
        Curve { x: get("x"), m: get("m") }
    }

    pub fn at(&self, x: f32) -> f32 {
        let n = self.x.len();
        if x <= self.x[0] {
            return self.m[0];
        }
        if x >= self.x[n - 1] {
            return self.m[n - 1];
        }
        let hi = self.x.partition_point(|v| *v <= x).min(n - 1);
        let t = (x - self.x[hi - 1]) / (self.x[hi] - self.x[hi - 1]);
        self.m[hi - 1] + t * (self.m[hi] - self.m[hi - 1])
    }
}
