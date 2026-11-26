//! A pan's render: the last render of the same recipe copied where both cover, only the rest evaluated.

use super::rendering::Last;
use super::*;

impl Graph {
    /// `region` of `image` evaluated into a new source from `before`, one texture of an earlier render of the same image:
    /// a pixel's value depends only on where it lies, so what both cover is copied and only the rest evaluated.
    pub fn materialise_over(&mut self, image: &Image, region: Rect, before: &Source) -> Result<Source, Error> {
        let image = &image.readable();
        let extent = image.extent();
        if region.is_empty() || !extent.contains(&region) {
            return Err(Error::RegionOutside { region, extent });
        }
        let [(held, texture)] = before.0.tiles.as_slice() else { panic!("an earlier render is one texture") };
        let kept = held.intersect(&region);
        if kept.is_empty() {
            return self.materialise_cached(image, region, None);
        }
        let strips = [
            Rect::from_corners(region.x, region.y, region.right(), kept.y),
            Rect::from_corners(region.x, kept.bottom(), region.right(), region.bottom()),
            Rect::from_corners(region.x, kept.y, kept.x, kept.bottom()),
            Rect::from_corners(kept.right(), kept.y, region.right(), kept.bottom()),
        ];
        let mut plans = Vec::new();
        for strip in strips.into_iter().filter(|s| !s.is_empty()) {
            let plan = Plan::new(image, strip, None)?;
            if !self.fits(&plan) {
                drop(plans);
                return self.materialise_cached(image, region, None);
            }
            plans.push((plan, strip));
        }
        let owned = self.pool.output(region.w, region.h);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.copy(&mut encoder, texture, *held, &owned, region, kept);
        let mut laid = Vec::with_capacity(plans.len());
        for (plan, strip) in plans {
            let staged = !matches!(plan.nodes[plan.last].kind, Kind::Source(_));
            let ready = self.lay(&mut encoder, plan);
            if !staged {
                let (from, rect) = ready.results[ready.last].clone().expect("a source is laid");
                self.copy(&mut encoder, &from, rect, &owned, region, strip);
                continue;
            }
            laid.push((ready, [strip.x - region.x, strip.y - region.y]));
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("strips"), timestamp_writes: None });
            for (ready, at) in laid {
                self.record(&mut pass, ready, None, Last::Sheet(&owned, at));
            }
        }
        self.submit(encoder);
        self.ended();
        let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
        Ok(Source(Arc::new(SourceInner { id, extent: region, tiles: vec![(region, owned)] })))
    }
}
