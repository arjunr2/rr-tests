//! Per-space source types for immediate index resolution.
//!
//! Each index space has its own source enum that reflects where items
//! in that space can actually come from. This provides type-safe resolution.
//!
//! Improvements possible:
//! * If `Space` was instead a trait and IndexSpaces were types implementing that trait,
//!     it would make things much more ergonomic.

use wirm::ir::component::idx_spaces::{Space, SpaceSubtype};
use wirm::ir::module::module_exports::Export;
use wirm::wasmparser::{
    CanonicalFunction, ComponentAlias, ComponentExport, ComponentExternalKind, ComponentImport,
    ComponentInstance, ComponentType, ComponentTypeRef, ComponentValType, CoreType, Instance,
    PrimitiveValType, TypeBounds,
};
use wirm::{Component, Module};

use crate::accessor::ComponentAccessor;

// ============================================================================
// Per-space source enums
// ============================================================================

/// Where a ComponentType was found.
#[derive(Debug, Clone)]
pub enum ComponentTypeSource<'a> {
    /// Inline type definition in `component_types`
    Inline(&'a ComponentType<'a>),
    /// Type introduced via import
    Imported(&'a ComponentImport<'a>),
    /// Type introduced via alias
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a CoreType was found.
#[derive(Debug, Clone)]
pub enum CoreTypeSource<'a> {
    /// Inline type definition in `core_types`
    Inline(&'a CoreType<'a>),
    /// Type introduced via alias (Outer only - core types can't be imported directly)
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a CoreModule was found.
#[derive(Debug, Clone)]
pub enum CoreModuleSource<'a> {
    /// Inline module in `modules`
    Inline(&'a Module<'a>),
    /// Module introduced via alias (Outer only - modules can't be imported directly)
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a ComponentInstance was found.
#[derive(Debug, Clone)]
pub enum ComponentInstanceSource<'a> {
    /// Inline instance in `component_instances`
    Inline(&'a ComponentInstance<'a>),
    /// Instance introduced via import
    Imported(&'a ComponentImport<'a>),
    /// Instance introduced via alias
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a CoreInstance (core module instance) was found.
#[derive(Debug, Clone)]
pub enum CoreInstanceSource<'a> {
    /// Inline instance in `instances`
    Inline(&'a Instance<'a>),
    /// Instance introduced via alias
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a ComponentFunc was found.
#[derive(Debug, Clone)]
pub enum ComponentFuncSource<'a> {
    /// Lifted from a core function via `canon lift`
    Lifted(&'a CanonicalFunction),
    /// Function introduced via import
    Imported(&'a ComponentImport<'a>),
    /// Function introduced via alias
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a CoreFunc was found.
#[derive(Debug, Clone)]
pub enum CoreFuncSource<'a> {
    /// Lowered from a component function via `canon lower`
    Canon(&'a CanonicalFunction),
    /// Function introduced via alias (from core instance export)
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a CoreTable was found.
#[derive(Debug, Clone)]
pub enum CoreTableSource<'a> {
    /// Table introduced via alias (from core instance export)
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a CoreMemory was found.
#[derive(Debug, Clone)]
pub enum CoreMemorySource<'a> {
    /// Memory introduced via alias (from core instance export)
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a CoreGlobal was found.
#[derive(Debug, Clone)]
pub enum CoreGlobalSource<'a> {
    /// Global introduced via alias (from core instance export)
    Aliased(&'a ComponentAlias<'a>),
}

/// Where a CoreTag was found.
#[derive(Debug, Clone)]
pub enum CoreTagSource<'a> {
    /// Tag introduced via alias (from core instance export)
    Aliased(&'a ComponentAlias<'a>),
}

// ============================================================================
// Resolve trait - resolve composite types containing indices
// ============================================================================

/// Trait for resolving types that contain indices to their resolved form.
pub trait Resolve<'a> {
    type Resolved;
    fn resolve(&self, accessor: &'a ComponentAccessor<'a>) -> Self::Resolved;
}

