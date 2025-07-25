use clap::{Parser, Args};
use wasmtime::*;
use std::fs::File;
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
    let rr_cfg = if let Some(path) = record_path {
        RRConfig::from(RecordConfig {
            writer_initializer: Arc::new(move || Box::new(File::create(&path).unwrap())),
            metadata: RecordMetadata {
                add_validation: validate
            }
        })
    } else if let Some(path) = replay_path {
        RRConfig::from(ReplayConfig {
            reader_initializer: Arc::new(move || Box::new(File::open(&path).unwrap())),
            metadata: ReplayMetadata {
                validate: validate
            }
        })
    } else {
        panic!("Record or replay not specified");
    };
    config.rr(Some(rr_cfg))
        .debug_info(true)
        .cranelift_opt_level(OptLevel::None);
    config
}