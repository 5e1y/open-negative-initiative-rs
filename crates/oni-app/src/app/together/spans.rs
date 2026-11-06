//! A slider's readout over frames shown together: the open frame's value where every frame reads
//! the same, else the span from the lowest to the highest, which a move shifts whole.

use super::*;

impl Ctl {
    /// Each slider's span over the frames shown together read again; the sliders whose span moved.
    fn respread(&self) -> Vec<Field> {
        let spans: Vec<(f32, f32)> = {
            let s = self.state.borrow();
            let live = s.together.as_ref().and(s.store.editor_ref()).map(|e| e.together_live()).unwrap_or_default();
            let open = s.store.settings();
            let span = |f: Field| {
                let at = f.knob().get(open);
                live.iter().map(|(_, l)| f.knob().get(l) - at).fold((0.0f32, 0.0f32), |(lo, hi), d| (lo.min(d), hi.max(d)))
            };
            Field::ALL.into_iter().map(span).collect()
        };
        let mut s = self.state.borrow_mut();
        let Some(g) = s.together.as_mut() else { return Vec::new() };
        let moved = Field::ALL.into_iter().zip(&spans).enumerate().filter(|(i, (_, span))| g.spans.get(*i) != Some(*span)).map(|(_, (f, _))| f).collect();
        g.spans = spans;
        moved
    }

    /// `f`'s readout at `v` on the open frame, the span of the frames shown with it moved along.
    pub(in crate::app) fn readout_at(&self, f: Field, v: f32) -> String {
        let span = self.state.borrow().together.as_ref().and_then(|g| g.spans.get(f.index() as usize).copied());
        span.map_or_else(|| f.readout(v), |(lo, hi)| f.span_readout(v + lo, v + hi))
    }

    /// The readouts whose span moved written again, a slider under the hand writing its own.
    pub(in crate::app) fn publish_spans(&self) {
        let moved = self.respread();
        self.publish_readouts(moved);
    }

    /// These sliders' readouts written as the frames shown read now, a slider under the hand writing its own.
    pub(in crate::app) fn publish_readouts(&self, fields: impl IntoIterator<Item = Field>) {
        let Some(w) = self.window() else { return };
        let g = w.global::<StoreGlobal>();
        for f in fields {
            let (v, held) = {
                let s = self.state.borrow();
                (s.store.get(f), s.slides[f.index() as usize].live.is_some())
            };
            let text: SharedString = self.readout_at(f, v).into();
            set_value(&g, f, text.clone());
            if !held {
                set_readout(&g, f, text);
            }
        }
    }
}
