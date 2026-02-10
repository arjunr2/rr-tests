//! CLI tool to decompose a WebAssembly Component into its constituent modules.

use anyhow::{Context, Result, anyhow};
use clap::Parser;
use env_logger;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use wirm::wasmparser::{
    CanonicalOption, ComponentExternalKind, ExternalKind, InstantiationArgKind,
};

use component_parser::Component;
use component_parser::ir::{
    CoreInstanceNode, Export, Resolve, ResolvedComponentFunc, ResolvedComponentInstance,
    ResolvedCoreFunc, ResolvedCoreInstance, ResolvedModule,
};
use component_parser::parse_component;
use component_parser::wasmparser::Validator;
use component_parser::wirm::Module;
use component_parser::wirm::ir::types::CustomSection;

mod linking;
use linking::*;

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

impl CanonicalOptionsIndex {
    /// Indexes the options for a canonical function within the module's IR
    pub fn from_options<'a>(
        component: &Component<'a>,
        options: &[CanonicalOption],
    ) -> Option<Self> {
        fn func_resolve<'a>(component: &Component<'a>, func_idx: u32) -> ModuleExport {
            match component.resolve_core_func(func_idx) {
                ResolvedCoreFunc::FromModule {
                    module_idx,
                    func_idx,
                } => {
                    let module_link_id = ModuleLinkID(module_idx);
                    let export_name = get_export_name_from_kind_idx(
                        component,
                        module_idx,
                        vec![ExternalKind::Func, ExternalKind::FuncExact],
                        func_idx,
                    );
                    ModuleExport(module_link_id, export_name)
                }
                _ => panic!("Canonical options core references can only come FromModule"),
            }
        }

        let mut opts_ref = CanonicalOptionsIndex::default();
        for opt in options {
            match opt {
                //CanonicalOption::Memory(memory_idx) => opts_ref.memory = Some(name.clone()),
                CanonicalOption::Realloc(func_idx) => {
                    opts_ref.realloc = Some(func_resolve(component, *func_idx));
                }
                CanonicalOption::PostReturn(func_idx) => {
                    opts_ref.post_return = Some(func_resolve(component, *func_idx));
                }
                CanonicalOption::Memory(memory_idx) => {
                    opts_ref.memory = Some(ModuleExport(
                        ModuleLinkID(*memory_idx),
                        get_export_name_from_kind_idx(
                            component,
                            *memory_idx,
                            vec![ExternalKind::Memory],
                            *memory_idx,
                        ),
                    ));
                }
                CanonicalOption::UTF8 | CanonicalOption::UTF16 => {
                    // These options are implicitly capturing in recording, so do nothing
                }
                _ => panic!("Canonical option variant not supported yet: {:?}", opt),
            }
        }
        (!options.is_empty()).then_some(opts_ref)
    }
}

/// Validate assumptions about the component that must hold for decomposition to be valid
///
/// Relax these as we build out this tool. Currently, we stop the following:
/// * Main component instances
/// * Nested components
///
/// Note: Imports can still use things like component, module. We are not testing for
/// full recursive enforcement of these assumptions.
fn validate_assumptions<'a>(component: &Component<'a>) -> Result<()> {
    unsupported!(!component.components.is_empty(), "Nested components")?;

    for inst in component.instances.iter_resolved(component) {
        match inst {
            ResolvedComponentInstance::Imported(_) => {}
            _ => {
                unsupported!("Main, inline component instances")?;
            }
        }
    }

    for module in component.modules.iter_resolved(&component) {
        match module {
            ResolvedModule::Imported { .. } => {
                unsupported!("Imported modules")?;
            }
            _ => {}
        }
    }

    Ok(())
}

pub(crate) fn get_export_name_from_kind_idx(
    component: &Component,
    module_idx: u32,
    kinds: Vec<ExternalKind>,
    kind_idx: u32,
) -> String {
    // This is safe since we assume no imported modules for now
    let link_module = component.resolve_module(module_idx).defined();
    let export = link_module
        .exports
        .iter()
        .find(|export| kinds.contains(&export.kind) && export.index == kind_idx)
        .expect(
            format!(
                "Export {:?}, {:?} should be found in module {:?}",
                kinds, kind_idx, module_idx,
            )
            .as_str(),
        );
    export.name.clone()
}

