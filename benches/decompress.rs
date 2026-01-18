use std::{path::PathBuf, sync::OnceLock};

use criterion::{criterion_group, criterion_main, Criterion};
use lidar_decompressor::{decompress_file, DecompressConfig};
use tempfile::NamedTempFile;
use tokio::runtime::Runtime;

fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| Runtime::new().expect("tokio runtime"))
}

fn decompress_benchmark(c: &mut Criterion) {
    // Provide a sample LAZ path via LAZ_SAMPLE env var to run a real benchmark.
    // Fallback is a no-op to keep the harness buildable without sample data.
    if let Ok(sample) = std::env::var("LAZ_SAMPLE") {
        let input = PathBuf::from(sample);
        c.bench_function("decompress_laz", |b| {
            b.iter(|| {
                runtime().block_on(async {
                    let output = NamedTempFile::new().expect("temp output");
                    let out_path = output.path().to_path_buf();
                    decompress_file(&input, &out_path, &DecompressConfig::default())
                        .await
                        .expect("decompress");
                });
            });
        });
    } else {
        c.bench_function("decompress_laz_missing_sample", |b| b.iter(|| {}));
    }
}

criterion_group!(benches, decompress_benchmark);
criterion_main!(benches);
