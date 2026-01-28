use anyhow::Result;
use clap::Parser;
use core::panic;
use env_logger;
use std::borrow::Cow;
use std::fs;
use std::path::PathBuf;
use wirm::ir::types::CustomSection;
use wirm::{Component, Module};

macro_rules! unsupported {
    // Single argument: unconditional panic
    ($feature:expr) => {
        panic!("'{}' is not supported yet...", $feature)
    };
    // Two arguments: conditional panic (panics if condition is true)
    ($cond:expr, $feature:expr) => {
        if $cond {
            panic!("'{}' is not supported yet...", $feature)
        }
    };
    // Three or more arguments: conditional panic with formatted message
    ($cond:expr, $feature:expr, $($arg:tt)*) => {
        if $cond {
            panic!("'{}' is not supported yet: {}", $feature, format!($($arg)*))
        }
    };
}

static MODULE_NAME_ERR_MSG: &str = "The module name should always be set for decomposed modules";

mod accessor;
use accessor::ComponentAccessor;

#[derive(Parser)]
struct CLI {
    /// Input component file to decompose.
    #[arg(short, long)]
    component: PathBuf,
    /// Whether to generate output in WAT format (as opposed to binary format).
    #[arg(short = 't', long = "wat")]
    wat: bool,
    /// Overwrite the output directory if it exists.
    #[arg(short = 'x', long = "overwrite")]
    overwrite: bool,
    /// Output directory for decomposed modules from component.
    #[arg(short, long)]
    outdir: PathBuf,
}

/// Decomposed representation of a component into its constituent modules with linking metadata
#[derive(Default)]
struct ComponentDecomposed<'a> {
    modules: Vec<Module<'a>>,
}

#[derive(Debug, Default, Clone)]
struct CrimpReplayMetadata {}

impl<'a> ComponentDecomposed<'a> {
    /// Produce a [ComponentDecomposed] from a [Component]
    fn from_component(component: Component<'a>) -> Self {
        let accessor = ComponentAccessor::from(component);
        accessor.assert_assumptions();

        let mut ret = ComponentDecomposed::default();

        // Populate the modules that the component will be decomposed into
        for (i, mut module) in accessor.module_list().into_iter().enumerate() {
            if module.module_name.is_none() {
                // Assign a default name if none exists
                module.module_name = Some(format!("module_{}", i));
            }
            let _cid = module.custom_sections.add(CustomSection {
                name: "crimp-replay",
                data: Cow::from(b""),
            });
            ret.modules.push(module);
        }

        ret
    }

    fn dump_to_files(self, wat: bool, outdir: &PathBuf) -> Result<()> {
        for mut module in self.modules {
            let bytes = if wat {
                wasmprinter::print_bytes(module.encode())?.into_bytes()
            } else {
                module.encode()
            };
            let mut module_path =
                outdir.join(module.module_name.clone().expect(MODULE_NAME_ERR_MSG));
            if !module_path.add_extension(if wat { "wat" } else { "wasm" }) {
                panic!("Failed to add extension to module path: {:?}", module_path);
            }
            fs::write(module_path, bytes)?;
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = CLI::parse();
    let file = wat::parse_file(&cli.component)?;
    if cli.outdir.exists() {
        fs::remove_dir(&cli.outdir)?;
    }
    fs::create_dir(&cli.outdir)?;
    let component = Component::parse(&file, true, true).unwrap();
    let decomposed = ComponentDecomposed::from_component(component);
    decomposed.dump_to_files(cli.wat, &cli.outdir)?;
    Ok(())
}
