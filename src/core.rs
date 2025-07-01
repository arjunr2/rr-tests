use clap::{Parser};
use std::error::Error;
use wasmtime::*;
use common::*;
use imports_core::*;

mod common;
mod imports_core;

fn main() -> Result<(), Box<dyn Error>> {

    let cli = CLI::parse();
    
    let is_replay = cli.rr.replay_path.is_some();
    let config = config_setup_rr(cli.rr.record_path, cli.rr.replay_path);

    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, cli.file)?;
    let mut linker = Linker::new(&engine);
    // Remove the imports for replay
    if is_replay {
        linker.func_wrap("env", "double", stub_double_fn)?
            .func_wrap("env", "complex", stub_complex_fn)?;
    } else {
        linker.func_wrap("env", "double", host_double_fn)?
            .func_wrap("env", "complex", host_complex_fn)?;
    }

    let mut store = Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module)?;

    let run = instance.get_typed_func::<i32, i32>(&mut store, "main")?;
    let input: i32 = 42;
    let result = run.call(&mut store, input)?;

    //assert_eq!(result, input*2);
    println!("Execution produced result: {}", result);
    Ok(())
}

