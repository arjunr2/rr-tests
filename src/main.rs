use clap::{Parser, Args};
use std::error::Error;
use wasmtime::*;

#[derive(Args)]
#[group(required = true, multiple = false)]
struct RrCLI {
    #[arg(short = 'c', long = "record")]
    record: Option<String>,

    #[arg(short = 'p', long = "replay")]
    replay: Option<String>,
}

#[derive(Parser)]
#[command(version, about = "Test Harness for Record/Replay in Wasmtime", long_about=None)]
struct CLI {
    #[arg(short, long, default_value_t = String::from("test-modules/double-import.wat"))]
    file: String,

    #[command(flatten)]
    rr: RrCLI,
}

// Import handlers
fn double_fn_impl(param: i32) -> i32 { param * 2 }
fn complex_fn_impl(p1: i32, p2: i64) -> (i32, i64, f32) { ( (p1 as f32).sqrt() as i32, (p1 * p1) as i64 * p2, 8.66 ) }

fn main() -> Result<(), Box<dyn Error>> {

    let cli = CLI::parse();
    
    let mut config = Config::default();
    config.rr(cli.rr.record, cli.rr.replay)
        .debug_info(true)
        .cranelift_opt_level(OptLevel::None);

    let engine = Engine::new(&config)?;
    let module = Module::from_file(&engine, cli.file)?;

    let mut linker = Linker::new(&engine);

    linker.func_wrap("env", "double", double_fn_impl)?;
    linker.func_wrap("env", "complex", complex_fn_impl)?;

    //let data = Log { integers_logged: Vec::new() };
    let mut store = Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module)?;

    let run = instance.get_typed_func::<i32, i32>(&mut store, "main")?;
    let input: i32 = 42;
    let result = run.call(&mut store, input)?;

    //assert_eq!(result, input*2);
    println!("Execution produced result: {}", result);
    Ok(())
}

