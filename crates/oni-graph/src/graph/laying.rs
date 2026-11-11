//! A batch laid on an image: each step of its members one pass for them all, then their stages over
//! their own areas and back onto the image, a layer at a time where areas meet.

use super::batch::{BatchInner, LaidInner, Step};
use super::instanced::{Cell, Instance};
use super::rendering::Put;
use super::*;

/// The pixels the members of one turn hold at their widest step; past it they are laid in turns, each
/// turn's textures handed back, so a thousand large members never hold a thousand textures at once.
const TURN_PIXELS: u64 = 4 << 20;
/// The members of one turn at most: its tables then stay well inside the uniform ring.
const TURN_MEMBERS: usize = 2048;
/// The width a turn's members are packed across, in rows.
const SHEET_WIDTH: u32 = 2048;

/// Rectangles of `sizes` packed in rows on one sheet: each one's corner, and the sheet's size.
fn packed(sizes: impl Iterator<Item = (u32, u32)>) -> (Vec<[i32; 2]>, (u32, u32)) {
    let (mut x, mut y, mut row, mut wide) = (0u32, 0u32, 0u32, 1u32);
    let places = sizes
        .map(|(w, h)| {
            if x > 0 && x + w > SHEET_WIDTH {
                (x, y, row) = (0, y + row, 0);
            }
            let at = [x as i32, y as i32];
            (x, row, wide) = (x + w, row.max(h), wide.max(x + w));
            at
        })
        .collect();
    (places, (wide, (y + row).max(1)))
}

/// What each of `members` must hold at each level of `batch`, its source first, for `top` at its last.
fn needs(batch: &BatchInner, members: &[usize], top: Vec<Rect>) -> Vec<Vec<Rect>> {
    let mut levels = vec![top];
    for (i, step) in batch.steps.iter().enumerate().rev() {
        let above = levels.last().expect("the level above");
        let below = members.iter().zip(above).map(|(&k, need)| match step {
            Step::Stage(stages) => stages.get(k).map_or(*need, |s| s.input_region(*need).nearest_in(&batch.extents[i][k])),
            Step::Moved(by) => Rect::new(need.x - by[k][0], need.y - by[k][1], need.w, need.h),
        });
        levels.push(below.collect());
    }
    levels.reverse();
    levels
}

impl Graph {
    /// The members of `laid` that `shown` names, each over its part of its area, onto the texture
    /// `onto`, which holds rectangle `onto.1` of an image of `extent`.
    pub(super) fn lay_on(&mut self, pass: &mut wgpu::ComputePass<'_>, laid: &Laid, shown: &[(usize, Rect)], onto: (&FloatTexture, Rect), extent: Rect) {
        let l = &*laid.0;
        let members: Vec<usize> = shown.iter().map(|s| s.0).collect();
        let stage = |k: usize| l.stages.get(k).expect("a laid member has its stage");
        let all: Vec<Vec<Vec<Rect>>> = (l.aux.iter().enumerate())
            .map(|(j, b)| needs(&b.0, &members, shown.iter().map(|&(k, hit)| stage(k).aux_region(j, hit).nearest_in(&b.extents()[k])).collect()))
            .collect();
        let weight = |i: usize| all.iter().flatten().map(|level| level[i].area()).max().unwrap_or(0).max(shown[i].1.area());
        let mut start = 0;
        while start < shown.len() {
            let (mut end, mut pixels) = (start + 1, weight(start));
            while end < shown.len() && end - start < TURN_MEMBERS && pixels + weight(end) <= TURN_PIXELS {
                pixels += weight(end);
                end += 1;
            }
            let turn: Vec<Vec<Vec<Rect>>> = all.iter().map(|levels| levels.iter().map(|level| level[start..end].to_vec()).collect()).collect();
            self.lay_turn(pass, l, &shown[start..end], &turn, onto, extent);
            start = end;
        }
    }

