use clap::{Parser};
use wasmtime::*;

#[derive(Parser)]
#[command(version, about = "Test Harness for Record/Replay in Wasmtime", long_about=None)]
pub struct CLI {
    #[arg(short, long, default_value_t = String::from("test.wat"))]
    pub file: String,

    #[arg(short, long, default_value_t = false)]
    pub determinism: bool,
}


pub fn config_setup(cli: &CLI) -> Config {
    let mut config = Config::default();
    if cli.determinism {
        config.debug_info(true)
            .cranelift_opt_level(OptLevel::None);
    }
    config
}