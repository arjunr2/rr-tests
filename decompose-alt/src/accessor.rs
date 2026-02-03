use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use wirm::ir::component::idx_spaces::{
    Depth, IndexSpaceOf, IndexedRef, ReferencedIndices, Space, SpaceSubtype,
};
use wirm::ir::id::ImportsID;
use wirm::wasmparser::{Export, ExternalKind, Instance, InstantiationArgKind};
use wirm::{Component, Module};

mod spaces;

pub use spaces::{ComponentTypeRefResolved, ComponentTypeSource, CoreInstanceSource, Resolve};

use crate::accessor::spaces::{
    CoreFuncSource, CoreGlobalSource, CoreInstanceExportSource, CoreMemorySource, CoreModuleSource,
    CoreTableSource, CoreTagSource, ToSource,
};

#[derive(Debug)]
struct ModuleMetadata<'a> {
    pub module: &'a Module<'a>,
    pub import_map: HashMap<&'a str, HashMap<&'a str, ImportsID>>,
}

#[derive(Debug)]
struct ImportRenames<'a> {
    /// Renames for packages imported by this module with the module (id: u32) providing the export
    pub packages: HashMap<ImportsID, u32>,
    /// Renames for the members imported by this module at idx (u32) with the member name
    /// in the module providing the export
    pub members: HashMap<ImportsID, &'a str>,
}

/// Metadata needed to capture the linking information for a module for CRIMP
#[derive(Debug)]
struct ModuleLinkingMetadata<'a> {
    /// The core Wasm module extracted for linked
    module: &'a Module<'a>,
    /// The ID for this module within the component
    module_id: u32,
    /// The order in which this module should be instantiated w.r.t other modules
    instantiate_order: u32,
    /// The import renames needed for this module to be correctly linked to sister modules
    import_renames: ImportRenames<'a>,
}

/// Type providing ergonomic accessor methods to the [Component]
pub struct ComponentAccessor<'a>(Component<'a>);

impl<'a> From<Component<'a>> for ComponentAccessor<'a> {
    fn from(component: Component<'a>) -> Self {
        Self(component)
    }
}

