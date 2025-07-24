use std::fs::{File};
use std::io::{BufReader};
use std::path::{PathBuf};
use std::error::Error;

use clap::Parser;
use walkdir::WalkDir;

/// Compress all files in a directory using Zstandard
#[derive(Parser, Debug)]
#[command(version, about = "Compresses all files in a directory using zstd", long_about = None)]
struct Args {
    /// Path to the input directory
    #[arg(value_name = "DIR")]
    input_dir: PathBuf,

    /// Compression level (1–21)
    #[arg(short, long, default_value_t = 3)]
    level: i32,
}

//#[unsafe(no_mangle)]
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let mut output_vec = Vec::new();
    for entry in WalkDir::new(&args.input_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path().to_path_buf();
        //println!("Compressing: {}", path.display());

        let input_file = File::open(&path)?;
        let reader = BufReader::new(input_file);

        // Output compressed data into Vec<u8>
        let mut encoder = zstd::Encoder::new(&mut output_vec, args.level)?;
        std::io::copy(&mut BufReader::new(reader), &mut encoder)?;
        encoder.finish()?;
    }

    let size = output_vec.len();
    println!("Compressed size: {}", size);
    Ok(())
}