    fn lay_turn(&mut self, pass: &mut wgpu::ComputePass<'_>, l: &LaidInner, shown: &[(usize, Rect)], needs: &[Vec<Vec<Rect>>], onto: (&FloatTexture, Rect), extent: Rect) {
        let members: Vec<usize> = shown.iter().map(|s| s.0).collect();
        let mut textures = vec![onto.0.clone()];
        let aux: Vec<Vec<Cell>> = l.aux.iter().zip(needs).map(|(b, levels)| self.cells(pass, &b.0, &members, levels, &mut textures)).collect();
        let image = Cell { texture: 0, rect: onto.1, at: [0, 0], extent };
        // A layer's members lie apart, so each reads the image as the layers under it left it.
        let mut order: Vec<usize> = (0..shown.len()).collect();
        order.sort_by_key(|&i| l.layers[shown[i].0]);
        for layer in order.chunk_by(|&a, &b| l.layers[shown[a].0] == l.layers[shown[b].0]) {
            let (places, size) = packed(layer.iter().map(|&i| (shown[i].1.w, shown[i].1.h)));
            let parts = self.pool.acquire(size.0, size.1);
            let instances: Vec<Instance> = (layer.iter().zip(&places))
                .map(|(&i, &at)| Instance { stage: l.stages.get(shown[i].0).expect("its stage"), input: image, aux: aux.iter().map(|cells| cells[i]).collect(), out: shown[i].1, out_at: at })
                .collect();
            self.dispatch_instanced(pass, &instances, &textures, &parts);
            textures.push(parts);
            let held = textures.len() - 1;
            let puts: Vec<Instance> = (layer.iter().zip(&places))
                .map(|(&i, &at)| {
                    let hit = shown[i].1;
                    Instance { stage: &Put, input: Cell { texture: held, rect: hit, at, extent: hit }, aux: Vec::new(), out: hit, out_at: [hit.x - onto.1.x, hit.y - onto.1.y] }
                })
                .collect();
            self.dispatch_instanced(pass, &puts, &textures, onto.0);
        }
        for texture in textures.into_iter().skip(1) {
            self.pool.release(texture);
        }
    }

    /// Each of `members` of `batch` evaluated over its needs, a pass a step for them all: where each
    /// one's result lies, among `textures`, which gains the sheets the steps were written on.
    fn cells(&mut self, pass: &mut wgpu::ComputePass<'_>, batch: &BatchInner, members: &[usize], needs: &[Vec<Rect>], textures: &mut Vec<FloatTexture>) -> Vec<Cell> {
        let (tile, texture) = &batch.source.0.tiles[0];
        textures.push(texture.clone());
        let source = textures.len() - 1;
        let from = |k: usize| Cell { texture: source, rect: batch.extents[0][k], at: [batch.at[k].x - tile.x, batch.at[k].y - tile.y], extent: batch.extents[0][k] };
        let mut cells: Vec<Cell> = members.iter().map(|&k| from(k)).collect();
        for (i, step) in batch.steps.iter().enumerate() {
            let stages = match step {
                Step::Stage(stages) => stages,
                Step::Moved(by) => {
                    let moved = |r: Rect, by: [i32; 2]| Rect::new(r.x + by[0], r.y + by[1], r.w, r.h);
                    for (cell, &k) in cells.iter_mut().zip(members) {
                        (cell.rect, cell.extent) = (moved(cell.rect, by[k]), moved(cell.extent, by[k]));
                    }
                    continue;
                }
            };
            // A member given no stage keeps the cell it has.
            let staged: Vec<usize> = (0..members.len()).filter(|&j| stages.get(members[j]).is_some()).collect();
            if staged.is_empty() {
                continue;
            }
            let out = &needs[i + 1];
            let (places, size) = packed(staged.iter().map(|&j| (out[j].w, out[j].h)));
            let sheet = self.pool.acquire(size.0, size.1);
            let instances: Vec<Instance> = (staged.iter().zip(&places))
                .map(|(&j, &at)| Instance { stage: stages.get(members[j]).expect("its stage"), input: cells[j], aux: Vec::new(), out: out[j], out_at: at })
                .collect();
            self.dispatch_instanced(pass, &instances, textures, &sheet);
            textures.push(sheet);
            for (&j, &at) in staged.iter().zip(&places) {
                cells[j] = Cell { texture: textures.len() - 1, rect: out[j], at, extent: batch.extents[i + 1][members[j]] };
            }
        }
        cells
    }
}
