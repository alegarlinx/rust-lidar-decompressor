# High-Throughput LiDAR Decompressor BETA (Rust) 🦀

![Language](https://img.shields.io/badge/rust-1.75%2B-orange.svg)
![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Architecture](https://img.shields.io/badge/architecture-zero--allocation-success)
![Binary Size](https://img.shields.io/badge/binary%20size-%3C1MB-brightgreen)

A high-performance systems engineering project designed to decompress massive geospatial datasets (`.laz` to `.las`) with extreme efficiency.

Unlike typical scripts, this tool uses a **Zero-Allocation Loop** architecture with **Object Pooling** and **Memory Mapped I/O**. It saturates CPU and Disk bandwidth simultaneously, achieving throughputs of **~2.95 million points per second** on consumer hardware.

---

## 🚀 Performance Benchmark

Benchmarks run on **Apple Silicon (M1/M2) MacBook Air**.

| Metric | Result |
| :--- | :--- |
| **Dataset** | Autzen Stadium (`autzen.laz`) |
| **Point Count** | 10,653,336 points |
| **Input Size** | 56 MB (Compressed LAZ) |
| **Output Size** | **~330 MB** (Uncompressed LAS) |
| **Execution Time** | **3.61 seconds** |
| **Throughput** | **~2.95 Million points/sec** |
| **CPU Utilization** | **174%** (Multicore Saturation) |

> *Note: The tool maintains O(1) memory usage regardless of input file size (tested with GB-scale files).*
---
#  Install 💻

## Prerequisites💁🏻
- Rust toolchain (cargo, rustc). Install via https://rustup.rs/.

## Build 🚧
```
cargo build --release
```

## Run 🏃🏼
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


---

## 🛠 System Architecture

This project solves the "Blocking I/O" and "Memory Allocation" bottlenecks common in naive implementations.

### The Pipeline Pattern
It uses a **Producer-Consumer** model orchestrated via `crossbeam` channels, with a **Recycle Loop** to reuse memory vectors, eliminating the need for constant memory allocation/deallocation calls to the OS.

```mermaid
graph LR
    subgraph Input
        A["Disk .laz"] -- "Memory Map (mmap)" --> B["Virtual Memory"]
    end

    subgraph "Thread 1: Producer (CPU)"
        B -- "Decode" --> C["Fill Vector"]
        C -- "Send Full Batch" --> D["Data Channel"]
    end

    subgraph "Thread 2: Consumer (I/O)"
        D -- "Receive" --> E["BufWriter (4MB)"]
        E -- "Write" --> F["Disk .las"]
        E -- "Return Empty Vec" --> G["Recycle Channel"]
    end

    G -- "Reuse Memory" --> C

    style D fill:#f9f,stroke:#333,stroke-width:2px
    style G fill:#bbf,stroke:#333,stroke-width:2px,stroke-dasharray: 5 5
