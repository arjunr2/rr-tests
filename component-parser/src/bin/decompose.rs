//! CLI tool to decompose a WebAssembly Component into its constituent modules.

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use std::cell::Ref;
use std::fs;
use std::path::PathBuf;

use component_parser::Component;
use component_parser::ir::{ComponentInstanceNode, ResolvedModule};
use component_parser::parse_component;
use component_parser::wasmparser::Validator;
use component_parser::wirm::Module;

macro_rules! unsupported {
    // Single argument: unconditional error
    ($feature:expr) => {
        Err(anyhow!("'{}' is not supported yet...", $feature))
    };
    // Two arguments: conditional error (returns Err if condition is true)
    ($cond:expr, $feature:expr) => {
        if $cond {
            Err(anyhow!("'{}' is not supported yet...", $feature))
        } else {
            Ok(())
        }
    };
}

#[derive(Parser, Debug)]
#[command(name = "decompose")]
#[command(about = "Decompose a WebAssembly Component into its modules")]
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

impl<'a> ComponentDecomposed<'a> {
    fn validate_assumptions(component: Ref<'a, Component<'a>>) -> Result<()> {
        // Currently no assumptions to assert
        unsupported!(
            component.components.is_empty(),
            "Component has nested components"
        )?;

        for x in component.instances.iter() {
            let y = ComponentInstanceNode::resolve(component, x);
            match x {
                ComponentInstanceNode::Imported(_) => {}
                _ 
            }
        }
        //unsupported!(
        //    !self.component_instances.is_empty(),
        //    "Main component instances"
        //);
        Ok(())
    }

    /// Validate all modules in the decomposed representation.
    fn validate_modules(&self) -> Result<()> {
        for module in &self.modules {
            Validator::new()
                .validate_all(&module.encode())
                .with_context(|| "Module validation failed")?;
        }
        Ok(())
    }

    /// Produce a [ComponentDecomposed] from a [Component]
    fn from_component(component: Ref<'a, Component<'a>>) -> Result<Self> {
        Self::validate_assumptions(component)?;

        let mut metadata = CrimpReplayMetadata::default();

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
            metadata.modules.push(module);
        }

        accessor.instantiate_commands()?;

        Ok(ComponentDecomposed {
            modules: metadata.modules,
        })
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
    let cli = CLI::parse();
    let file = wat::parse_file(&cli.component)?;

    // Validate with wasmparser
    Validator::new()
        .validate_all(&file)
        .with_context(|| "Validation failed")?;

    let component_rc = parse_component(&file).with_context(|| "Failed to parse component")?;
    let component = component_rc.borrow();
    println!("{:?}", component);

    if cli.outdir.exists() {
        fs::remove_dir(&cli.outdir)?;
    }
    fs::create_dir(&cli.outdir)?;

    let decomposed = ComponentDecomposed::from_component(component)?;
    decomposed.dump_to_files(cli.wat, &cli.outdir)?;
    Ok(())

    //for (idx, _module_node) in component_ref.modules.iter() {
    //    let resolved = component_ref.resolve_module(idx);
    //    match resolved {
    //        ResolvedModule::Defined { mut module } => {
    //            // Re-encode the module from the parsed IR
    //            let encoded_bytes = module.encode();

    //            let filename = if cli.wat {
    //                format!("module_{}.wat", idx)
    //            } else {
    //                format!("module_{}.wasm", idx)
    //            };
    //            let output_path = cli.outdir.join(&filename);

    //            if cli.wat {
    //                // Convert to WAT format
    //                let wat_string = wasmprinter::print_bytes(&encoded_bytes)?;
    //                fs::write(&output_path, wat_string)?;
    //            } else {
    //                fs::write(&output_path, &encoded_bytes)?;
    //            }

    //            //println!("Module writing: {:?}", module);
    //        }
    //        ResolvedModule::Imported(resolved_import) => {
    //            println!(
    //                "Module {} is imported as '{}', skipping",
    //                idx, resolved_import.name
    //            );
    //        }
    //    }
    //}
}