/// Gather linking information for a single `InstantiationArg` into `import_md`
fn gather_instance_link(
    import_md: &mut ImportMetadata,
    mut member_imports: HashMap<String, CoreImportIndex>,
    component: &Component,
    instance_exports: &Vec<Export>,
) -> Result<()> {
    for export in instance_exports.iter() {
        let core_import_idx: CoreImportIndex = member_imports
            .remove(&export.name.to_string())
            .expect("export should be matched by an import")
            .into();
        match export.kind {
            ExternalKind::Func => {
                let core_func = component.resolve_core_func(export.index);
                match core_func {
                    ResolvedCoreFunc::Lowered { func_idx, options } => {
                        let comp_func = component.resolve_component_func(func_idx);
                        log::trace!(
                            "CoreFunc[{:?}] lowered from ComponentFunc[{:?}] with options {:?}",
                            export.index,
                            comp_func,
                            options
                        );
                        match comp_func {
                            ResolvedComponentFunc::Imported { .. } => {
                                import_md.true_imports.insert(
                                    core_import_idx,
                                    CanonicalOptionsIndex::from_options(&component, &options),
                                );
                            }
                            ResolvedComponentFunc::Lifted { .. } => {
                                panic!("Lowered CoreFunc should not come from lifted ComponentFunc")
                            }
                        }
                    }
                    ResolvedCoreFunc::FromModule {
                        module_idx,
                        func_idx,
                    } => {
                        log::trace!(
                            "CoreFunc[{:?}] from module {:?} func idx {:?}",
                            export.index,
                            module_idx,
                            func_idx
                        );
                        // This is safe since we assume no imported modules for now
                        let export_name = get_export_name_from_kind_idx(
                            component,
                            module_idx,
                            vec![ExternalKind::Func, ExternalKind::FuncExact],
                            func_idx,
                        );
                        import_md
                            .member_renames
                            .insert(core_import_idx, export_name);
                        // The module_idx is being used for the module ID for now since we don't have nested/imported modules
                        import_md
                            .package_renames
                            .insert(core_import_idx, ModuleLinkID(module_idx));
                    }
                    ResolvedCoreFunc::ResourceDrop { .. } => {
                        log::trace!("CoreFunc[{:?}] is a resource drop", export.index);
                        import_md.builtins.insert(core_import_idx);
                    }
                }
            }
            ExternalKind::Table => {
                let table = component.resolve_core_table(export.index);
                log::trace!("CoreTable resolved to {:?}", table);
                let export_name = get_export_name_from_kind_idx(
                    component,
                    table.module_idx,
                    vec![ExternalKind::Table],
                    table.table_idx,
                );
                import_md
                    .member_renames
                    .insert(core_import_idx, export_name);
                import_md
                    .package_renames
                    .insert(core_import_idx, ModuleLinkID(table.module_idx));
            }
            ExternalKind::Memory => {
                let memory = component.resolve_core_memory(export.index);
                log::trace!("CoreMemory resolved to {:?}", memory);
                let export_name = get_export_name_from_kind_idx(
                    component,
                    memory.module_idx,
                    vec![ExternalKind::Memory],
                    memory.memory_idx,
                );
                import_md
                    .member_renames
                    .insert(core_import_idx, export_name);
                import_md
                    .package_renames
                    .insert(core_import_idx, ModuleLinkID(memory.module_idx));
            }
            _ => {
                unsupported!(format!("Linking of export kind {:?}", export.kind))?;
            }
        }
    }
    assert!(
        member_imports.is_empty(),
        "All imports should be matched by exports"
    );
    Ok(())
}

