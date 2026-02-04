//! Node type definitions for each index space.
//!
//! Each node type has specialized variants reflecting how items
//! in that index space can be introduced (Import, Alias, Definition, etc.)

use wirm::Module;
use wirm::wasmparser::CanonicalOption;

// Re-export wasmparser types we use in our API
pub use wirm::wasmparser::{ComponentType, ComponentTypeRef};

// =============================================================================
// Common Info Types
// =============================================================================

/// Information about an alias.
#[derive(Debug, Clone)]
pub enum AliasInfo {
    /// Alias to an export of a component instance
    InstanceExport { instance_idx: u32, name: String },
    /// Alias to an export of a core instance
    CoreInstanceExport { instance_idx: u32, name: String },
    /// Alias to an outer scope item
    Outer { count: u32, index: u32 },
}

// =============================================================================
// Component-Level Nodes
// =============================================================================

/// A core WebAssembly module in the module index space.
#[derive(Debug, Clone)]
pub enum ModuleNode<'a> {
    /// Imported module - index into component's imports vector
    Imported(u32),
    Aliased(AliasInfo),
    Defined {
        /// Parsed module IR from wirm
        module: Module<'a>,
    },
}

/// A nested component in the component index space.
#[derive(Debug)]
pub enum ComponentNode<'a> {
    /// Imported component - index into component's imports vector
    Imported(u32),
    Aliased(AliasInfo),
    Defined {
        /// Recursively parsed component (Rc<RefCell> for shared access and parent chain setup)
        component: super::ComponentRef<'a>,
    },
}

impl<'a> Clone for ComponentNode<'a> {
    fn clone(&self) -> Self {
        match self {
            Self::Imported(idx) => Self::Imported(*idx),
            Self::Aliased(info) => Self::Aliased(info.clone()),
            Self::Defined { component } => Self::Defined {
                component: component.clone(), // Clones the Rc, not the inner data
            },
        }
    }
}

/// A component instance in the instance index space.
#[derive(Debug, Clone)]
pub enum ComponentInstanceNode {
    /// Imported instance - index into component's imports vector
    Imported(u32),
    Aliased(AliasInfo),
    /// Created by instantiating a component
    Instantiated {
        component_idx: u32,
        args: Vec<InstantiationArg>,
    },
    /// Created inline from a list of exports
    FromExports(Vec<InlineExport>),
}

