use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Instant;

use anyhow::{Context, Result};
use crossbeam_channel::bounded;
use indicatif::{ProgressBar, ProgressStyle};
// CORRECCIÓN AQUÍ: Añadimos 'Read' y 'Write' para desbloquear los métodos
use las::{Read, Reader, Write, Writer}; 
use memmap2::Mmap;
use tracing::{info, debug};

/// Configuración para el proceso de descompresión.
#[derive(Debug, Clone, Default)]
pub struct DecompressConfig {
    pub threads: Option<usize>,
    pub output: Option<PathBuf>,
}

// Tamaño del bloque de puntos. 50k es un buen equilibrio.
const CHUNK_SIZE: usize = 50_000;

pub async fn decompress_file(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    _config: &DecompressConfig,
) -> Result<()> {
    let input_path = input.as_ref().to_path_buf();
    let output_path = output.as_ref().to_path_buf();
    let start_time = Instant::now();

    // 1. ANÁLISIS PRELIMINAR (Lectura de cabecera)
    let file = File::open(&input_path).context("No se pudo abrir el archivo de entrada")?;
    // SAFETY: mmap es seguro siempre que el archivo no se modifique externamente.
    let mmap = unsafe { Mmap::map(&file).context("Fallo al mapear archivo a memoria (mmap)")? };
    
    let cursor = Cursor::new(&mmap);
    let reader = Reader::new(cursor).context("Fallo al leer cabecera LAZ")?;
    let header = reader.header().clone();
    let total_points = header.number_of_points();

    info!("Iniciando pipeline: {} -> {}", input_path.display(), output_path.display());
    debug!("Total de puntos a procesar: {}", total_points);

    // 2. CONFIGURACIÓN DE UX
    let pb = ProgressBar::new(total_points);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} pts ({eta}) {msg}")?
        .progress_chars("#>-"));
    pb.set_message("Descomprimiendo...");

    // 3. CANAL DE COMUNICACIÓN
    let (tx, rx) = bounded::<Vec<las::Point>>(10);

    // 4. HILO ESCRITOR (Consumer)
    let writer_handle = thread::spawn(move || -> Result<()> {
        let file_out = File::create(&output_path).context("No se pudo crear archivo de salida")?;
        let buf_writer = BufWriter::new(file_out);
        
        // Ahora funcionará porque hemos importado 'Write' arriba
        let mut writer = Writer::new(buf_writer, header).context("Fallo al inicializar LAS writer")?;
        
        while let Ok(chunk) = rx.recv() {
            for point in chunk {
                writer.write(point)?;
            }
        }
        Ok(())
    });

    // 5. HILO LECTOR/DESCOMPRESOR (Producer)
    tokio::task::spawn_blocking(move || -> Result<()> {
        // Re-abrimos para evitar problemas de lifetime con el mmap original en el closure
        let file = File::open(&input_path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let cursor = Cursor::new(&mmap);
        
        // Ahora funcionará porque hemos importado 'Read' arriba
        let mut reader = Reader::new(cursor)?;

        let mut chunk = Vec::with_capacity(CHUNK_SIZE);
        
        for point in reader.points() {
            let p = point.context("Error decodificando punto")?;
            chunk.push(p);

            if chunk.len() >= CHUNK_SIZE {
                tx.send(chunk).context("Fallo al enviar datos al escritor")?;
                chunk = Vec::with_capacity(CHUNK_SIZE);
                pb.inc(CHUNK_SIZE as u64);
            }
        }

        if !chunk.is_empty() {
            let len = chunk.len() as u64;
            tx.send(chunk).context("Fallo al enviar último bloque")?;
            pb.inc(len);
        }

        pb.finish_with_message("¡Completado!");
        Ok(())
    }).await??;

    writer_handle.join().map_err(|_| anyhow::anyhow!("El hilo escritor entró en pánico"))??;

    let duration = start_time.elapsed();
    let throughput = (total_points as f64 / 1_000_000.0) / duration.as_secs_f64();
    
    info!("Proceso finalizado en {:.2?}", duration);
    info!("Velocidad promedio: {:.2} Millones de puntos/seg", throughput);

    Ok(())
}