use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use lidar_decompressor::{decompress_file, DecompressConfig};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "lidar-decompressor", version = "0.2.0", about = "High-throughput LAZ to LAS decompressor")]
struct Cli {
    /// Ruta del archivo de entrada (.laz o .copc.laz)
    input: PathBuf,

    /// Ruta del archivo de salida (.las). Si se omite, se deduce del input.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Número de hilos (Opcional - Preparado para futura implementación SIMD)
    #[arg(short = 't', long)]
    threads: Option<usize>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    // Inicializar sistema de logs
    // Filtramos librerías ruidosas para que la terminal se vea limpia
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lidar_decompressor=info"));
    
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    let args = Cli::parse();

    // 1. Validación de entrada
    if !args.input.exists() {
        anyhow::bail!("El archivo de entrada no existe: {:?}", args.input);
    }

    // 2. Deducir salida si no se especifica
    let output = args.output.clone()
        .unwrap_or_else(|| derive_output_path(&args.input));

    // 3. Configuración
    let config = DecompressConfig {
        threads: args.threads,
        output: Some(output.clone()),
    };

    // 4. Ejecución Controlada
    decompress_file(&args.input, &output, &config)
        .await
        .with_context(|| "Error fatal durante la ejecución del pipeline")?;

    Ok(())
}

fn derive_output_path(input: &PathBuf) -> PathBuf {
    let mut out = input.clone();
    // Si es .laz, cambia a .las. Si es .copc.laz, trata de manejarlo inteligentemente.
    out.set_extension("las");
    out
}