#[derive(Debug, Clone)]
pub struct InstantiationArg {
    pub name: String,
    pub kind: InstantiationArgKind,
    pub index: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum InstantiationArgKind {
    Module,
    Component,
    Instance,
    Func,
    Value,
    Type,
    // Core kinds
    CoreModule,
    CoreInstance,
    CoreFunc,
    CoreMemory,
    CoreTable,
    CoreGlobal,
}

#[derive(Debug, Clone)]
pub struct InlineExport {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
}

/// A component function in the func index space.
#[derive(Debug, Clone)]
pub enum ComponentFuncNode {
    /// Imported function - index into component's imports vector
    Imported(u32),
    Aliased(AliasInfo),
    /// Created by `canon lift`
    Lifted {
        core_func_idx: u32,
        type_idx: u32,
        options: Vec<CanonicalOption>,
    },
}

/// A value in the value index space.
#[derive(Debug, Clone)]
pub enum ValueNode {
    /// Imported value - index into component's imports vector
    Imported(u32),
    Aliased(AliasInfo),
}

/// A type in the type index space.
#[derive(Debug, Clone)]
pub enum TypeNode<'a> {
    /// Component type defined inline (from wasmparser)
    Defined(ComponentType<'a>),
    Aliased(AliasInfo),
    /// Imported type - index into component's imports vector
    Imported(u32),
}

// =============================================================================
// Core-Level Nodes (visible at component level)
// =============================================================================

/// A core instance in the core instance index space.
#[derive(Debug, Clone)]
pub enum CoreInstanceNode {
    Aliased(AliasInfo),
    /// Created by instantiating a module
    Instantiated {
        module_idx: u32,
        args: Vec<CoreInstantiationArg>,
    },
    /// Created inline from exports
    FromExports(Vec<CoreInlineExport>),
}

#[derive(Debug, Clone)]
pub struct CoreInstantiationArg {
    pub name: String,
    pub kind: CoreExportKind,
    pub index: u32,
}

#[derive(Debug, Clone)]
pub struct CoreInlineExport {
    pub name: String,
    pub kind: CoreExportKind,
    pub index: u32,
}

/// A core function in the core func index space.
#[derive(Debug, Clone)]
pub enum CoreFuncNode {
    Aliased(AliasInfo),
    /// Created by `canon lower`
    Lowered {
        func_idx: u32,
        options: Vec<CanonicalOption>,
    },
}

/// A core memory in the core memory index space.
#[derive(Debug, Clone)]
pub enum CoreMemoryNode {
    Aliased(AliasInfo),
}

/// A core table in the core table index space.
#[derive(Debug, Clone)]
pub enum CoreTableNode {
    Aliased(AliasInfo),
}

/// A core global in the core global index space.
#[derive(Debug, Clone)]
pub enum CoreGlobalNode {
    Aliased(AliasInfo),
}

/// A core type in the core type index space.
#[derive(Debug, Clone)]
pub enum CoreTypeNode {
    Aliased(AliasInfo),
    /// Defined inline (function signature, etc.)
    Defined(CoreTypeDef),
}

#[derive(Debug, Clone)]
pub enum CoreTypeDef {
    // Placeholder - will be expanded
    Func,
    Module,
}

// =============================================================================
// Exports
// =============================================================================

/// An export from this component.
#[derive(Debug, Clone)]
pub struct ExportNode {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
    /// Optional type ascription
    pub ty: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Module,
    Component,
    Instance,
    Func,
    Value,
    Type,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreExportKind {
    Func,
    Table,
    Memory,
    Global,
    Tag,
}

// =============================================================================
// Resolved Types (without Aliased variants)
// =============================================================================

/// A resolved import with name and type reference.
#[derive(Debug, Clone)]
pub struct ResolvedImport<'a> {
    /// The import name
    pub name: &'a str,
    /// The type reference (Module, Func, Value, Type, Instance, or Component)
    pub ty: ComponentTypeRef,
}

/// Resolved module - either imported or defined inline.
#[derive(Debug, Clone)]
pub enum ResolvedModule<'a> {
    Imported(ResolvedImport<'a>),
    /// A defined module with its parsed IR.
    Defined {
        module: Module<'a>,
    },
}

/// Resolved component - either imported or defined inline.
#[derive(Debug)]
pub enum ResolvedComponent<'a> {
    Imported(ResolvedImport<'a>),
    Defined { component: super::ComponentRef<'a> },
}

impl<'a> Clone for ResolvedComponent<'a> {
    fn clone(&self) -> Self {
        match self {
            Self::Imported(import) => Self::Imported(import.clone()),
            Self::Defined { component } => Self::Defined {
                component: component.clone(),
            },
        }
    }
}

/// Resolved component instance.
#[derive(Debug, Clone)]
pub enum ResolvedComponentInstance<'a> {
    Imported(ResolvedImport<'a>),
    Instantiated {
        component_idx: u32,
        args: Vec<InstantiationArg>,
    },
    FromExports(Vec<InlineExport>),
}

/// Resolved component function.
#[derive(Debug, Clone)]
pub enum ResolvedComponentFunc<'a> {
    Imported(ResolvedImport<'a>),
    Lifted {
        core_func_idx: u32,
        type_idx: u32,
        options: Vec<CanonicalOption>,
    },
}

/// Resolved value - only imported (values have no other definition form).
pub type ResolvedValue<'a> = ResolvedImport<'a>;

/// Resolved component type.
#[derive(Debug, Clone)]
pub enum ResolvedType<'a> {
    Imported(ResolvedImport<'a>),
    Defined(ComponentType<'a>),
}

/// Resolved core instance.
#[derive(Debug, Clone)]
pub enum ResolvedCoreInstance {
    Instantiated {
        module_idx: u32,
        args: Vec<CoreInstantiationArg>,
    },
    FromExports(Vec<CoreInlineExport>),
}

/// Resolved core function - either lowered or from a module.
#[derive(Debug, Clone)]
pub enum ResolvedCoreFunc {
    /// Created by `canon lower`
    Lowered {
        func_idx: u32,
        options: Vec<CanonicalOption>,
    },
    /// From a module's export (traced through core instance)
    FromModule { module_idx: u32, func_idx: u32 },
}

/// Resolved core memory - always from a module.
#[derive(Debug, Clone)]
pub struct ResolvedCoreMemory {
    pub module_idx: u32,
    pub memory_idx: u32,
}

/// Resolved core table - always from a module.
#[derive(Debug, Clone)]
pub struct ResolvedCoreTable {
    pub module_idx: u32,
    pub table_idx: u32,
}

/// Resolved core global - always from a module.
#[derive(Debug, Clone)]
pub struct ResolvedCoreGlobal {
    pub module_idx: u32,
    pub global_idx: u32,
}

/// Resolved core type.
#[derive(Debug, Clone)]
pub enum ResolvedCoreType {
    Defined(CoreTypeDef),
}