// ============================================================================
// ComponentTypeRef resolution
// ============================================================================

/// Resolved form of `ComponentTypeRef` with indices replaced by Source types.
#[derive(Debug, Clone)]
pub enum ComponentTypeRefResolved<'a> {
    /// Reference to a core module type
    Module(CoreTypeSource<'a>),
    /// Reference to a component function type
    Func(ComponentTypeSource<'a>),
    /// Reference to a value type
    Value(ComponentValTypeResolved<'a>),
    /// Reference to a bounded type
    Type(TypeBoundsResolved<'a>),
    /// Reference to an instance type
    Instance(ComponentTypeSource<'a>),
    /// Reference to a component type
    Component(ComponentTypeSource<'a>),
}

// Resolved form of `ComponentAlias` for a source T of a given sort
#[derive(Debug, Clone)]
pub struct AliasResolved<'a, T: 'a> {
    pub sort_source: &'a T,
    pub kind: ComponentAliasResolvedKind<'a>,
}

#[derive(Debug, Clone)]
enum ComponentAliasResolvedKind<'a> {
    InstanceExport {
        instance: ComponentInstanceSource<'a>,
        name: &'a str,
    },
    CoreInstanceExport {
        instance: CoreInstanceSource<'a>,
        name: &'a str,
    },
    Outer {
        count: u32,
        index: u32,
    },
}

/// Resolved form of `ComponentValType`.
#[derive(Debug, Clone)]
pub enum ComponentValTypeResolved<'a> {
    /// A primitive value type (no index to resolve)
    Primitive(PrimitiveValType),
    /// A reference to a defined type
    Type(ComponentTypeSource<'a>),
}

/// Resolved form of `TypeBounds`.
#[derive(Debug, Clone)]
pub enum TypeBoundsResolved<'a> {
    /// Type bounded by equality to another type
    Eq(ComponentTypeSource<'a>),
    /// A fresh sub-resource type (no index to resolve)
    SubResource,
}

impl<'a> Resolve<'a> for ComponentTypeRef {
    type Resolved = ComponentTypeRefResolved<'a>;

    fn resolve(&self, accessor: &'a ComponentAccessor<'a>) -> Self::Resolved {
        match self {
            ComponentTypeRef::Module(idx) => {
                ComponentTypeRefResolved::Module(accessor.resolve_to_source(*idx))
            }
            ComponentTypeRef::Func(idx) => {
                ComponentTypeRefResolved::Func(accessor.resolve_to_source(*idx))
            }
            ComponentTypeRef::Value(val_type) => {
                ComponentTypeRefResolved::Value(val_type.resolve(accessor))
            }
            ComponentTypeRef::Type(bounds) => {
                ComponentTypeRefResolved::Type(bounds.resolve(accessor))
            }
            ComponentTypeRef::Instance(idx) => {
                ComponentTypeRefResolved::Instance(accessor.resolve_to_source(*idx))
            }
            ComponentTypeRef::Component(idx) => {
                ComponentTypeRefResolved::Component(accessor.resolve_to_source(*idx))
            }
        }
    }
}

impl<'a> Resolve<'a> for ComponentValType {
    type Resolved = ComponentValTypeResolved<'a>;

    fn resolve(&self, accessor: &'a ComponentAccessor<'a>) -> Self::Resolved {
        match self {
            ComponentValType::Primitive(prim) => ComponentValTypeResolved::Primitive(*prim),
            ComponentValType::Type(idx) => {
                ComponentValTypeResolved::Type(accessor.resolve_to_source(*idx))
            }
        }
    }
}

impl<'a> Resolve<'a> for TypeBounds {
    type Resolved = TypeBoundsResolved<'a>;

    fn resolve(&self, accessor: &'a ComponentAccessor<'a>) -> Self::Resolved {
        match self {
            TypeBounds::Eq(idx) => TypeBoundsResolved::Eq(accessor.resolve_to_source(*idx)),
            TypeBounds::SubResource => TypeBoundsResolved::SubResource,
        }
    }
}

// ============================================================================
// Source resolution for component definitions
// ============================================================================

/// Trait for source types that can be constructed from an index in a given space.
pub trait ToSource<'a>: Sized {
    /// The space from which this source originates
    const SPACE: Space;
    /// The root type this source would bottom out to
    type Root;

    /// Construct this source from an index in its `SPACE`.
    fn from_idx(accessor: &'a ComponentAccessor, idx: u32) -> Self;

    /// Traverse down chain of sources to get the root field this source references
    fn root_field(_accessor: &'a ComponentAccessor, _idx: u32) -> Self::Root {
        panic!("Root unimplemented for this type");
    }
}

impl<'a> ToSource<'a> for ComponentTypeSource<'a> {
    const SPACE: Space = Space::CompType;
    type Root = &'a ComponentType<'a>;

    fn from_idx(accessor: &'a ComponentAccessor, idx: u32) -> Self {
        //accessor.resolve_index_component_type(idx)
        let (subspace, vec_idx) = accessor.resolve_index(Self::SPACE, idx);
        match subspace {
            SpaceSubtype::Main => {
                ComponentTypeSource::Inline(&accessor.component_types.items[vec_idx])
            }
            SpaceSubtype::Import => ComponentTypeSource::Imported(&accessor.imports[vec_idx]),
            SpaceSubtype::Alias => ComponentTypeSource::Aliased(&accessor.alias.items[vec_idx]),
            SpaceSubtype::Export => panic!("ComponentType cannot come from Export"),
            SpaceSubtype::Components => panic!("Nested components not supported"),
        }
    }
}

impl<'a> ToSource<'a> for CoreTypeSource<'a> {
    const SPACE: Space = Space::CoreType;
    type Root = &'a CoreType<'a>;

    fn from_idx(accessor: &'a ComponentAccessor, idx: u32) -> Self {
        let (subspace, vec_idx) = accessor.resolve_index(Self::SPACE, idx);
        match subspace {
            SpaceSubtype::Main => CoreTypeSource::Inline(&accessor.core_types[vec_idx]),
            SpaceSubtype::Alias => CoreTypeSource::Aliased(&accessor.alias.items[vec_idx]),
            _ => panic!(
                "CoreType can only come from Main or Alias, got {:?}",
                subspace
            ),
        }
    }
}

impl<'a> ToSource<'a> for CoreModuleSource<'a> {
    const SPACE: Space = Space::CoreModule;
    type Root = &'a Module<'a>;

    fn from_idx(accessor: &'a ComponentAccessor, idx: u32) -> Self {
        let (subspace, vec_idx) = accessor.resolve_index(Self::SPACE, idx);
        match subspace {
            SpaceSubtype::Main => CoreModuleSource::Inline(&accessor.modules[vec_idx]),
            SpaceSubtype::Alias => CoreModuleSource::Aliased(&accessor.alias.items[vec_idx]),
            _ => panic!(
                "CoreModule can only come from Main or Alias, got {:?}",
                subspace
            ),
        }
    }

    fn root_field(accessor: &'a ComponentAccessor, idx: u32) -> Self::Root {
        let (subspace, vec_idx) = accessor.resolve_index(Self::SPACE, idx);
        match subspace {
            SpaceSubtype::Main => &accessor.modules[vec_idx],
            SpaceSubtype::Alias => match &accessor.alias.items[vec_idx] {
                ComponentAlias::Outer { .. } => {
                    unsupported!("Outer alias")
                }
                _ => panic!("CoreModule can only be aliased with \'outer\'"),
            },
            _ => panic!(
                "CoreModule can only come from Main or Alias, got {:?}",
                subspace
            ),
        }
    }
}

impl<'a> ToSource<'a> for ComponentInstanceSource<'a> {
    const SPACE: Space = Space::CompInst;
    type Root = &'a ComponentInstance<'a>;

    fn from_idx(accessor: &'a ComponentAccessor, idx: u32) -> Self {
        let (subspace, vec_idx) = accessor.resolve_index(Self::SPACE, idx);
        match subspace {
            SpaceSubtype::Main => {
                ComponentInstanceSource::Inline(&accessor.component_instances[vec_idx])
            }
            SpaceSubtype::Import => ComponentInstanceSource::Imported(&accessor.imports[vec_idx]),
            SpaceSubtype::Alias => ComponentInstanceSource::Aliased(&accessor.alias.items[vec_idx]),
            _ => panic!(
                "ComponentInstance can only come from Main, Import, or Alias, got {:?}",
                subspace
            ),
        }
    }
}

