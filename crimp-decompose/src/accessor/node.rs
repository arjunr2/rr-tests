use anyhow::{Result, anyhow};
use wirm::Module;
use wirm::ir::component::idx_spaces::{Depth, ReferencedIndices, Refs};
use wirm::wasmparser::{
    CanonicalFunction, ComponentAlias, ComponentExport, ComponentImport, ComponentInstance,
    ComponentType, CoreType, Instance,
};

/// Wrapper enum for all IR node types that can be referenced through
/// referenced indices
#[derive(Debug, Clone, Copy)]
pub enum ComponentNode<'a> {
    ComponentType(&'a ComponentType<'a>),
    ComponentImport(&'a ComponentImport<'a>),
    ComponentExport(&'a ComponentExport<'a>),
    ComponentInstance(&'a ComponentInstance<'a>),
    Module(&'a Module<'a>),
    CoreType(&'a CoreType<'a>),
    CoreInstance(&'a Instance<'a>),
    Alias(&'a ComponentAlias<'a>),
    Canon(&'a CanonicalFunction),
}

impl ReferencedIndices for ComponentNode<'_> {
    fn referenced_indices(&self, depth: Depth) -> Option<Refs> {
        match self {
            ComponentNode::ComponentType(ty) => ty.referenced_indices(depth),
            ComponentNode::ComponentImport(imp) => imp.referenced_indices(depth),
            ComponentNode::ComponentExport(exp) => exp.referenced_indices(depth),
            ComponentNode::ComponentInstance(inst) => inst.referenced_indices(depth),
            ComponentNode::Module(m) => m.referenced_indices(depth),
            ComponentNode::CoreType(ty) => ty.referenced_indices(depth),
            ComponentNode::CoreInstance(inst) => inst.referenced_indices(depth),
            ComponentNode::Alias(alias) => alias.referenced_indices(depth),
            ComponentNode::Canon(canon) => canon.referenced_indices(depth),
        }
    }
}

impl<'a> ComponentNode<'a> {
    pub fn core_instance(self) -> Result<&'a Instance<'a>> {
        match self {
            ComponentNode::CoreInstance(inst) => Ok(inst),
            _ => Err(anyhow!("Not a core instance node | found {:?}", self)),
        }
    }
}

impl<'a> From<&'a ComponentType<'a>> for ComponentNode<'a> {
    fn from(node: &'a ComponentType<'a>) -> Self {
        ComponentNode::ComponentType(node)
    }
}

impl<'a> From<&'a ComponentImport<'a>> for ComponentNode<'a> {
    fn from(node: &'a ComponentImport<'a>) -> Self {
        ComponentNode::ComponentImport(node)
    }
}

impl<'a> From<&'a ComponentExport<'a>> for ComponentNode<'a> {
    fn from(node: &'a ComponentExport<'a>) -> Self {
        ComponentNode::ComponentExport(node)
    }
}

impl<'a> From<&'a ComponentInstance<'a>> for ComponentNode<'a> {
    fn from(node: &'a ComponentInstance<'a>) -> Self {
        ComponentNode::ComponentInstance(node)
    }
}

impl<'a> From<&'a Module<'a>> for ComponentNode<'a> {
    fn from(node: &'a Module<'a>) -> Self {
        ComponentNode::Module(node)
    }
}

impl<'a> From<&'a CoreType<'a>> for ComponentNode<'a> {
    fn from(node: &'a CoreType<'a>) -> Self {
        ComponentNode::CoreType(node)
    }
}

impl<'a> From<&'a Instance<'a>> for ComponentNode<'a> {
    fn from(node: &'a Instance<'a>) -> Self {
        ComponentNode::CoreInstance(node)
    }
}

impl<'a> From<&'a ComponentAlias<'a>> for ComponentNode<'a> {
    fn from(node: &'a ComponentAlias<'a>) -> Self {
        ComponentNode::Alias(node)
    }
}

impl<'a> From<&'a CanonicalFunction> for ComponentNode<'a> {
    fn from(node: &'a CanonicalFunction) -> Self {
        ComponentNode::Canon(node)
    }
}
