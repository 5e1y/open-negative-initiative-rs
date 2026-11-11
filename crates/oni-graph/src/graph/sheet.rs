//! Many small renders laid on one sheet in one submission.

use super::rendering::Last;
use super::*;

impl Graph {
    /// Each image evaluated whole and laid with its corner at its place on one clear sheet of `size`,
    /// all in one submission: many small pictures pay one hand-over to the GPU and no readback.
    pub fn laid_out(&mut self, images: &[(&Image, (i32, i32))], size: (u32, u32)) -> Result<Source, Error> {
        if size.0 > self.max_side || size.1 > self.max_side {
            return Err(Error::TooLarge);
        }
        let sheet_rect = Rect::new(0, 0, size.0, size.1);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        // An output no source holds any more, cleared, where the device clears in place; else a fresh one.
        let sheet = if self.device.features().contains(wgpu::Features::CLEAR_TEXTURE) {
            let sheet = self.pool.output(size.0, size.1);
            encoder.clear_texture(sheet.texture(), &Default::default());
            sheet
        } else {
            self.pool.create_owned(size.0, size.1, Precision::Full)
        };
        // Every image's sources laid first, then all their stages in one compute pass, each last stage
        // writing at its place on the sheet: one command buffer for the whole sheet.
        let readable: Vec<Image> = images.iter().map(|(image, _)| image.readable()).collect();
        let mut laid = Vec::with_capacity(images.len());
        for (image, (_, (x, y))) in readable.iter().zip(images) {
            let region = image.extent();
            let at = Rect::new(*x, *y, region.w, region.h);
            if !sheet_rect.contains(&at) {
                return Err(Error::RegionOutside { region: at, extent: sheet_rect });
            }
            let plan = Plan::new(image, region, None)?;
            if !self.fits(&plan) {
                return Err(Error::TooLarge);
            }
            let staged = !matches!(plan.nodes[plan.last].kind, Kind::Source(_));
            let plan = self.lay(&mut encoder, plan);
            if !staged {
                let (texture, rect) = plan.results[plan.last].clone().expect("a source is laid");
                self.copy(&mut encoder, &texture, Rect::new(rect.x - region.x + x, rect.y - region.y + y, rect.w, rect.h), &sheet, sheet_rect, at);
                continue;
            }
            laid.push((plan, [*x, *y]));
        }
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("sheet"), timestamp_writes: None });
            for (plan, at) in laid {
                self.record(&mut pass, plan, None, Last::Sheet(&sheet, at));
            }
        }
        self.submit(encoder);
        self.ended();
        let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
        Ok(Source(Arc::new(SourceInner { id, extent: sheet_rect, tiles: vec![(sheet_rect, sheet)] })))
    }

    /// Each source copied whole with its corner at its place on one sheet of `size`, in one submission:
    /// what a batch reads its members off. Panics on a source in half floats, which copies to no full one.
    pub fn gathered(&mut self, sources: &[(&Source, (i32, i32))], size: (u32, u32)) -> Result<Source, Error> {
        if size.0 > self.max_side || size.1 > self.max_side {
            return Err(Error::TooLarge);
        }
        let sheet_rect = Rect::new(0, 0, size.0, size.1);
        let sheet = self.pool.create_owned(size.0, size.1, Precision::Full);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        for (source, (x, y)) in sources {
            let e = source.extent();
            let at = Rect::new(*x, *y, e.w, e.h);
            assert!(source.precision() == Precision::Full, "a gathered source holds full floats");
            if !sheet_rect.contains(&at) {
                return Err(Error::RegionOutside { region: at, extent: sheet_rect });
            }
            for (rect, tile) in &source.0.tiles {
                let moved = Rect::new(rect.x - e.x + x, rect.y - e.y + y, rect.w, rect.h);
                self.copy(&mut encoder, tile, moved, &sheet, sheet_rect, moved);
            }
        }
        self.submit(encoder);
        let id = SOURCE_IDS.fetch_add(1, Ordering::Relaxed);
        Ok(Source(Arc::new(SourceInner { id, extent: sheet_rect, tiles: vec![(sheet_rect, sheet)] })))
    }
}