impl<'a> ToSource<'a> for CoreInstanceSource<'a> {
    const SPACE: Space = Space::CoreInst;
    type Root = &'a Instance<'a>;

    fn from_idx(accessor: &'a ComponentAccessor, idx: u32) -> Self {
        let (subspace, vec_idx) = accessor.resolve_index(Self::SPACE, idx);
        match subspace {
            SpaceSubtype::Main => CoreInstanceSource::Inline(&accessor.instances[vec_idx]),
            SpaceSubtype::Alias => CoreInstanceSource::Aliased(&accessor.alias.items[vec_idx]),
            _ => panic!(
                "CoreInstance can only come from Main or Alias, got {:?}",
                subspace
            ),
        }
    }
}

impl<'a> ToSource<'a> for ComponentFuncSource<'a> {
    const SPACE: Space = Space::CompFunc;
    type Root = &'a CanonicalFunction;

    fn from_idx(accessor: &'a ComponentAccessor, idx: u32) -> Self {
        let (subspace, vec_idx) = accessor.resolve_index(Self::SPACE, idx);
        match subspace {
            SpaceSubtype::Main => ComponentFuncSource::Lifted(&accessor.canons.items[vec_idx]),
            SpaceSubtype::Import => ComponentFuncSource::Imported(&accessor.imports[vec_idx]),
            SpaceSubtype::Alias => ComponentFuncSource::Aliased(&accessor.alias.items[vec_idx]),
            _ => panic!(
                "ComponentFunc can only come from Main, Import, or Alias, got {:?}",
                subspace
            ),
        }
    }
}

