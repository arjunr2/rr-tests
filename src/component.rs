use clap::{Parser};
use std::error::Error;
use wasmtime::*;
use common::*;
use imports_component::complex;

mod common;
mod imports_component;

fn main() -> Result<(), Box<dyn Error>> {

    let cli = CLI::parse();
    
    let is_replay = cli.rr.replay_path.is_some();
    let config = config_setup_rr(cli.rr.record_path, cli.rr.replay_path);

    let engine = Engine::new(&config)?;
    let component = component::Component::from_file(&engine, cli.file)?;

    let mut linker = component::Linker::new(&engine);
    complex::Root::add_to_linker::<_, component::HasSelf<_>>(&mut linker, |state| state)?;
    // Remove the imports for replay

    let mut store = Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &component)?;

    let func = instance.get_typed_func::<(u32,), (u32,)>(&mut store, "main").expect("main export not found"); 
    let input = (42,);
    let result = func.call(&mut store, input)?;

    println!("Execution produced result: {:?}", result);
    Ok(())
}

