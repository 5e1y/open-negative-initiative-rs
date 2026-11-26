//! The settings no preset row carries, each with why.

/// Fields outside every row, with the reason: absent rather than unticked, so none is ticked by mistake.
pub const EXCLUDED: [(&str, &str); 16] = [
    ("geometry", "depends on this frame's content: pasting one destroys hand-made cropping"),
    ("mode", "negative or positive is what the frame is, not a look one applies"),
    ("invertedSource", "a positive read as its own negative is what the frame is, as its mode"),
    ("virtualFilm", "the film a digital photo is shot on is what it is, as a film's mode"),
    ("film", "which roll a frame belongs to: pasting a look would move photographs about"),
    ("manualOrder", "a frame's own hand-placed position among its siblings"),
    ("sheet", "a contact sheet's own grid, what this scan is"),
    ("corrections", "a defect on this negative, which pasted would paint dust onto another photograph"),
    ("decodedFromMosaic", "which decoder read the file, a fact of the file setting its axis's rest"),
    ("primariesConverted", "the decode this frame's windows were measured on"),
    ("cameraChannels", "whether the decode reads the camera's own channels, a fact of the file's reading its windows stand on"),
    ("rollReference", "the window this frame's roll was measured on, which a look from another roll must not move"),
    ("measuredGains", "the gains this slide's own measure placed, which no other frame's measure is"),
    ("captureStops", "how this scan's capture was exposed against its roll's"),
    ("rgbPasses", "the three files this frame is composed of"),
    ("passOf", "the frame a pass belongs to"),
];