/// Gather exported functions from the component and return them
///
/// TODO: Change when handling component instances - need to consider exports from nested components as well.
/// Right now, we can simply use the `export_id` from enumerating exports, but this will change when we have exports from instances
fn gather_component_exports(
    export_funcs: &mut HashMap<ModuleLinkID, Vec<ExportFuncMetadata>>,
    component: &Component,
) -> Result<()> {
    for (export_id, export) in component.exports.iter().enumerate() {
        match export.kind {
            ComponentExternalKind::Func => match component.resolve_component_func(export.index) {
                ResolvedComponentFunc::Imported(_) => {
                    unsupported!("Export of imported component functions")?;
                }
                ResolvedComponentFunc::Lifted {
                    core_func_idx,
                    type_idx: _type_idx,
                    options,
                } => {
                    let core_func = component.resolve_core_func(core_func_idx);
                    match core_func {
                        ResolvedCoreFunc::FromModule {
                            module_idx,
                            func_idx,
                        } => {
                            export_funcs
                                .entry(ModuleLinkID(module_idx))
                                .or_default()
                                .push(ExportFuncMetadata {
                                    record_id: RecordExportIndex(export_id as u32),
                                    name: get_export_name_from_kind_idx(
                                        component,
                                        module_idx,
                                        vec![ExternalKind::Func, ExternalKind::FuncExact],
                                        func_idx,
                                    ),
                                    opts: CanonicalOptionsIndex::from_options(&component, &options),
                                });
                        }
                        _ => {
                            unsupported!(
                                "Lifted ComponentFunc sourced from non-FromModule CoreFuncs"
                            )?;
                        }
                    }
                }
            },

            _ => {
                log::warn!(
                    "Export kind from {:?} is not supported for access yet..",
                    export
                );
            }
        }
    }
    log::trace!("Gathered export funcs: {:?}", export_funcs);
    Ok(())
}

/// Construct the [`ModuleLinkingMetadata`] for the component
fn linking_metadata<'a>(
    component: &Component<'a>,
    checksum: Checksum,
) -> Result<LinkingMetadata<'a>> {
    // Keep track of synthetic export instances
    let mut synthetic_core_instances_exports = HashMap::<u32, Vec<Export>>::new();
    let mut linking = LinkingMetadata {
        checksum,
        ..Default::default()
    };
    // Only needs to handle core instances for now
    for (instance_id, instance) in component.core_instances.iter().enumerate() {
        if let CoreInstanceNode::Aliased(alias) = instance {
            unsupported!(format!("Aliased core instance: {:?}", alias))?;
        }
        match instance.resolve(&component) {
            ResolvedCoreInstance::FromExports(exports) => {
                synthetic_core_instances_exports.insert(instance_id as u32, exports);
            }
            ResolvedCoreInstance::Instantiated { module_idx, args } => {
                let module_link_id = ModuleLinkID(module_idx);
                linking.mm.entry(module_link_id).or_insert_with(|| {
                    // Populate import map for the module being instantiated
                    let mut metadata = ModuleMetadata {
                        module: component.resolve_module(module_idx).defined(),
                        import_map: HashMap::new(),
                    };
                    for (i, import) in metadata.module.imports.iter().enumerate() {
                        let members = metadata
                            .import_map
                            .entry(import.module.as_ref().to_owned())
                            .or_default();
                        members.insert(import.name.to_string(), CoreImportIndex(i as u32));
                    }
                    metadata
                });

                // Gather linking information from args
                let mut expected_imports = linking
                    .mm
                    .get(&module_link_id)
                    .expect("Module should be already defined")
                    .import_map
                    .clone();
                assert_eq!(args.len(), expected_imports.len());
                log::debug!(
                    "Linking for CoreInstance[{:?}] from Module[{:?}]",
                    instance_id,
                    module_idx
                );
                let mut instance_metadata = InstanceLinkingMetadata {
                    module_link_id,
                    // Works for now since we only consider core instances
                    instantiate_order: instance_id as u32,
                    import_md: ImportMetadata::default(),
                };
                for arg in args {
                    // Ensure no new kinds of instantiation args are introduced
                    match arg.kind {
                        InstantiationArgKind::Instance => {}
                    };
                    // Get the export for instance providing 'arg.name' package
                    let instance_exports = synthetic_core_instances_exports
                        .get(&arg.index)
                        .expect("exported core instance should be already populated");
                    // Get imports for 'arg.name' package
                    let member_imports = expected_imports
                        .remove(arg.name)
                        .expect("import should be populated");
                    gather_instance_link(
                        &mut instance_metadata.import_md,
                        member_imports,
                        &component,
                        &instance_exports,
                    )?;
                }
                log::info!(
                    "Instantiated CoreInstance[{:?}]: {:?}",
                    instance_id,
                    instance_metadata
                );
                linking.instances.push(instance_metadata);
            }
        }
    }

    gather_component_exports(&mut linking.export_funcs, &component)?;
    Ok(linking)
}

