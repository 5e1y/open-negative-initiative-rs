//! Nikon's High Efficiency RAW is named apart, as LibRaw identifies it without unpacking it; the
//! same camera's lossless NEF still decodes.

#[test]
fn a_high_efficiency_nef_is_named_and_a_lossless_one_decodes() {
    let Some(dir) = oni_fixtures::present(oni_fixtures::NIKON_HE_DIR, "a_high_efficiency_nef_is_named_and_a_lossless_one_decodes") else { return };
    for he in ["Nikon_Z8_high_efficiency_low.NEF", "Nikon_Z8_raw_high_efficiency_hight.NEF"] {
        let failed = oni_decode::decode_on(&dir.join(he), 1.0, 1400, oni_decode::Neutral::Swift).err();
        assert!(failed.as_ref().is_some_and(oni_decode::DecodeError::is_nikon_he), "{he}: {failed:?}");
        assert!(oni_decode::is_nikon_he(&dir.join(he)), "{he} is told from its identification");
    }
    assert!(!oni_decode::is_nikon_he(&dir.join("Nikon_Z8_raw_14_bit_lossless_compression.NEF")));
    // Twin: the lossless NEF of the same body is no High Efficiency one.
    let lossless = oni_decode::decode_on(&dir.join("Nikon_Z8_raw_14_bit_lossless_compression.NEF"), 1.0, 1400, oni_decode::Neutral::Swift);
    assert!(lossless.is_ok_and(|(d, _)| d.width >= 4000), "the lossless NEF decodes at half size");
}
