use std::{
    fs::File,
    path::Path,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use crownos_files_core::{
    fs::lister::list_dir,
    model::{
        entries::EntryId,
        entry::Entry,
        folder::Folder,
        sort::{SortKey, SortOrder},
        ticket::Ticket,
        view_options::ViewOptions,
    },
};
use tokio_util::sync::CancellationToken;

const FILE_COUNT: usize = 100_000;
const INLINE_NAME_BYTES: usize = 24;
const KIB: usize = 1024;

fn resident_bytes() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse::<usize>()
        .ok()?;
    Some(kib * KIB)
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[test]
#[ignore = "benchmark: cargo test --release --test bench -- --ignored --nocapture"]
fn lists_and_sorts_a_hundred_thousand_files() {
    let temp = tempfile::tempdir().unwrap();
    for index in 0..FILE_COUNT {
        let shuffled = index * 7919 % FILE_COUNT;
        File::create(
            temp.path()
                .join(format!("IMG_2024{shuffled:06}_{index}.jpg")),
        )
        .unwrap();
    }
    let dir: Arc<Path> = temp.path().into();
    let resident_before = resident_bytes().unwrap();

    let started = Instant::now();
    let mut folder = Folder::new(dir.clone(), ViewOptions::default());
    let mut first_chunk = None;
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::channel();
        let listed = dir.clone();
        scope.spawn(move || {
            list_dir(&listed, Ticket::next(), &CancellationToken::new(), |op| {
                let _ = sender.send(op);
            });
        });
        for op in receiver {
            folder.apply(op);
            first_chunk.get_or_insert_with(|| started.elapsed());
        }
    });
    let sorted = started.elapsed();
    assert_eq!(folder.len(), FILE_COUNT);

    folder.set_sort(SortOrder {
        key: SortKey::Modified,
        reverse: true,
        dirs_first: true,
    });
    let resort_started = Instant::now();
    folder.refresh_order();
    let resorted = resort_started.elapsed();

    let heap_names: usize = folder
        .iter()
        .map(|entry| entry.name.len())
        .filter(|&len| len > INLINE_NAME_BYTES)
        .sum();
    let model_bytes = folder.len() * (size_of::<Entry>() + size_of::<EntryId>()) + heap_names;
    let resident = resident_bytes().unwrap().saturating_sub(resident_before);

    println!("entries:             {FILE_COUNT}");
    println!(
        "time to first chunk: {:.2} ms",
        millis(first_chunk.unwrap_or_default())
    );
    println!("time to sorted:      {:.2} ms", millis(sorted));
    println!("full parallel resort:{:.2} ms", millis(resorted));
    println!(
        "model bytes/entry:   {:.1}",
        model_bytes as f64 / FILE_COUNT as f64
    );
    println!(
        "RSS delta bytes/entry: {:.1}",
        resident as f64 / FILE_COUNT as f64
    );
}
