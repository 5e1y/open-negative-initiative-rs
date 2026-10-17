//! Two decodes at once must give, bit for bit, what each gives alone: LibRaw keeps per-instance state.

mod common;

use std::sync::{Arc, Barrier};

fn bits(px: &[[f32; 4]]) -> Vec<u32> {
    px.iter().flat_map(|p| p.map(f32::to_bits)).collect()
}

#[test]
fn concurrent_decodes_match_sequential_ones() {
    let Some(files) = common::frames("concurrency", 2) else {
        return;
    };
    let (a, b) = (files[0].clone(), files[1].clone());
    let alone = [&a, &b].map(|f| bits(&oni_decode::decode(f, 1.0).expect("decode").pixels));
    let start = Arc::new(Barrier::new(2));
    let together: Vec<Vec<u32>> = [a.clone(), b.clone()]
        .into_iter()
        .map(|f| {
            let start = start.clone();
            std::thread::spawn(move || {
                start.wait();
                bits(&oni_decode::decode(&f, 1.0).expect("decode").pixels)
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|t| t.join().expect("decode thread"))
        .collect();
    assert!(
        together[0] == alone[0],
        "{} differs when decoded beside another file",
        a.display()
    );
    assert!(
        together[1] == alone[1],
        "{} differs when decoded beside another file",
        b.display()
    );
    // Twin: the comparison sees a difference, since two frames never decode to the same bits.
    assert!(
        alone[0] != alone[1],
        "the comparison cannot tell two frames apart"
    );
}
