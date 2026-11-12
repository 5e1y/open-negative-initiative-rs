use super::*;

#[test]
fn only_a_notarised_package_signed_by_adobe_is_opened() {
    let adobe = "Package \"DNGConverter_18_0.pkg\":\n   Status: signed by a developer certificate issued by Apple for distribution\n   Notarization: trusted by the Apple notary service\n   Signed with a trusted timestamp on: 2025-10-16 04:41:11 +0000\n   Certificate Chain:\n    1. Developer ID Installer: Adobe Inc. (JQ525L2MZD)\n";
    assert!(adobe_signed(adobe));
    // Twins: another signer, or Adobe's name without Apple's notarisation, is refused.
    assert!(!adobe_signed(&adobe.replace("Adobe Inc. (JQ525L2MZD)", "Adobe Systems (ABCDE12345)")));
    assert!(!adobe_signed(&adobe.replace("Notarization: trusted by the Apple notary service\n", "")));
}

/// A stand-in for Adobe's program: each file after `-d <folder>` gets its `.dng` there, a copy of
/// `sample.dng` but for the `cut` file's, which stops half way through its strip; every run's files
/// are logged, so the check sees which were handed over.
#[cfg(unix)]
fn stand_in(dir: &Path, cut: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let program = dir.join("converter.sh");
    let log = dir.join("handed.log");
    let sample = dir.join("sample.dng");
    oni_fixtures::dng::mosaic_dng(&sample);
    let whole = std::fs::metadata(&sample).unwrap().len() as usize;
    let half = whole - oni_fixtures::dng::MOSAIC_W * oni_fixtures::dng::MOSAIC_H;
    let script = format!(
        "#!/bin/sh\nwhile [ \"$1\" != \"-d\" ]; do shift; done\nshift; out=\"$1\"; shift\nfor f in \"$@\"; do n=$(basename \"$f\"); echo \"$n\" >> '{log}'; if [ \"$n\" = '{cut}' ]; then head -c {half} '{sample}' > \"$out/${{n%.*}}.dng\"; else cp '{sample}' \"$out/${{n%.*}}.dng\"; fi; done\n",
        log = log.display(),
        sample = sample.display(),
    );
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    program
}

#[cfg(unix)]
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oni-adobe-dng-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
#[test]
fn a_folder_converts_in_batches_and_only_a_dng_that_reads_counts() {
    let dir = scratch("batches");
    let program = stand_in(&dir, "DSC_0002.NEF");
    let raws: Vec<PathBuf> = (1..=6).map(|i| dir.join(format!("DSC_000{i}.NEF"))).collect();
    for r in &raws {
        std::fs::write(r, b"nef").unwrap();
    }
    // One DNG already stands, the person's own: it keeps its bytes, its NEF is never handed over, and it is not this run's.
    std::fs::write(dir.join("DSC_0003.dng"), b"theirs").unwrap();
    let counts = std::cell::RefCell::new(Vec::new());
    let converted = convert(&program, &raws, &|n| {
        counts.borrow_mut().push(n);
        true
    })
    .unwrap();
    let expected: Vec<PathBuf> = [1, 4, 5, 6].iter().map(|i| raws[i - 1].clone()).collect();
    assert_eq!(converted, expected, "a cut DNG and the person's own DNG are not conversions");
    assert_eq!(*counts.borrow(), vec![2, 4], "the count moves after each batch of {BATCH}");
    assert_eq!(std::fs::read(dir.join("DSC_0003.dng")).unwrap(), b"theirs");
    assert!(!dir.join("DSC_0002.dng").exists(), "the cut DNG is removed");
    // Nothing stands for the NEF, so convertible() offers it again.
    assert!(!has_twin(&raws[1]));
    let handed = std::fs::read_to_string(dir.join("handed.log")).unwrap();
    assert!(!handed.contains("DSC_0003.NEF") && handed.contains("DSC_0004.NEF"), "{handed}");
    // Twin: the same run with no cut file counts DSC_0002, so the exclusion above is the cut's.
    let whole = scratch("whole");
    let program = stand_in(&whole, "none");
    let raw = whole.join("DSC_0002.NEF");
    std::fs::write(&raw, b"nef").unwrap();
    assert_eq!(convert(&program, std::slice::from_ref(&raw), &|_| true).unwrap(), vec![raw]);
    // Twin: stopped after the first batch, the rest is never converted.
    let more: Vec<PathBuf> = (7..=9).map(|i| dir.join(format!("DSC_000{i}.NEF"))).chain([dir.join("DSC_0010.NEF"), dir.join("DSC_0011.NEF")]).collect();
    for r in &more {
        std::fs::write(r, b"nef").unwrap();
    }
    let stopped = convert(&program, &more, &|_| false).unwrap();
    assert_eq!(stopped.len(), BATCH);
    assert!(!dir.join("DSC_0011.dng").exists());
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&whole);
}

#[test]
fn a_high_efficiency_nef_leaves_the_listing_to_its_dng() {
    let Some(samples) = oni_fixtures::present(oni_fixtures::NIKON_HE_DIR, "a_high_efficiency_nef_leaves_the_listing_to_its_dng") else { return };
    let dir = std::env::temp_dir().join(format!("oni-adobe-dng-listing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (he, lossless) = (dir.join("HE.NEF"), dir.join("Lossless.NEF"));
    std::fs::copy(samples.join("Nikon_Z8_high_efficiency_low.NEF"), &he).unwrap();
    std::fs::copy(samples.join("Nikon_Z8_raw_14_bit_lossless_compression.NEF"), &lossless).unwrap();
    assert_eq!(convertible(&[&he, &lossless]), vec![he.clone()], "only the High Efficiency NEF asks to be converted");
    let names = |paths: &[&Path]| paths.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>();
    let mut listed = names(&[&he, &dir.join("HE.dng"), &lossless, &dir.join("Lossless.dng")]);
    superseded(&mut listed);
    assert_eq!(listed, names(&[&dir.join("HE.dng"), &lossless, &dir.join("Lossless.dng")]), "the DNG stands for the HE NEF alone");
    // Twin: with no DNG beside it, the HE NEF stays listed.
    let mut alone = names(&[&he]);
    superseded(&mut alone);
    assert_eq!(alone.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
