//! IR definitions for the Component Model index spaces.

mod index_space;
mod nodes;
mod resolve;

pub use index_space::IndexSpace;
pub use nodes::*;
pub use resolve::Resolve;

use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// Shared reference to a Component (Rc + RefCell for interior mutability).
pub type ComponentRef = Rc<RefCell<Component>>;
/// Weak reference to a Component for parent chains.
pub type ComponentWeak = Weak<RefCell<Component>>;

/// A parent scope for Outer alias resolution.
/// In the Component Model, parent scopes can be Components, ComponentTypes, or InstanceTypes.
#[derive(Debug, Clone)]
pub enum ParentScope {
    /// Parent is a Component
    Component(ComponentWeak),
    /// Parent is a ComponentType definition (placeholder - has its own index spaces)
    ComponentType,
    /// Parent is an InstanceType definition (placeholder - has its own index spaces)
    InstanceType,
}

/// Which index space an import resides in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSpace {
    Module,
    Component,
    Instance,
    Func,
    Value,
    Type,
}

/// Reference to an imported item: which space it's in and its index within that space.
#[derive(Debug, Clone)]
pub struct ImportRef {
    pub space: ImportSpace,
    pub index: u32,
}

/// A parsed WebAssembly Component with all 12 index spaces accessible.
#[derive(Debug, Default)]
pub struct Component {
    /// Parent scopes for Outer alias resolution.
    /// Ordered innermost to outermost: parents[0] is the immediate parent.
    pub parents: Vec<ParentScope>,

    /// All imports in order they appear, with references to their location in index spaces.
    pub imports: Vec<ImportRef>,

    // Component-level index spaces
    pub modules: IndexSpace<ModuleNode>,
    pub components: IndexSpace<ComponentNode>,
    pub instances: IndexSpace<ComponentInstanceNode>,
    pub funcs: IndexSpace<ComponentFuncNode>,
    pub values: IndexSpace<ValueNode>,
    pub types: IndexSpace<TypeNode>,

    // Core-level index spaces (visible to the component)
    pub core_instances: IndexSpace<CoreInstanceNode>,
    pub core_funcs: IndexSpace<CoreFuncNode>,
    pub core_memories: IndexSpace<CoreMemoryNode>,
    pub core_tables: IndexSpace<CoreTableNode>,
    pub core_globals: IndexSpace<CoreGlobalNode>,
    pub core_types: IndexSpace<CoreTypeNode>,

    // Exports (name -> what is exported)
    pub exports: IndexMap<String, ExportNode>,
}
