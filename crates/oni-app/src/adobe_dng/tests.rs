use super::*;

#[test]
fn only_a_notarised_package_signed_by_adobe_is_opened() {
    let adobe = "Package \"DNGConverter_18_0.pkg\":\n   Status: signed by a developer certificate issued by Apple for distribution\n   Notarization: trusted by the Apple notary service\n   Signed with a trusted timestamp on: 2025-10-16 04:41:11 +0000\n   Certificate Chain:\n    1. Developer ID Installer: Adobe Inc. (JQ525L2MZD)\n";
    assert!(adobe_signed(adobe));
    // Twins: another signer, or Adobe's name without Apple's notarisation, is refused.
    assert!(!adobe_signed(&adobe.replace("Adobe Inc. (JQ525L2MZD)", "Adobe Systems (ABCDE12345)")));
    assert!(!adobe_signed(&adobe.replace("Notarization: trusted by the Apple notary service\n", "")));
}

/// A stand-in for Adobe's program: each file after `-d <folder>` gets its `.dng` there, and every
/// run's files are logged, so the check sees which were handed over.
#[cfg(unix)]
fn stand_in(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let program = dir.join("converter.sh");
    let log = dir.join("handed.log");
    let script = format!(
        "#!/bin/sh\nwhile [ \"$1\" != \"-d\" ]; do shift; done\nshift; out=\"$1\"; shift\nfor f in \"$@\"; do n=$(basename \"$f\"); echo \"$n\" >> '{}'; : > \"$out/${{n%.*}}.dng\"; done\n",
        log.display()
    );
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    program
}

#[cfg(unix)]
#[test]
fn a_folder_converts_in_batches_and_no_dng_is_written_over() {
    let dir = std::env::temp_dir().join(format!("oni-adobe-dng-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let program = stand_in(&dir);
    let raws: Vec<PathBuf> = (1..=6).map(|i| dir.join(format!("DSC_000{i}.NEF"))).collect();
    for r in &raws {
        std::fs::write(r, b"nef").unwrap();
    }
    // One DNG already stands, the person's own: it keeps its bytes and its NEF is never handed over.
    std::fs::write(dir.join("DSC_0003.dng"), b"theirs").unwrap();
    let counts = std::cell::RefCell::new(Vec::new());
    let converted = convert(&program, &raws, &|n| {
        counts.borrow_mut().push(n);
        true
    })
    .unwrap();
    assert_eq!(converted, raws, "every NEF has its DNG");
    assert_eq!(*counts.borrow(), vec![4, 6], "the count moves after each batch of {BATCH}");
    assert_eq!(std::fs::read(dir.join("DSC_0003.dng")).unwrap(), b"theirs");
    let handed = std::fs::read_to_string(dir.join("handed.log")).unwrap();
    assert!(!handed.contains("DSC_0003.NEF") && handed.contains("DSC_0004.NEF"), "{handed}");
    // Twin: stopped after the first batch, the rest is never converted.
    let more: Vec<PathBuf> = (7..=9).map(|i| dir.join(format!("DSC_000{i}.NEF"))).chain([dir.join("DSC_0010.NEF"), dir.join("DSC_0011.NEF")]).collect();
    for r in &more {
        std::fs::write(r, b"nef").unwrap();
    }
    let stopped = convert(&program, &more, &|_| false).unwrap();
    assert_eq!(stopped.len(), BATCH);
    assert!(!dir.join("DSC_0011.dng").exists());
    let _ = std::fs::remove_dir_all(&dir);
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
