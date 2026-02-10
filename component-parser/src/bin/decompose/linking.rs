use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::ops::Deref;

use component_parser::wirm::Module;
use serde::Serialize;

pub type Checksum = [u8; 32];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
/// Index for core imports within a module's IR.
pub struct CoreImportIndex(pub u32);
impl std::ops::Deref for CoreImportIndex {
    type Target = u32;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<component_parser::wirm::ir::id::ImportsID> for CoreImportIndex {
    fn from(id: component_parser::wirm::ir::id::ImportsID) -> Self {
        Self(id.0)
    }
}

#[derive(Debug)]
/// Metadata associated with a module that is instantiated in the component
pub struct ModuleMetadata<'a> {
    /// The module
    pub module: Module<'a>,
    /// Map of its imports (to prevent re-computation when it is instantiated multiple times)
    pub import_map: HashMap<String, HashMap<String, CoreImportIndex>>,
}

/// Index into [`LinkingMetadata::mm`]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct ModuleLinkID(pub u32);
impl Deref for ModuleLinkID {
    type Target = u32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type InstantiateOrder = u32;
/// Index type into a (module, export_name)
#[derive(Debug, Clone, Serialize)]
pub struct ModuleExport(pub ModuleLinkID, pub String);

/// The export index as assigned by the recorder in RR.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash, Serialize)]
pub struct RecordExportIndex(pub u32);

/// Index for canonical options adapters within a module's IR.
#[derive(Debug, Default, Serialize)]
pub struct CanonicalOptionsIndex {
    pub memory: Option<ModuleExport>,
    pub realloc: Option<ModuleExport>,
    pub post_return: Option<ModuleExport>,
}

/// Metadata capturing all the import linking information for a module instantiation
///
/// For a given instance, every import ID in the module must fall into at least
/// one of these fields.
#[derive(Debug, Default)]
pub struct ImportMetadata {
    /// Renames for import packages with the module name
    pub package_renames: HashMap<CoreImportIndex, ModuleLinkID>,
    /// Renames for import members with the member name
    pub member_renames: HashMap<CoreImportIndex, String>,
    /// The IDs for the imports in this module that are true imports (not linked into from sister modules)
    /// with optional canonical options if they are canonical lowers.
    pub true_imports: HashMap<CoreImportIndex, Option<CanonicalOptionsIndex>>,
    /// The IDs for the imports in this module that are builtins (e.g. from canonical options)
    pub builtins: HashSet<CoreImportIndex>,
}

/// Metadata needed to capture the linking information for a module for CRIMP replay custom section
#[derive(Debug)]
pub struct InstanceLinkingMetadata {
    /// The module from which this instance was created
    pub module_link_id: ModuleLinkID,
    /// The order in which this module should be instantiated w.r.t other modules
    pub instantiate_order: InstantiateOrder,
    /// The import metadata needed for this module to be correctly linked to sister modules
    pub import_md: ImportMetadata,
}

#[derive(Debug, Serialize)]
/// Metadata to identify core functions being exported.
pub struct ExportFuncMetadata {
    pub name: String,
    /// ID, as assigned to this export by the CRIMP recorder.
    pub record_id: RecordExportIndex,
    pub opts: Option<CanonicalOptionsIndex>,
}

/// Metadata needed to capture complete linking information for a component for CRIMP replay custom section
#[derive(Debug, Default)]
pub struct LinkingMetadata<'a> {
    /// The checksum of the component for which this linking metadata is generated
    pub checksum: Checksum,
    /// The metadata for each module in the component needed for linking and replaying
    pub mm: HashMap<ModuleLinkID, ModuleMetadata<'a>>,
    /// The linking information for each instantiated module
    pub instances: Vec<InstanceLinkingMetadata>,
    /// The exported functions from this component arranged by the module they are sourced from.
    pub export_funcs: HashMap<ModuleLinkID, Vec<ExportFuncMetadata>>,
}

#[derive(Debug, Serialize, Default)]
struct CrimpSerializableModuleData<'a> {
    checksum: Checksum,
    instantiate_order: InstantiateOrder,
    exports: Vec<&'a ExportFuncMetadata>,
}

impl<'a> LinkingMetadata<'a> {
    /// Serialize the crimp section for a single module, specified by a [`ModuleLinkID`]
    ///
    /// Note: This serialization strategy currently only assumes one instance per module,
    pub fn serialize_crimp_section(&self, module_id: ModuleLinkID) -> Result<Vec<u8>> {
        let data = CrimpSerializableModuleData::default();
        postcard::to_stdvec(&data).map_err(Into::into)
    }
}
