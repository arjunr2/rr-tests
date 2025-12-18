use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use wirm::Component;

#[derive(Parser)]
struct CLI {
    #[arg(short, long)]
    component: PathBuf,
}

fn main() -> Result<()> {
    let cli = CLI::parse();
    let file = wat::parse_file(&cli.component)?;
    let component = Component::parse(&file, true, true).unwrap();
    println!("{:?}: {:?}!", cli.component, component);
    Ok(())
}
