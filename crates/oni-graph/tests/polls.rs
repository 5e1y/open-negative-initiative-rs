//! Threads polling the one device: a wait that runs out while another thread retires the work it
//! waits on is a timeout, the next slice seeing it run, never a panic in the waiting thread.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the threads race: wgpu-core 30 unpatched panics within half a second.
const RACE: Duration = Duration::from_secs(2);

/// 32 MB copies a submission: more GPU time than `ran`'s slice, as a render takes.
const COPIES: usize = 8;

/// What the race counted: the work handed over, the waits that ran out and the drains that found the
/// queue empty, and each panic a polling thread raised.
struct Raced {
    submitted: u64,
    timeouts: u64,
    emptied: u64,
    panics: Vec<String>,
}

/// One thread hands copies to the queue and drains it, as a render thread does; `waiters` threads wait on
/// its latest submission, the first through `oni_graph::ran`, the others in slices cut to nothing.
fn race(waiters: usize) -> Option<Raced> {
    let gpu = oni_graph::gpu().ok()?;
    let (device, queue) = (gpu.device, gpu.queue);
    let size = 1 << 25;
    let usage = wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST;
    let buffer = |label| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage, mapped_at_creation: false });
    let (from, to) = (buffer("race from"), buffer("race to"));
    let latest: Arc<Mutex<Option<wgpu::SubmissionIndex>>> = Arc::default();
    let (submitted, timeouts, emptied) = (AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0));
    let ends = Instant::now() + RACE;
    let panics = std::thread::scope(|s| {
        // Each copy is drained to an empty queue, the moment a waiter's slice may be running out.
        let mut threads = vec![s.spawn(|| {
            while Instant::now() < ends {
                let mut encoder = device.create_command_encoder(&Default::default());
                for _ in 0..COPIES {
                    encoder.copy_buffer_to_buffer(&from, 0, &to, 0, size);
                }
                *latest.lock().unwrap() = Some(queue.submit([encoder.finish()]));
                submitted.fetch_add(1, Ordering::Relaxed);
                while !matches!(device.poll(wgpu::PollType::Poll), Ok(wgpu::PollStatus::QueueEmpty) | Err(_)) {}
                emptied.fetch_add(1, Ordering::Relaxed);
            }
        })];
        for waiter in 0..waiters {
            let (device, latest, timeouts) = (&device, &latest, &timeouts);
            threads.push(s.spawn(move || {
                while Instant::now() < ends {
                    let Some(index) = latest.lock().unwrap().clone() else { continue };
                    if waiter == 0 {
                        oni_graph::ran(device, &index).expect("the device holds");
                        continue;
                    }
                    let wait = wgpu::PollType::Wait { submission_index: Some(index), timeout: Some(Duration::ZERO) };
                    if let Err(wgpu::PollError::Timeout) = device.poll(wait) {
                        timeouts.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }));
        }
        let panics: Vec<String> = threads
            .into_iter()
            .filter_map(|t| t.join().err())
            .map(|p| p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())
            .collect();
        panics
    });
    let _ = device.poll(wgpu::PollType::wait_indefinitely());
    Some(Raced { submitted: submitted.into_inner(), timeouts: timeouts.into_inner(), emptied: emptied.into_inner(), panics })
}

/// The race was run: waits ran out by the thousand while the drains emptied the queue under them.
fn ran_the_race(raced: &Raced) -> bool {
    raced.timeouts > 1000 && raced.emptied > 20
}

#[test]
fn a_wait_that_runs_out_while_another_thread_retires_its_work_is_a_timeout() {
    let Some(raced) = race(5) else { return println!("SKIPPED polls: no adapter") };
    println!("{} submissions, {} waits ran out, {} drains found the queue empty", raced.submitted, raced.timeouts, raced.emptied);
    assert!(raced.panics.is_empty(), "a polling thread panicked: {:?}", raced.panics);
    assert!(ran_the_race(&raced), "the threads never raced: the check saw nothing");
    // Twin: with no thread waiting, nothing runs out and the judge refuses a race that could not panic.
    let alone = race(0).expect("the same adapter");
    assert!(alone.panics.is_empty() && !ran_the_race(&alone), "a race with no waiter passed for one");
}
