use std::ops::{Deref, DerefMut};

use wirm::ir::component::idx_spaces::{
    Depth, IndexedRef, ReferencedIndices, Refs, Space, SpaceSubtype,
};
/// Abstraction to easily access various parts of a [Component]
use wirm::wasmparser::ComponentTypeRef;
use wirm::{Component, Module};

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

#[derive(Debug, Copy, Clone)]
struct SpaceAccessor {
    idx_ref: IndexedRef,
    subspace: SpaceSubtype,
    subspace_idx: usize,
}

impl<'a> ComponentAccessor<'a> {
    /// Compute the [SpaceAccessor] in the root component for a referencable index
    /// and a function to extract the relevant [IndexedRef]
    fn space_accessor_root_for_node<T, F>(&self, node: T, ref_accessor: F) -> SpaceAccessor
    where
        T: ReferencedIndices,
        F: Fn(&Refs) -> &IndexedRef,
    {
        let refs = node.referenced_indices(Depth::default()).unwrap();
        let idx_ref = ref_accessor(&refs);

        let mut store = self.index_store.borrow_mut();
        let (subspace, subspace_idx, rec) = store.index_from_assumed_id(&self.space_id, idx_ref);
        unsupported!(rec.is_some(), "Recgroups");
        SpaceAccessor {
            idx_ref: *idx_ref,
            subspace,
            subspace_idx,
        }
    }

    /// Assert assumptions about the component that must hold for decomposition to be valid
    ///
    /// Relax these as we build out this tool. Currently, we stop the following:
    /// * Main component instances
    /// * Nested components
    /// * Imports of various kinds, specifically barring imported modules
    pub fn assert_assumptions(&self) {
        unsupported!(!self.component_instances.is_empty(), "Main component instances");
        unsupported!(!self.components.is_empty(), "Nested components");

        for import in self.imports.iter().copied() {
            // Get the space and index of the ref
            let sa = self.space_accessor_root_for_node(import, |refs| {
                // Assert only type refs for import
                assert_eq!(refs.as_list().len(), 1);
                let idx_ref = refs.ty();
                assert!(
                    idx_ref.space == Space::CompType,
                    "Import refs must be component type refs"
                );
                idx_ref
            });
            log::debug!("Space Accessor: {:?}", sa);

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
                    match sa.subspace {
                        SpaceSubtype::Import => {
                            unsupported!("Imported instances not supported");
                        }
                        SpaceSubtype::Main => {}
                        _ => unreachable!(),
                        SpaceSubtype::Alias => {
                            unreachable!("Alias subtype not possible for imports");
                        }
                        SpaceSubtype::Components => {}
                        SpaceSubtype::Export => {}
                    }
                }
                _ => {
                    unsupported!("Import types besides [type, instance]");
                }
            }
        }
    }

    /// Get a list of all core modules in the component
    pub fn module_list(&self) -> Vec<Module<'a>> {
        // Currently, since we do not support nested components or imported modules, this is just all modules
        self.modules.to_vec()
    }
}
