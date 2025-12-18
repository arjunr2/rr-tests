use anyhow::{Result, bail};
use clap::Parser;
use env_logger;
use std::fs::File;
use std::io::BufReader;
use wasmtime::component::Component;
use wasmtime::{Config, Engine, OptLevel, RRConfig, ReplayEnvironment, ReplaySettings};

use wasmtime_rr_tests::Knobs;

#[derive(Parser)]
#[command(version, about = "Replay Harness for Wasmtime RR Traces", long_about=None)]
pub struct ReplayCLI {
    #[arg(short, long, default_value_t = String::from("test.wat"))]
    pub file: String,

    #[arg(short = 'c', long = "replay")]
    pub replay_path: String,

    #[arg(short = 'v', long = "validate", default_value_t = false)]
    pub validate: bool,
}

fn replay_cli_setup() -> Knobs<BufReader<File>, ReplaySettings> {
    env_logger::init();

    let cli = ReplayCLI::parse();
    let (replay_path, validate) = (cli.replay_path, cli.validate);

    // Config
    let mut config = Config::default();
    config
        .debug_info(true)
        .cranelift_opt_level(OptLevel::None)
        .rr(RRConfig::Replaying);

    Knobs {
        config,
        buf: BufReader::new(File::open(&replay_path).unwrap()),
        settings: ReplaySettings {
            validate: validate,
            deserialize_buffer_size: 1024,
        },
        cli_file: cli.file.clone(),
    }
}

fn main() -> Result<()> {
    let knobs = replay_cli_setup();
    let config = knobs.config;
    let engine = Engine::new(&config)?;
    let mut renv = ReplayEnvironment::new(&engine, knobs.settings);
    if let Ok(component) = Component::from_file(&engine, &knobs.cli_file) {
        renv.add_component(component);
    } else if let Ok(module) = wasmtime::Module::from_file(&engine, &knobs.cli_file) {
        renv.add_module(module);
    } else {
        bail!(
            "{} file provided is neither a Wasm component nor a module",
            &knobs.cli_file
        );
    }
    let mut instance = renv.instantiate(knobs.buf)?;
    instance.run_to_completion()?;
    println!("Replay completed successfully");
    Ok(())
}
