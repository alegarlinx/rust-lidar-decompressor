# lidar-decompressor

High-throughput LAZ/COPC decompressor scaffold in Rust. Current implementation streams LAZ -> LAS using the `las` crate’s built-in LAZ decoder; replace with SIMD-friendly entropy decoding, chunk-parallel scheduling, and COPC-aware tiling.

## Prerequisites
- Rust toolchain (cargo, rustc). Install via https://rustup.rs/.

## Build
```
cargo build --release
```

## Run
```
cargo run --release -- <input.laz> --output <output.las>
```

## Benchmarks
- Provide a LAZ sample via `LAZ_SAMPLE=/path/to/file.laz`
- Run: `cargo bench --bench decompress`
- Reports are emitted by Criterion (HTML available under `target/criterion`).

## Roadmap
- Integrate custom LAZ decoding (vectorized entropy + predictors) to replace the default decoder.
- COPC range-request and tiling support.
- Async I/O + chunked parallel decode tuned for L2/L3.
- Benchmarks targeting ≥1–2 GB/s/core on AVX2.
- Optional clean-room decoder for IP.
