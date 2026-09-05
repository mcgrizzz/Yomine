//! Warm in-memory storage comparison, not an application startup benchmark.
use std::{
    hint::black_box,
    time::Instant,
};

use yomine::dictionary::kana_preference::{
    bundled,
    KanaIndex,
};
type Rows = Vec<(Vec<u8>, Vec<u8>)>;
fn main() {
    let index = bundled();
    let rows: Rows = (0..index.len())
        .map(|i| {
            let (k, v) = index.record(i).unwrap();
            (k.to_vec(), v.to_vec())
        })
        .collect();
    let json = serde_json::to_vec(&rows).unwrap();
    let binary = bincode::serde::encode_to_vec(&rows, bincode::config::standard()).unwrap();
    let direct = include_bytes!("../assets/kana-preference.bin");
    println!(
        "records={} JSON={} bincode={} direct={} bytes",
        rows.len(),
        json.len(),
        binary.len(),
        direct.len()
    );
    for (name, count) in [("JSON", 100), ("bincode", 100), ("direct", 100000)] {
        let start = Instant::now();
        for _ in 0..count {
            match name {
                "JSON" => {
                    black_box(serde_json::from_slice::<Rows>(black_box(&json)).unwrap());
                }
                "bincode" => {
                    black_box(
                        bincode::serde::decode_from_slice::<Rows, _>(
                            black_box(&binary),
                            bincode::config::standard(),
                        )
                        .unwrap(),
                    );
                }
                _ => {
                    black_box(KanaIndex::from_bytes(black_box(direct)).unwrap());
                }
            }
        }
        println!(
            "{name}: {:.3} us initialization (includes owned result drop)",
            start.elapsed().as_secs_f64() * 1e6 / f64::from(count)
        );
    }
    let start = Instant::now();
    for _ in 0..100 {
        for (key, value) in &rows {
            assert_eq!(index.lookup(black_box(key)), Some(value.as_slice()));
        }
    }
    println!(
        "direct lookup: {:.3} us/query",
        start.elapsed().as_secs_f64() * 1e6 / (100 * rows.len()) as f64
    );
}
