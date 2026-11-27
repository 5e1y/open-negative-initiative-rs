//! The films' checks: every film reads and decodes; the player, replayed on a clock of its own with a chosen cost of
//! decoding, keeps the film's time on any processor, so no check here hangs on how loaded the machine is.

use super::*;

fn films() -> Vec<(Cut, &'static [u8])> {
    FILMS.iter().map(|(gift, bytes, list)| (Cut::read(*gift, bytes, list).expect("the film's list matches its frames"), *bytes)).collect()
}

#[test]
fn every_film_reads_and_each_frame_decodes_at_its_size() {
    for (gift, bytes, list) in FILMS {
        let cut = Cut::read(gift, bytes, list).expect("the film's list matches its frames");
        assert!(cut.length > 4.0 && cut.frames.len() > 20, "gift {gift}: {} s, {} frames", cut.length, cut.frames.len());
        for i in [0, cut.frames.len() / 2, cut.frames.len() - 1] {
            assert!(decode(&cut, bytes, i).is_some(), "gift {gift}: frame {i}");
        }
        // Twin: a list that leaves a frame out does not match the file.
        let short: String = list.lines().take(list.lines().count() - 1).map(|l| format!("{l}\n")).collect();
        assert!(Cut::read(gift, bytes, &short).is_none());
    }
}

#[test]
fn the_decoder_aims_where_the_film_will_be_and_the_screen_never_goes_back() {
    let (cut, _) = films().remove(0);
    let n = cut.frames.len();
    let t = 3.0;
    assert_eq!(aimed(&cut, t, 0.0)[0], cut.index(t, false), "with no cost, the frame due now");
    let ahead = aimed(&cut, t, 0.3)[0];
    assert_eq!(ahead, cut.index(t + 0.3, false), "a frame costing 0.3 s aims at the one due then");
    assert!(after(cut.index(t, false), ahead, n) > 0, "twin: past the frame due now");
    // The frame due if it is ready, else the latest past; never one still to come.
    assert_eq!(pick([5, 6, 7].into_iter(), 6, n), Some(6));
    assert_eq!(pick([3, 5, 9].into_iter(), 7, n), Some(5));
    assert_eq!(pick([8, 9].into_iter(), 7, n), None);
}

/// What a screen at 120 Hz saw of `cut` over its whole length, with `decoders` that each take `cost` seconds a frame,
/// aiming ahead by what a frame costs or not: how many frames were late on the tick that first wanted them, by how
/// many frames at most the screen fell behind the film, and how many of the frames it showed were already past.
fn replay(cut: &Cut, cost: f32, decoders: usize, lead: bool) -> (usize, usize, usize, usize) {
    let n = cut.frames.len();
    let tick = 1.0 / 120.0;
    let (mut decoded, mut busy): (Vec<usize>, Vec<(usize, f32)>) = (Vec::new(), Vec::new());
    let (mut shown, mut last, mut late, mut changes, mut behind, mut reckoned) = (None, usize::MAX, 0, 0, 0, FIRST_COST);
    let mut stale = 0;
    let mut t = 0.0;
    while t < cut.length {
        t += tick;
        let aim = |reckoned: f32| aimed(cut, t, if lead { reckoned } else { 0.0 });
        // Decoders done by now hand in their frame, kept as the decoders keep it.
        for (frame, _) in busy.iter().filter(|(_, done)| *done <= t) {
            reckoned = 0.7 * reckoned + 0.3 * cost;
            decoded.push(*frame);
        }
        busy.retain(|(_, done)| *done > t);
        let due = cut.index(t, false);
        let past = pick(decoded.iter().copied(), due, n);
        let wanted = aim(reckoned);
        decoded.retain(|i| kept(*i, due, &wanted, past, n));
        while busy.len() < decoders {
            let Some(next) = wanted.iter().find(|i| !decoded.contains(i) && busy.iter().all(|(b, _)| b != *i)) else { break };
            busy.push((*next, t + cost));
        }
        if let Some(chosen) = pick(decoded.iter().copied(), due, n)
            && shown.is_none_or(|s| s != chosen && after(chosen, s, n) >= n / 2)
        {
            shown = Some(chosen);
            stale += usize::from(chosen != due);
        }
        if due != last {
            (changes, last) = (changes + 1, due);
            late += usize::from(shown != Some(due));
        }
        let lag = shown.map_or(0, |s| after(s, due, n));
        if lag > behind && std::env::var_os("ONI_REPLAY").is_some() {
            eprintln!("t {t:.2} due {due} shown {shown:?} decoded {decoded:?} busy {busy:?} reckoned {reckoned:.3}");
        }
        behind = behind.max(lag);
        assert!(decoded.len() <= AHEAD + 2, "at most {} frames held: {}", AHEAD + 2, decoded.len());
    }
    (late, changes, behind, stale)
}

