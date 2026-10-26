# Research notes

Notes written before or beside the port, kept in the language they were written in (French). Each claim is
tagged where the note says so: measured on our files, said by a cited source, or inferred.

- [performance.md](performance.md): fifteen ways to make the Rust app fast, ranked by impact, with their
  sources; what Slint, `wgpu` and the platforms allow or forbid.
- [frontier/recherche-frontier.md](frontier/recherche-frontier.md): the Frontier SP-3000 research and the
  first parameter set of the Frontier Look Lab.
- [frontier/analyse-table-frontier.md](frontier/analyse-table-frontier.md): the measured Frontier table
  taken apart (input curves, matrix, paper curve, hue and chroma), with its figures.
- [frontier/neutre-et-frontier.md](frontier/neutre-et-frontier.md): for each Look Lab parameter, its
  "neutral" value (close to ONI's plain inversion) and its "Frontier" value.
- [decoder-apple-vs-libraw.md](decoder-apple-vs-libraw.md): `CIRAWFilter` against LibRaw on roll #8, the
  S1R and a RAF, the per-camera matrix between them, and how rolls prepared in the Swift app recover.
- [sensor-colours.md](sensor-colours.md): negatives decoded in the sensor's own colours, no camera matrix,
  on roll #8 and the S1R roll against the matrix and the Swift app; the per-roll setting it would become.
- [i18n-dev-builds.md](i18n-dev-builds.md): why the window's `@tr` texts stay English in dev and test builds,
  read in Slint 1.18's interpreter and live preview.
