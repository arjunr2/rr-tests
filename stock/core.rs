use clap::{Parser};
use std::error::Error;
use wasmtime::*;
use common::*;
use imports_core::*;

mod common;
mod imports_core;

fn main() -> Result<(), Box<dyn Error>> {

    let cli = CLI::parse();
    
    let config = config_setup(&cli);

    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, cli.file)?;
    let mut linker = Linker::new(&engine);
    linker.func_wrap("env", "double", host_double_fn)?
        .func_wrap("env", "complex", host_complex_fn)?;

    let mut store = Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module)?;

    let run = instance.get_typed_func::<i32, i32>(&mut store, "main")?;
    let input: i32 = 42;
    let result = run.call(&mut store, input)?;

    //assert_eq!(result, input*2);
    println!("Execution produced result: {}", result);
    Ok(())
}

