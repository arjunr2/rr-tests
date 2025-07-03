use clap::{Parser, Args};
use wasmtime::*;

#[derive(Args)]
#[group(required = true, multiple = false)]
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
}


pub fn config_setup_rr(record_path: Option<String>, replay_path: Option<String>) -> Config {
    let mut config = Config::default();
    let rr_cfg = Some (if let Some(path) = &record_path {
        RRConfig::record_cfg(path.clone(), None)
    } else if let Some(path) = &replay_path {
        RRConfig::replay_cfg(path.clone(), None)
    } else {
        panic!("Record or replay has to be specified")
    });
    config.rr(rr_cfg.clone())
        .debug_info(true)
        .cranelift_opt_level(OptLevel::None);
    config
}