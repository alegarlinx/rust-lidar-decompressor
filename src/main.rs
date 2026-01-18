use clap::Parser;
use crossbeam_channel::bounded;
use indicatif::{ProgressBar, ProgressStyle};
use las::{Read, Write, Reader, Writer};
use memmap2::MmapOptions;
use mimalloc::MiMalloc;
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::PathBuf;
use std::thread;
use std::time::Instant;

// --- OPTIMIZACIÓN DE MEMORIA GLOBAL ---
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(required = true)]
    input: PathBuf,
    #[arg(short, long)]
    output: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let start_time = Instant::now();

    // --- CONFIGURACIÓN DE RENDIMIENTO ---
    let batch_size = 50_000;    // El "Sweet Spot" para la caché L2 del M1
    let channel_cap = 20;       // Suficiente buffer para no bloquear
    let write_buffer = 4 * 1024 * 1024; // 4 MB de Buffer de escritura (Muy importante)

    println!("Starting Elite Decompressor (Object Pooling + 4MB Write Buffer)...");
    
    // 1. INPUT (Memory Map)
    let file = File::open(&args.input)?;
    let mmap = unsafe { MmapOptions::new().map(&file)? };
    let cursor = Cursor::new(&mmap);
    let mut reader = Reader::new(cursor)?;
    let header = reader.header().clone();
    let total_points = header.number_of_points();

    // 2. CANALES (DOBLE VÍA PARA RECICLAJE)
    // tx/rx: Envían datos llenos (Productor -> Consumidor)
    // recycle_tx/recycle_rx: Devuelven vectores vacíos (Consumidor -> Productor)
    let (tx, rx) = bounded::<Vec<las::Point>>(channel_cap);
    let (recycle_tx, recycle_rx) = bounded::<Vec<las::Point>>(channel_cap);

    // 3. PRE-CALENTAMIENTO (POOLING)
    // Creamos los vectores UNA sola vez y los metemos en el circuito.
    for _ in 0..channel_cap {
        recycle_tx.send(Vec::with_capacity(batch_size)).unwrap();
    }

    let pb = ProgressBar::new(total_points);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")?
        .progress_chars("#>-"));

    // 4. THREAD PRODUCTOR
    let producer_handle = thread::spawn(move || {
        // Cogemos el primer vector vacío del pool
        let mut batch = recycle_rx.recv().unwrap();

        for point in reader.points() {
            if let Ok(p) = point {
                batch.push(p);

                if batch.len() >= batch_size {
                    // Enviamos el vector lleno
                    if tx.send(batch).is_err() { break; }
                    
                    // Esperamos recibir un vector vacío reciclado para seguir
                    match recycle_rx.recv() {
                        Ok(new_batch) => batch = new_batch,
                        Err(_) => break, // El consumidor murió
                    }
                }
            }
        }
        // Enviar el último lote si tiene algo
        if !batch.is_empty() {
            let _ = tx.send(batch);
        }
    });

    // 5. THREAD CONSUMIDOR (MAIN)
    // Usamos BufWriter manual con 4MB para reducir syscalls al disco
    let file_out = File::create(&args.output)?;
    let buf_writer = BufWriter::with_capacity(write_buffer, file_out);
    let mut writer = Writer::new(buf_writer, header)?;

    for mut batch in rx {
        for point in &batch {
            writer.write(point.clone())?;
        }
        
        let processed = batch.len() as u64;
        
        // --- LA MAGIA DEL RECICLAJE ---
        batch.clear(); // Borra los datos pero MANTIENE la memoria reservada
        
        // Devolvemos el vector vacío al productor
        if recycle_tx.send(batch).is_err() {
            break;
        }
        
        pb.inc(processed);
    }

    producer_handle.join().expect("Producer panic");
    pb.finish_with_message("Done!");

    let duration = start_time.elapsed();
    let throughput = (total_points as f64 / 1_000_000.0) / duration.as_secs_f64();
    
    println!("Finished in {:.2?}", duration);
    println!("⚡ Throughput: {:.2} Million points/sec", throughput);

    Ok(())
}