/// Decomposed representation of a component into its constituent modules with linking metadata
#[derive(Default)]
struct ComponentDecomposed<'a> {
    modules: Vec<Module<'a>>,
}

impl<'a> ComponentDecomposed<'a> {
    /// Validate all modules in the decomposed representation.
    fn validate_modules(&self) -> Result<()> {
        for module in &self.modules {
            Validator::new()
                .validate_all(&module.encode())
                .with_context(|| "Module validation failed")?;
        }
        Ok(())
    }

    fn from_linking_metadata(mut linking: LinkingMetadata<'a>) -> Result<Self> {
        assert_eq!(
            linking.mm.len(),
            linking.instances.len(),
            "Each module should be instantiated exactly once for now"
        );

        let instantiated_modules = linking.mm.keys().collect::<HashSet<_>>();
        let export_func_modules = linking.export_funcs.keys().collect::<HashSet<_>>();
        assert!(
            export_func_modules.is_subset(&instantiated_modules),
            "Exported functions should only come from instantiated modules"
        );

        linking
            .mm
            .iter_mut()
            .for_each(|(id, md)| md.module.module_name = Some(format!("module_{}", **id as usize)));

        let modules = linking
            .mm
            .iter()
            .map(|(module_id, module_metadata)| {
                let crimp = linking.serialize_crimp_section(*module_id)?;
                let mut module = module_metadata.module.clone();
                let _cid = module.custom_sections.add(CustomSection {
                    name: "crimp-replay",
                    data: Cow::from(crimp),
                });
                Ok(module)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self { modules })
    }

    /// Produce a [ComponentDecomposed] from a [Component]
    fn from_component(
        component_rc: Rc<RefCell<Component<'a>>>,
        checksum: Checksum,
    ) -> Result<Self> {
        let component = component_rc.borrow();
        validate_assumptions(&component)?;
        let lm = linking_metadata(&component, checksum)?;
        let decomposed = Self::from_linking_metadata(lm)?;
        decomposed.validate_modules()?;
        Ok(decomposed)
    }

    fn dump_to_files(self, wat: bool, outdir: &PathBuf) -> Result<()> {
        for module in self.modules {
            let bytes = if wat {
                wasmprinter::print_bytes(module.encode())?.into_bytes()
            } else {
                module.encode()
            };
            let mut module_path = outdir.join(
                module
                    .module_name
                    .clone()
                    .expect("The module name should always be set for decomposed modules"),
            );
            if !module_path.add_extension(if wat { "wat" } else { "wasm" }) {
                panic!("Failed to add extension to module path: {:?}", module_path);
            }
            log::info!("Writing module: {:?}", module_path);
            fs::write(module_path, bytes)?;
        }
        Ok(())
    }
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = CLI::parse();
    let file = wat::parse_file(&cli.component)?;

    // Validate with wasmparser
    Validator::new()
        .validate_all(&file)
        .with_context(|| "Validation failed")?;

    let checksum: Checksum = Sha256::digest(&file).as_slice().try_into().unwrap();
    let component_rc = parse_component(&file).with_context(|| "Failed to parse component")?;

    if cli.outdir.exists() {
        fs::remove_dir(&cli.outdir)?;
    }
    fs::create_dir(&cli.outdir)?;

    let decomposed = ComponentDecomposed::from_component(component_rc, checksum)?;
    decomposed.dump_to_files(cli.wat, &cli.outdir)?;
    Ok(())
}
