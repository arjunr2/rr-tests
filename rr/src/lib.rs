use anyhow::Result;
use clap::Parser;
use env_logger;
use std::fmt::Debug;
use std::fs::File;
use std::io::BufWriter;
use wasmtime::component::{Component, ComponentNamedList, Instance, Lift, Linker, Lower};
use wasmtime::{
    Config, Engine, OptLevel, RecordSettings, RecordWriter, ReplayReader, ReplaySettings, Store,
};

#[derive(Parser)]
#[command(version, about = "Test Harness for Replaying a Trace in Wasmtime", long_about=None)]
pub struct ReplayCLI {
    #[arg(short, long, default_value_t = String::from("test.wat"))]
    pub file: String,

    #[arg(short = 'p', long = "replay")]
    pub replay_path: Option<String>,

    #[arg(short = 's', long = "stub", default_value_t = false)]
    pub stub_imports: bool,

    #[arg(short = 'v', long = "validate", default_value_t = false)]
    pub validate: bool,
}

#[derive(Parser)]
#[command(version, about = "Test Harness for Recording Program Execution in Wasmtime", long_about=None)]
pub struct RecordCLI {
    #[arg(short, long, default_value_t = String::from("test.wat"))]
    pub file: String,

    #[arg(short = 'c', long = "record")]
    pub record_path: Option<String>,

    #[arg(short = 'v', long = "validate", default_value_t = false)]
    pub validate: bool,
}

pub struct RecordKnobs<R: RecordWriter> {
    pub config: Config,
    pub record: Option<(R, RecordSettings)>,
    pub cli_file: String,
}

pub struct ReplayKnobs<R: ReplayReader> {
    pub config: Config,
    pub replay: Option<(R, ReplaySettings)>,
    pub stub_imports: bool,
    pub cli_file: String,
}

pub fn cli_setup() -> RecordKnobs<BufWriter<File>> {
    env_logger::init();

    let cli = RecordCLI::parse();

    let (record_path, validate) = (cli.record_path, cli.validate);

    // Config
    let mut config = Config::default();
    config
        .recording(true)
        .debug_info(true)
        .cranelift_opt_level(OptLevel::None);

    // Record
    let record = record_path.and_then(|path| {
        Some((
            BufWriter::new(File::create(&path).unwrap()),
            RecordSettings {
                add_validation: validate,
                ..Default::default()
            },
        ))
    });

    let cli_file = cli.file.clone();
    RecordKnobs {
        config,
        record,
        cli_file,
    }
}

pub enum ComponentFmt<'a> {
    File(&'a str),
    Raw(&'a str),
}

pub enum RunMode<'a, Params, T>
where
    Params: ComponentNamedList + Lower,
    T: FnOnce(Store<()>, Linker<()>, Component) -> Result<()>,
{
    InstantiateAndCallOnce { name: &'a str, params: Params },
    InstantiateOnly,
    Custom(T),
}

pub type RunTy = fn(Store<()>, Linker<()>, Component) -> Result<()>;

pub fn component_run<'a, L, T, Params, Results>(
    cfmt: ComponentFmt<'a>,
    l: L,
    mode: RunMode<'a, Params, T>,
) -> Result<()>
where
    L: FnOnce(&mut Linker<()>) -> Result<()>,
    T: FnOnce(Store<()>, Linker<()>, Component) -> Result<()>,
    Params: ComponentNamedList + Lower,
    Results: ComponentNamedList + Lift + Debug,
{
    let knobs = cli_setup();

    let engine = Engine::new(&knobs.config)?;
    let component = match cfmt {
        // Don't use CLI.file for components since it's static anyway
        ComponentFmt::File(s) => Component::from_file(&engine, s)?,
        ComponentFmt::Raw(s) => Component::new(&engine, s)?,
    };

    let mut linker = Linker::new(&engine);
    //// Remove the imports for replay
    //if knobs.stub_imports {
    //    println!("Stubbing out all imports...");
    //    linker.define_unknown_imports_as_traps(&component)?;
    //} else {
    l(&mut linker)?;
    //}
    let mut store = Store::new(&engine, ());

    if let Some((writer, settings)) = knobs.record {
        store.init_recording(writer, settings)?;
    }

    match mode {
        RunMode::InstantiateAndCallOnce { name, params } => {
            let result =
                instantiate_and_call_once::<_, Results>(store, linker, component, name, params)?;
            println!("Call produced result: {:?}", result);
        }
        RunMode::InstantiateOnly => {
            let _ = linker.instantiate(&mut store, &component)?;
        }
        RunMode::Custom(x) => x(store, linker, component)?,
    }
    Ok(())
}

pub fn call_once<Params, Results>(
    mut store: Store<()>,
    instance: Instance,
    name: &str,
    params: Params,
) -> Result<Results>
where
    Params: ComponentNamedList + Lower,
    Results: ComponentNamedList + Lift,
{
    let func = instance
        .get_typed_func::<Params, Results>(&mut store, name)
        .expect(&format!("{} export not found", name));
    Ok(func.call(&mut store, params)?)
}

pub fn instantiate_and_call_once<Params, Results>(
    mut store: Store<()>,
    linker: Linker<()>,
    component: Component,
    name: &str,
    params: Params,
) -> Result<Results>
where
    Params: ComponentNamedList + Lower,
    Results: ComponentNamedList + Lift,
{
    let instance = linker.instantiate(&mut store, &component)?;
    call_once::<Params, Results>(store, instance, name, params)
}

pub mod component_macro;
