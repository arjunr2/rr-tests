use anyhow::Result;
use clap::Parser;
use env_logger;
use std::fmt::Debug;
use std::fs::File;
use std::io::BufWriter;
use wasmtime::component::{
    Component, ComponentNamedList, Instance, Lift, Linker, Lower, ResourceTable,
};
use wasmtime::{Config, Engine, OptLevel, RRConfig, RecordSettings, Store};
use wasmtime_wasi::{self, WasiCtx, WasiCtxView, WasiView};

pub struct MyState {
    ctx: WasiCtx,
    table: ResourceTable,
}
impl WasiView for MyState {
    fn ctx(&mut self) -> wasmtime_wasi::WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

#[derive(Parser)]
#[command(version, about = "Test Harness for Recording Program Execution in Wasmtime", long_about=None)]
pub struct RecordCLI {
    #[arg(short, long, default_value_t = String::from("test.wat"))]
    pub file: String,

    #[arg(short = 'c', long = "record")]
    pub record_path: String,

    #[arg(short = 'v', long = "validate", default_value_t = false)]
    pub validate: bool,
}

/// TODO: Remove all replay stuff once we have a replay driver
pub struct Knobs<R, S> {
    pub config: Config,
    pub buf: R,
    pub settings: S,
    pub cli_file: String,
}

pub fn record_cli_setup() -> Knobs<BufWriter<File>, RecordSettings> {
    env_logger::init();

    let cli = RecordCLI::parse();

    let (record_path, validate) = (cli.record_path, cli.validate);

    // Config
    let mut config = Config::default();
    config
        .debug_info(true)
        .cranelift_opt_level(OptLevel::None)
        .rr(RRConfig::Recording);

    Knobs {
        config,
        buf: BufWriter::new(File::create(&record_path).unwrap()),
        settings: RecordSettings {
            add_validation: validate,
            ..Default::default()
        },
        cli_file: cli.file.clone(),
    }
}

pub enum ComponentFmt<'a> {
    File(&'a str),
    Raw(&'a str),
}

pub enum RunMode<'a, Params, T>
where
    Params: ComponentNamedList + Lower,
    T: FnOnce(Store<MyState>, Linker<MyState>, Component) -> Result<()>,
{
    InstantiateAndCallOnce { name: &'a str, params: Params },
    InstantiateOnly,
    Custom(T),
}

pub type RunTy = fn(Store<MyState>, Linker<MyState>, Component) -> Result<()>;

pub fn component_run<'a, L, T, Params, Results>(
    cfmt: ComponentFmt<'a>,
    l: L,
    mode: RunMode<'a, Params, T>,
) -> Result<()>
where
    L: FnOnce(&mut Linker<MyState>) -> Result<()>,
    T: FnOnce(Store<MyState>, Linker<MyState>, Component) -> Result<()>,
    Params: ComponentNamedList + Lower,
    Results: ComponentNamedList + Lift + Debug,
{
    let knobs = record_cli_setup();

    let engine = Engine::new(&knobs.config)?;
    let component = match cfmt {
        // Don't use CLI.file for components since it's static anyway
        ComponentFmt::File(s) => Component::from_file(&engine, s)?,
        ComponentFmt::Raw(s) => Component::new(&engine, s)?,
    };

    let mut linker = Linker::<MyState>::new(&engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)?;

    let mut wasi_builder = WasiCtx::builder();
    let state = MyState {
        ctx: wasi_builder.build(),
        table: ResourceTable::new(),
    };

    // Linker setup
    l(&mut linker)?;

    // Store setup
    let mut store = Store::new(&engine, state);
    store.init_recording(knobs.buf, knobs.settings)?;

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
    mut store: Store<MyState>,
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
    mut store: Store<MyState>,
    linker: Linker<MyState>,
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
