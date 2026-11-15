//! What the Kit adds to an export's EXIF, from the piece a typed name stands for.

use oni_tldb::{Kept, Kind};

/// An export's EXIF completed from the Kit: the brand, a lens's focal lengths and apertures, and a
/// fixed-lens body's own lens where none is named.
pub fn complete(kept: &Kept, m: &mut oni_settings::ExifMetadata) {
    let body = kept.named(Kind::Camera, &m.camera);
    if let Some(body) = body {
        m.camera_make = body.brand.clone().unwrap_or_default();
    }
    let lens = if m.lens.trim().is_empty() {
        body.and_then(|b| b.built_in_lens.as_deref()).inspect(|own| m.lens = own.name.clone())
    } else {
        kept.named(Kind::Lens, &m.lens)
    };
    if let Some(lens) = lens {
        m.lens_make = lens.brand.clone().unwrap_or_default();
        m.lens_specification = lens.focal_length_mm.map(|f| {
            let a = lens.max_aperture;
            oni_settings::LensSpecification::new(f.min, f.max, a.map(|a| a.at_min_focal), a.map(|a| a.at_max_focal))
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oni_settings::{ExifMetadata, LensSpecification};
    use oni_tldb::{Aperture, Piece, Span};

    fn piece(kind: Kind, id: u64, brand: Option<&str>, model: &str) -> Piece {
        let name = brand.map_or(model.to_string(), |b| format!("{b} {model}"));
        let brand = brand.map(str::to_string);
        Piece { kind, id, name, brand, model: model.into(), url: String::new(), mount: None, mounts: None, year_introduced: None, focal_length_mm: None, max_aperture: None, built_in_lens: None }
    }

    fn kept() -> Kept {
        let mut zoom = piece(Kind::Lens, 1, Some("Canon"), "EF 70-300mm f/4-5.6 IS USM");
        zoom.focal_length_mm = Some(Span { min: 70.0, max: 300.0 });
        zoom.max_aperture = Some(Aperture { at_min_focal: 4.0, at_max_focal: 5.6 });
        let mut sonnar = piece(Kind::Lens, 2, Some("Carl Zeiss"), "Sonnar 38mm f/2.8");
        sonnar.focal_length_mm = Some(Span { min: 38.0, max: 38.0 });
        let mut t2 = piece(Kind::Camera, 3, Some("Contax"), "T2");
        t2.built_in_lens = Some(Box::new(sonnar));
        Kept { account: None, items: vec![piece(Kind::Camera, 1, Some("Canon"), "AE-1 Program"), zoom, t2], etag: None }
    }

    #[test]
    fn a_kit_piece_named_on_a_line_gives_its_brand_and_specification() {
        let mut m = ExifMetadata { camera: "canon ae-1 program".into(), lens: "EF 70-300mm f/4-5.6 IS USM".into(), ..ExifMetadata::default() };
        complete(&kept(), &mut m);
        assert_eq!((m.camera_make.as_str(), m.lens_make.as_str()), ("Canon", "Canon"), "by its name or its model, its case aside");
        assert_eq!(m.lens_specification, Some(LensSpecification::new(70.0, 300.0, Some(4.0), Some(5.6))));
        // Twin: a name the Kit does not hold adds nothing.
        let mut other = ExifMetadata { camera: "Leica M6".into(), lens: "Summicron 50".into(), ..ExifMetadata::default() };
        complete(&kept(), &mut other);
        assert_eq!(other, ExifMetadata { camera: "Leica M6".into(), lens: "Summicron 50".into(), ..ExifMetadata::default() });
    }

    #[test]
    fn a_fixed_lens_body_names_its_own_lens_only_where_none_is_named() {
        let mut m = ExifMetadata { camera: "Contax T2".into(), ..ExifMetadata::default() };
        complete(&kept(), &mut m);
        assert_eq!((m.lens.as_str(), m.lens_make.as_str()), ("Carl Zeiss Sonnar 38mm f/2.8", "Carl Zeiss"));
        assert_eq!(m.lens_specification, Some(LensSpecification::new(38.0, 38.0, None, None)), "an aperture the catalogue lacks stays unknown");
        // Twin: a lens a hand named stays the lens.
        let mut named = ExifMetadata { camera: "Contax T2".into(), lens: "Something Else".into(), ..ExifMetadata::default() };
        complete(&kept(), &mut named);
        assert_eq!((named.lens.as_str(), named.lens_make.as_str()), ("Something Else", ""));
    }
}
