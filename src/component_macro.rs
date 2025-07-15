#[macro_export]
macro_rules! bin {
    ($bin:ident, $str_world: literal in $path: literal, $file: literal, $rs_world:ident) => (
        use clap::{Parser};
        use std::error::Error;
        use wasmtime::*;
        use wasmtime::component::{Component, Linker, HasSelf, bindgen};
        use common::*;

        mod common;

        bindgen!($str_world in $path);

        fn main() -> Result<(), Box<dyn Error>> {

            let cli = CLI::parse();
            
            let stub_imports = if cli.rr.replay_path.is_some() {
                cli.stub_imports
            } else {
                false
            };
            let config = config_setup_rr(cli.rr.record_path, cli.rr.replay_path);

            let engine = Engine::new(&config)?;
            // Don't use CLI.file for components since it's static anyway
            let component = Component::from_file(&engine, $file)?;

            let mut linker = Linker::new(&engine);
            // Remove the imports for replay
            if stub_imports {
                println!("Stubbing out all imports...");
                linker.define_unknown_imports_as_traps(&component)?;
            } else {
                $rs_world::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)?;
            }

            let mut store = Store::new(&engine, ());
            let instance = linker.instantiate(&mut store, &component)?;

            let func = instance.get_typed_func::<(u32,), (u32,)>(&mut store, "main").expect("main export not found"); 
            let input = (42,);
            let result = func.call(&mut store, input)?;
            // // Untyped
            //let func = instance.get_func(&mut store, "main").expect("main export not found"); 
            //let input = [component::Val::U32(42)];
            //let mut result = [component::Val::S32(0)];
            //let _  = func.call(&mut store, &input, &mut result)?;

            println!("Execution produced result: {:?}", result);
            Ok(())
        }
    );
}