use clap::{Parser, Args};
use wasmtime::*;
use std::fs::File;
use std::io::{BufWriter, BufReader};
use std::sync::Arc;

#[derive(Args)]
#[group(multiple = false)]
pub struct RrCLI {
    #[arg(short = 'c', long = "record")]
    pub record_path: Option<String>,

    #[arg(short = 'p', long = "replay")]
    pub replay_path: Option<String>,
}

#[derive(Parser)]
#[command(version, about = "Test Harness for Record/Replay in Wasmtime", long_about=None)]
pub struct CLI {
    #[arg(short, long, default_value_t = String::from("test.wat"))]
    pub file: String,

    #[command(flatten)]
    pub rr: RrCLI,

    #[arg(short = 's', long = "stub", default_value_t = false)]
    pub stub_imports: bool,

    #[arg(short = 'v', long = "validate", default_value_t = false)]
    pub validate: bool
}


pub fn config_setup_rr(record_path: Option<String>, replay_path: Option<String>, validate: bool) -> Config {
    let mut config = Config::default();
    if let Some(path) = record_path {
        config.enable_record(RecordConfig {
            writer_initializer: Arc::new(move || Box::new(BufWriter::new(File::create(&path).unwrap()))),
            settings: RecordSettings {
                add_validation: validate,
                ..Default::default()
            }
        }).unwrap();
    } else if let Some(path) = replay_path {
        config.enable_replay(ReplayConfig {
            reader_initializer: Arc::new(move || Box::new(BufReader::new(File::open(&path).unwrap()))),
            settings: ReplaySettings {
                validate: validate
            }
        }).unwrap();
    } else {
        panic!("Record or replay not specified");
    };
    config.debug_info(true)
        .cranelift_opt_level(OptLevel::None);
    config
}