impl<'a> Deref for ComponentAccessor<'a> {
    type Target = Component<'a>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a> DerefMut for ComponentAccessor<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a> ComponentAccessor<'a> {
    fn space_size(&self, space: Space) -> u32 {
        self.index_store
            .borrow()
            .get(&self.space_id)
            .get_space(&space)
            .unwrap()
            .len_assumed_id() as u32
    }

    /// Assert assumptions about the component that must hold for decomposition to be valid
    ///
    /// Relax these as we build out this tool. Currently, we stop the following:
    /// * Main component instances
    /// * Nested components
    ///
    /// Note: Imports can still use things like component, module. We are not testing for
    /// full recursive enforcement of these assumptions.
    pub fn assert_assumptions(&self) {
        unsupported!(
            !self.component_instances.is_empty(),
            "Main component instances"
        );
        unsupported!(!self.components.is_empty(), "Nested components");

        for import in self.imports.iter() {
            // Use the Resolve trait to resolve ComponentTypeRef
            let resolved = import.ty.resolve(self);

            // Limit the kinds of imports we support currently
            match resolved {
                // Types are abstracted in replay, so they can be ignored
                ComponentTypeRefResolved::Type(_) => {}
                ComponentTypeRefResolved::Instance(t) => {
                    if let ComponentTypeSource::Inline(_) = t {
                    } else {
                        unsupported!("Imported instance types besides inline instance types");
                    }
                }
                // TODO: Validate instance imports refer to main component instances
                _ => {
                    unsupported!("Import types besides [type, instance]");
                }
            }
        }
    }

    /// * Multiple instantiations of a single core instance are currently disallowed
    pub fn instantiate_commands(&self) -> Result<()> {
        let mut export_core_instances = HashMap::<u32, &Box<[Export]>>::new();
        let mut mm_map = HashMap::<u32, ModuleMetadata>::new();
        // Only needs to handle core instances for now
        // But we iterate the space because core instances could be aliased from inner component instances
        for instance_id in 0..self.space_size(Space::CoreInst) {
            match self.resolve_to_source(instance_id) {
                CoreInstanceSource::Inline(instance) => {
                    println!("Inline instance: {:?}", instance);
                    match instance {
                        // These exports may be used later by Instantiate instances for linking
                        Instance::FromExports(exports) => {
                            export_core_instances.insert(instance_id, exports);
                        }
                        Instance::Instantiate { module_index, args } => {
                            if mm_map.contains_key(module_index) {
                                unsupported!(format!(
                                    "Multiple instantiations of core module: {}",
                                    module_index
                                ));
                            }

                            // Populate import map for the module being instantiated
                            let module = self.resolve_to_root::<CoreModuleSource>(*module_index);
                            let mut import_map =
                                HashMap::<&'a str, HashMap<&'a str, ImportsID>>::new();
                            for (i, import) in module.imports.iter().enumerate() {
                                let members = import_map.entry(import.module.as_ref()).or_default();
                                members.insert(&import.name, ImportsID(i as u32));
                            }
                            mm_map.insert(*module_index, ModuleMetadata { module, import_map });

                            // Gather linking information from args
                            let mut expected_imports = mm_map
                                .get(module_index)
                                .expect("Module should be already defined")
                                .import_map
                                .clone();
                            for arg in args {
                                // Ensure no new kinds of instantiation args are introduced
                                match arg.kind {
                                    InstantiationArgKind::Instance => {}
                                };
                                let member_imports = expected_imports
                                    .remove(arg.name)
                                    .expect("import should be already populated");
                                for export in export_core_instances
                                    .get(&arg.index)
                                    .expect("exported core instance should be already populated")
                                    .iter()
                                {
                                    let import_id = member_imports.get(export.name).unwrap();
                                    let e = self.resolve_core_export(export);
                                    match e {
                                        CoreInstanceExportSource::Func(f) => {}
                                    }
                                    println!("Export: {:?} ", e);
                                }
                            }
                            assert!(expected_imports.is_empty());
                            // Args is
                        }
                    }
                }
                CoreInstanceSource::Aliased(alias) => {
                    unsupported!(format!("Aliased core instances: {:?}", alias));
                }
            }
        }
        Ok(())
    }

    // Resolve an index in a [Space] to (subspace, vec_idx)
    fn resolve_index(&self, space: Space, idx: u32) -> (SpaceSubtype, usize) {
        let idx_ref = IndexedRef {
            depth: Depth::default(),
            space,
            index: idx,
        };
        let mut store = self.index_store.borrow_mut();
        let (subspace, vec_idx, rec) = store.index_from_assumed_id(&self.space_id, &idx_ref);
        assert!(rec.is_none(), "Recgroups not supported in resolve_index");
        (subspace, vec_idx)
    }

    /// Resolve an index to a specific source type.
    pub fn resolve_to_source<T: ToSource<'a>>(&'a self, idx: u32) -> T {
        T::from_idx(self, idx)
    }

    /// Resolve an index to its root type.
    pub fn resolve_to_root<T>(&'a self, idx: u32) -> T::Root
    where
        T: ToSource<'a>,
    {
        T::root_field(self, idx)
    }

    /// Resolve a core instance export by kind and index.
    ///
    /// Returns a `CoreInstanceExportSource` which can be converted to the specific
    /// source type via `try_into()`.
    ///
    /// # Usage
    /// ```ignore
    /// let source = accessor.resolve_core_export(export.kind, export.index);
    /// let func_src: CoreFuncSource = source.try_into().unwrap();
    /// ```
    pub fn resolve_core_export(&'a self, export: &'a Export<'a>) -> CoreInstanceExportSource<'a> {
        match export.kind {
            ExternalKind::Func | ExternalKind::FuncExact => {
                CoreInstanceExportSource::Func(self.resolve_to_source(export.index))
            }
            ExternalKind::Table => CoreInstanceExportSource::Table(CoreTableSource::Aliased(
                &self.alias.items[export.index as usize],
            )),
            ExternalKind::Memory => CoreInstanceExportSource::Memory(CoreMemorySource::Aliased(
                &self.alias.items[export.index as usize],
            )),
            ExternalKind::Global => CoreInstanceExportSource::Global(CoreGlobalSource::Aliased(
                &self.alias.items[export.index as usize],
            )),
            ExternalKind::Tag => CoreInstanceExportSource::Tag(CoreTagSource::Aliased(
                &self.alias.items[export.index as usize],
            )),
        }
    }

    /// Get a list of all core modules in the component
    pub fn module_list(&self) -> Vec<Module<'a>> {
        // Currently, since we do not support nested components or imported modules, this is just all modules
        self.modules.to_vec()
    }
}
