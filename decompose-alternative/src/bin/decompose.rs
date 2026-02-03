//! CLI tool to decompose a WebAssembly Component into its constituent modules.

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use wasmparser::Validator;

use decompose_alternative::ir::ResolvedModule;
use decompose_alternative::parse_component;

#[derive(Parser, Debug)]
#[command(name = "decompose")]
#[command(about = "Decompose a WebAssembly Component into its modules")]
struct CLI {
    /// Input component file to decompose.
    #[arg(short, long)]
    component: PathBuf,
    /// Whether to generate output in WAT format (as opposed to binary format).
    #[arg(short = 't', long = "wat")]
    wat: bool,
    /// Overwrite the output directory if it exists.
    #[arg(short = 'x', long = "overwrite")]
    overwrite: bool,
    /// Output directory for decomposed modules from component.
    #[arg(short, long)]
    outdir: PathBuf,
}

fn main() -> Result<()> {
    let cli = CLI::parse();
    let file = wat::parse_file(&cli.component)?;

    // Validate with wasmparser
    Validator::new()
        .validate_all(&file)
        .with_context(|| "Validation failed")?;

    let component = parse_component(&file).with_context(|| "Failed to parse component")?;

    if cli.outdir.exists() {
        fs::remove_dir(&cli.outdir)?;
    }
    fs::create_dir(&cli.outdir)?;

    // Extract and write modules
    let component_ref = component.borrow();
    let mut module_count = 0;

    for (idx, _module_node) in component_ref.modules.iter() {
        let resolved = component_ref.resolve_module(idx);
        match resolved {
            ResolvedModule::Defined { module } => {
                // Re-encode the module from the parsed IR
                let encoded_bytes = module.encode();

                let filename = if cli.wat {
                    format!("module_{}.wat", idx)
                } else {
                    format!("module_{}.wasm", idx)
                };
                let output_path = cli.outdir.join(&filename);

                if cli.wat {
                    // Convert to WAT format
                    let wat_string = wasmprinter::print_bytes(&encoded_bytes)?;
                    fs::write(&output_path, wat_string)?;
                } else {
                    fs::write(&output_path, &encoded_bytes)?;
                }

                println!("Module writing: {:?}", module);
                module_count += 1;
            }
            ResolvedModule::Imported { name, .. } => {
                println!("Module {} is imported as '{}', skipping", idx, name);
            }
        }
    }

    println!("\nDecomposed {} modules to {:?}", module_count, cli.outdir);

    Ok(())
}
