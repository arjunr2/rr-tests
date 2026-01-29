use anyhow::Result;
use std::ops::{Deref, DerefMut};
use wirm::ir::component::idx_spaces::{
    Depth, IndexedRef, ReferencedIndices, Refs, Space, SpaceSubtype,
};
/// Abstraction to easily access various parts of a [Component]
use wirm::wasmparser::{ComponentTypeRef, Instance};
use wirm::{Component, Module};

mod node;
use node::ComponentNode;

/// Aggregate struct to hold information about both a node's location in the component's index spaces
/// and where it can be found in the IR
#[derive(Debug, Copy, Clone)]
struct SpaceAccessor {
    idx_ref: IndexedRef,
    subspace: SpaceSubtype,
    subspace_idx: usize,
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
    /// Compute the [SpaceAccessor] in the root component for a referencable index
    /// and a function to extract the relevant [IndexedRef]
    fn space_accessor_root<T, F>(&self, field: T, index_ref_accessor: F) -> SpaceAccessor
    where
        T: ReferencedIndices,
        F: Fn(&Refs) -> &IndexedRef,
    {
        let refs = field.referenced_indices(Depth::default()).unwrap();
        let idx_ref = index_ref_accessor(&refs);
        self.space_accessor_root_with_ref(idx_ref)
    }

    /// Compute the [SpaceAccessor] in the root component with an [IndexedRef]
    fn space_accessor_root_with_ref(&self, idx_ref: &IndexedRef) -> SpaceAccessor {
        let mut store = self.index_store.borrow_mut();
        let (subspace, subspace_idx, rec) = store.index_from_assumed_id(&self.space_id, idx_ref);
        unsupported!(rec.is_some(), "Recgroups");

        SpaceAccessor {
            idx_ref: *idx_ref,
            subspace,
            subspace_idx,
        }
    }

    /// Resolve a [SpaceAccessor] in the root component to the actual IR node
    fn resolve_node(&'a self, sa: SpaceAccessor) -> ComponentNode<'a> {
        match sa.subspace {
            SpaceSubtype::Main => match sa.idx_ref.space {
                Space::CompType => {
                    ComponentNode::ComponentType(&self.component_types.items[sa.subspace_idx])
                }
                Space::CompInst => {
                    ComponentNode::ComponentInstance(&self.component_instances[sa.subspace_idx])
                }
                Space::CoreModule => ComponentNode::Module(&self.modules[sa.subspace_idx]),
                Space::CoreType => ComponentNode::CoreType(&self.core_types[sa.subspace_idx]),
                Space::CoreInst => ComponentNode::CoreInstance(&self.instances[sa.subspace_idx]),
                Space::CompFunc | Space::CoreFunc => {
                    ComponentNode::Canon(&self.canons.items[sa.subspace_idx])
                }
                _ => panic!("Unsupported node resolution for {:?}", sa),
            },
            SpaceSubtype::Import => ComponentNode::ComponentImport(&self.imports[sa.subspace_idx]),
            SpaceSubtype::Export => ComponentNode::ComponentExport(&self.exports[sa.subspace_idx]),
            SpaceSubtype::Alias => ComponentNode::Alias(&self.alias.items[sa.subspace_idx]),
            SpaceSubtype::Components => {
                unsupported!("Nested components");
            }
        }
    }

    /// Follow an IndexedRef to get the node it points to
    fn get_node(&'a self, idx_ref: &IndexedRef) -> ComponentNode<'a> {
        let sa = self.space_accessor_root_with_ref(idx_ref);
        self.resolve_node(sa)
    }

    fn space_size(&self, space: Space) -> u32 {
        self.index_store
            .borrow()
            .get(&self.space_id)
            .get_space(&space)
            .unwrap()
            .len_assumed_id() as u32
    }

    fn iter_space(&self, space: Space) -> impl Iterator<Item = IndexedRef> + '_ {
        let size = self.space_size(space);
        (0..size).map(move |i| IndexedRef {
            depth: Depth::default(),
            space,
            index: i,
        })
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
            let import_node = ComponentNode::ComponentImport(import);
            // Get the space accessor for import definition
            let sa = self.space_accessor_root(import_node, |refs| {
                // Imports can only have type refs
                assert_eq!(refs.as_list().len(), 1);
                let idx_ref = refs.ty();
                assert!(
                    idx_ref.space == Space::CompType,
                    "Import refs must be component type refs"
                );
                idx_ref
            });

            // Limit the kinds of imports we support currently
            match import.ty {
                // Types are abstracted in replay, so they can be ignored
                ComponentTypeRef::Type(_) => {}
                ComponentTypeRef::Instance(_) => {
                    assert_eq!(
                        sa.subspace,
                        SpaceSubtype::Main,
                        "Instance imports must refer to main component instances"
                    );
                }
                _ => {
                    unsupported!("Import types besides [type, instance]");
                }
            }
        }
    }

    pub fn instantiate_commands(&self) -> Result<()> {
        // Only needs to handle core instances for now
        // But we iterate the space because core instances could be aliased from inner component instances
        for iref in self.iter_space(Space::CoreInst) {
            let instance = self.get_node(&iref).core_instance()?;
            println!("Instance: {:?}", instance);
            let x = instance.referenced_indices(Depth::default()).unwrap();
            match instance {
                Instance::FromExports(exports) => {
                    println!("Exports: {:?}", x.as_list());
                }
                Instance::Instantiate { module_index, args } => {
                    println!("Instantiate: {:?}", x.as_list());
                }
            }
        }
        Ok(())
    }

    /// Get a list of all core modules in the component
    pub fn module_list(&self) -> Vec<Module<'a>> {
        // Currently, since we do not support nested components or imported modules, this is just all modules
        self.modules.to_vec()
    }
}