impl<'a> ToSource<'a> for CoreFuncSource<'a> {
    const SPACE: Space = Space::CoreFunc;
    type Root = &'a CanonicalFunction;

    fn from_idx(accessor: &'a ComponentAccessor, idx: u32) -> Self {
        let (subspace, vec_idx) = accessor.resolve_index(Self::SPACE, idx);
        match subspace {
            SpaceSubtype::Main => {
                let f = &accessor.canons.items[vec_idx];
                if let CanonicalFunction::Lower { .. } = f {
                } else {
                    panic!("CoreFunc source from Main must be Lower, found: {:?}", f);
                }
                CoreFuncSource::Canon(f)
            }
            SpaceSubtype::Alias => CoreFuncSource::Aliased(&accessor.alias.items[vec_idx]),
            _ => panic!(
                "CoreFunc can only come from Main or Alias, got {:?}",
                subspace
            ),
        }
    }
}

// ============================================================================
// Export resolution with enum + try_into().
// Note: This would be possible cleanly without macros if Space was a trait vs
// an enum.
// ============================================================================

/// Unified enum for all resolvable export sources.
/// Used for runtime dispatch when the Space variant isn't known at compile time.
#[derive(Debug, Clone)]
pub enum CoreInstanceExportSource<'a> {
    Func(CoreFuncSource<'a>),
    Table(CoreTableSource<'a>),
    Memory(CoreMemorySource<'a>),
    Global(CoreGlobalSource<'a>),
    Tag(CoreTagSource<'a>),
}

// ============================================================================
// TryFrom implementations for extracting specific source types
// ============================================================================

impl<'a> TryFrom<CoreInstanceExportSource<'a>> for CoreFuncSource<'a> {
    type Error = &'static str;
    fn try_from(value: CoreInstanceExportSource<'a>) -> Result<Self, Self::Error> {
        match value {
            CoreInstanceExportSource::Func(src) => Ok(src),
            _ => Err("CoreInstanceExportSource is not a Func"),
        }
    }
}

impl<'a> TryFrom<CoreInstanceExportSource<'a>> for CoreTableSource<'a> {
    type Error = &'static str;
    fn try_from(value: CoreInstanceExportSource<'a>) -> Result<Self, Self::Error> {
        match value {
            CoreInstanceExportSource::Table(src) => Ok(src),
            _ => Err("CoreInstanceExportSource is not a Table"),
        }
    }
}

impl<'a> TryFrom<CoreInstanceExportSource<'a>> for CoreMemorySource<'a> {
    type Error = &'static str;
    fn try_from(value: CoreInstanceExportSource<'a>) -> Result<Self, Self::Error> {
        match value {
            CoreInstanceExportSource::Memory(src) => Ok(src),
            _ => Err("CoreInstanceExportSource is not a Memory"),
        }
    }
}

impl<'a> TryFrom<CoreInstanceExportSource<'a>> for CoreGlobalSource<'a> {
    type Error = &'static str;
    fn try_from(value: CoreInstanceExportSource<'a>) -> Result<Self, Self::Error> {
        match value {
            CoreInstanceExportSource::Global(src) => Ok(src),
            _ => Err("CoreInstanceExportSource is not a Global"),
        }
    }
}