#[test]
fn the_player_keeps_the_films_time_however_slow_its_decoders() {
    // Each film's frames as decoded here, timed: printed, never judged, as the machine's load moves them.
    for (cut, bytes) in films() {
        let began = Instant::now();
        let some = (0..cut.frames.len()).step_by(cut.frames.len() / 8).inspect(|i| assert!(decode(&cut, bytes, *i).is_some())).count();
        eprintln!("films: gift {}: a frame decodes in {:.0} ms here", cut.gift, began.elapsed().as_secs_f32() * 1000.0 / some as f32);
    }
    for (cut, _) in films() {
        let gift = cut.gift;
        // At this Mac's pace, about 30 ms a frame, two decoders keep every frame on time.
        let (late, changes, _, _) = replay(&cut, 0.03, 2, true);
        assert!(late <= 1, "gift {gift}: {late} of {changes} frames late at 30 ms a frame");
        // Four times slower, a processor a quarter as fast: frames are skipped, the film keeps its time.
        let (late, changes, behind, stale) = replay(&cut, 0.12, 2, true);
        // Twin: a decoder that aims at the frame due now, not where the film will be, shows frames already past.
        let (_, _, behind_now, stale_now) = replay(&cut, 0.12, 2, false);
        eprintln!("films: gift {gift} at 120 ms a frame: {behind} frames behind at most, {stale} shown past, {late} of {changes} skipped; aimed at now: {behind_now} behind, {stale_now} shown past");
        assert!(behind <= 4 && late < changes, "gift {gift}: {behind} frames behind, {late} of {changes} late, at 120 ms a frame");
        assert!(stale_now >= 3 * stale.max(1) && behind_now > behind, "gift {gift}: aimed at now {stale_now} shown past and {behind_now} behind, aimed ahead {stale} and {behind}");
    }
}

#[test]
fn a_film_on_screen_comes_without_holding_the_interface_and_off_screen_holds_nothing() {
    let mut reel = reels().into_iter().next().expect("a film");
    let asked = Instant::now();
    let mut first = None;
    while first.is_none() && asked.elapsed() < Duration::from_secs(10) {
        let t = asked.elapsed().as_secs_f32();
        let call = Instant::now();
        first = reel.at(t, false);
        // The interface asks and is answered at once, whatever the decoders are doing.
        assert!(call.elapsed() < Duration::from_millis(5), "a frame asked takes {:?}", call.elapsed());
        std::thread::sleep(Duration::from_millis(8));
    }
    assert!(first.is_some(), "a frame comes");
    assert!(reel.slots.0.lock().unwrap().decoded.len() <= AHEAD + 2);
    reel.rest();
    std::thread::sleep(Duration::from_millis(100));
    assert!(reel.slots.0.lock().unwrap().decoded.is_empty(), "a film off screen holds no frame");
    // Twin: held still, the frame it holds longest.
    let still = reel.cut.index(0.0, true);
    reel.again();
    let asked = Instant::now();
    while reel.at(0.0, true).is_none() && asked.elapsed() < Duration::from_secs(10) {
        std::thread::sleep(Duration::from_millis(8));
    }
    assert_eq!(reel.shown, Some(still));
}
