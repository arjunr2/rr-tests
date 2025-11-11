use std::error::Error;
use wasmtime::*;

use wasmtime_rr_tests::*;

// Import handler implementations
fn host_double_fn(param: i32) -> i32 {
    param * 2
}
fn host_complex_fn(p1: i32, p2: i64) -> (i32, i64, f32) {
    ((p1 as f32).sqrt() as i32, (p1 * p1) as i64 * p2, 8.66)
}

// Stub handlers
//fn stub_double_fn(_param: i32) -> i32 {
//    0
//}
//fn stub_complex_fn(_p1: i32, _p2: i64) -> (i32, i64, f32) {
//    (0, 0, 0.0)
//}

fn main() -> Result<(), Box<dyn Error>> {
    let knobs = record_cli_setup();

    let engine = Engine::new(&knobs.config)?;
    let module = Module::from_file(&engine, knobs.cli_file)?;
    let mut linker = Linker::new(&engine);
    // Remove the imports for replay
    //if is_replay {
    //    linker
    //        .func_wrap("env", "double", stub_double_fn)?
    //        .func_wrap("env", "complex", stub_complex_fn)?;
    //} else {
    linker
        .func_wrap("env", "double", host_double_fn)?
        .func_wrap("env", "complex", host_complex_fn)?;
    //}

    let mut store = Store::new(&engine, ());
    store.init_recording(knobs.buf, knobs.settings)?;

    let instance = linker.instantiate(&mut store, &module)?;

    let run = instance.get_typed_func::<i32, i32>(&mut store, "main")?;
    let input: i32 = 42;
    let result = run.call(&mut store, input)?;

    println!("Execution produced result: {}", result);
    Ok(())
}