impl<'a> TryFrom<CoreInstanceExportSource<'a>> for CoreTagSource<'a> {
    type Error = &'static str;
    fn try_from(value: CoreInstanceExportSource<'a>) -> Result<Self, Self::Error> {
        match value {
            CoreInstanceExportSource::Tag(src) => Ok(src),
            _ => Err("CoreInstanceExportSource is not a Tag"),
        }
    }
}

// ============================================================================
// Macros for export resolution
// ============================================================================

/// Resolve an index to an `ExportSource` enum based on the `Space` variant.
/// Supports runtime dispatch—returns an `ExportSource` wrapper.
///
/// # Usage
/// ```ignore
/// let source = resolve_export!(accessor, idx, space);
/// ```
#[macro_export]
macro_rules! resolve_export {
    ($accessor:expr, $idx:expr, $space:expr) => {
        match $space {
            Space::CoreFunc => ExportSource::CoreFunc($accessor.resolve($idx)),
            Space::CoreInst => ExportSource::CoreInstance($accessor.resolve($idx)),
            Space::CoreModule => ExportSource::CoreModule($accessor.resolve($idx)),
            Space::CoreType => ExportSource::CoreType($accessor.resolve($idx)),
            Space::CompFunc => ExportSource::ComponentFunc($accessor.resolve_($idx)),
            Space::CompInst => ExportSource::ComponentInstance($accessor.resolve($idx)),
            Space::CompType => ExportSource::ComponentType($accessor.resolve($idx)),
            // Spaces not directly resolvable at component level (live inside modules or not exported)
            Space::CoreMemory | Space::CoreTable | Space::CoreGlobal | Space::CoreTag | Space::CompVal => {
                panic!("Space {:?} is not resolvable at the component level", $space)
            }
        }
    };
}

/// Resolve an index and extract a specific source type in one call.
/// Panics if the space doesn't match the requested type.
///
/// # Usage
/// ```ignore
/// let func_src: CoreFuncSource = resolve_export_as!(accessor, idx, space, CoreFuncSource);
/// ```
#[macro_export]
macro_rules! resolve_export_as {
    ($accessor:expr, $idx:expr, $space:expr, $target:ty) => {
        <$target>::try_from(resolve_export!($accessor, $idx, $space))
            .expect(concat!("Export is not a ", stringify!($target)))
    };
}
