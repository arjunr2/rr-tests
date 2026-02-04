//! Resolution helpers for traversing the component graph.
//!
//! The `Resolve` trait provides a uniform interface for resolving aliases
//! to their root definitions. Resolution traces through:
//! - `InstanceExport` aliases (by looking into instantiated components)
//! - `CoreInstanceExport` aliases (by looking into module internals)
//! - `Outer` aliases (by walking up the parent scope chain)
//!
//! Resolution panics if an alias cannot be resolved (invalid component structure).

use super::{
    AliasInfo, Component, ComponentFuncNode, ComponentInstanceNode, ComponentNode, ComponentRef,
    CoreExportKind, CoreFuncNode, CoreGlobalNode, CoreInstanceNode, CoreMemoryNode, CoreTableNode,
    CoreTypeNode, ExportKind, ModuleNode, ParentScope, ResolvedComponent, ResolvedComponentFunc,
    ResolvedComponentInstance, ResolvedCoreFunc, ResolvedCoreGlobal, ResolvedCoreInstance,
    ResolvedCoreMemory, ResolvedCoreTable, ResolvedCoreType, ResolvedImport, ResolvedModule,
    ResolvedType, ResolvedValue, TypeNode, ValueNode,
};
use wirm::wasmparser::ExternalKind;

// =============================================================================
// Resolve Trait
// =============================================================================

/// Trait for node types that can be resolved to a root (non-aliased) form.
pub trait Resolve<'a> {
    /// The resolved type without `Aliased` variants.
    type Root;

    /// Resolve a node at the given index to its root form.
    /// Follows all alias chains until reaching a definition or import.
    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root;
}

// =============================================================================
// Helper Methods on Component
// =============================================================================

impl<'a> Component<'a> {
    /// Look up an import by index and create a ResolvedImport.
    fn get_resolved_import(&self, import_idx: u32) -> ResolvedImport<'a> {
        let import = self
            .imports
            .get(import_idx as usize)
            .unwrap_or_else(|| panic!("Import index {} out of bounds", import_idx));
        ResolvedImport {
            name: import.name.0,
            ty: import.ty,
        }
    }

    /// Get a parent Component by count (1 = immediate parent, 2 = grandparent, etc.)
    /// Panics if the parent doesn't exist or is not a Component.
    fn get_parent_component(&self, count: u32) -> ComponentRef<'a> {
        if count == 0 {
            panic!("Outer alias count=0 is invalid (refers to current scope)");
        }
        let parent_scope = self.parents.get((count - 1) as usize).unwrap_or_else(|| {
            panic!(
                "Outer alias count={} but only {} parents available",
                count,
                self.parents.len()
            )
        });
        match parent_scope {
            ParentScope::Component(weak) => weak
                .upgrade()
                .expect("Parent component was dropped (Weak reference expired)"),
            ParentScope::ComponentType => {
                panic!("Cannot resolve Outer alias: parent is a ComponentType (not yet supported)")
            }
            ParentScope::InstanceType => {
                panic!("Cannot resolve Outer alias: parent is an InstanceType (not yet supported)")
            }
        }
    }

    /// Resolve an instance export alias by looking into the instantiated component.
    /// Returns (component_ref, export_index).
    fn resolve_instance_export(
        &self,
        instance_idx: u32,
        name: &str,
        expected_kind: ExportKind,
    ) -> (ComponentRef<'a>, u32) {
        let instance = self
            .instances
            .get(instance_idx)
            .unwrap_or_else(|| panic!("Instance index {} out of bounds", instance_idx));

        match instance {
            ComponentInstanceNode::Instantiated { component_idx, .. } => {
                let nested_node = self
                    .components
                    .get(*component_idx)
                    .unwrap_or_else(|| panic!("Component index {} out of bounds", component_idx));
                if let ComponentNode::Defined { component } = nested_node {
                    let nested = component.borrow();
                    let export = nested.exports.get(name).unwrap_or_else(|| {
                        panic!("Export '{}' not found in component {}", name, component_idx)
                    });
                    if export.kind != expected_kind {
                        panic!(
                            "Export '{}' has kind {:?}, expected {:?}",
                            name, export.kind, expected_kind
                        );
                    }
                    (component.clone(), export.index)
                } else {
                    panic!(
                        "Component {} is not defined inline (imported or aliased)",
                        component_idx
                    );
                }
            }
            ComponentInstanceNode::FromExports(exports) => {
                // Find the export with the matching name and kind
                let export = exports.iter().find(|e| e.name == name).unwrap_or_else(|| {
                    panic!(
                        "Export '{}' not found in FromExports instance {}",
                        name, instance_idx
                    )
                });
                let actual_kind = match export.kind {
                    ExportKind::Module => ExportKind::Module,
                    ExportKind::Component => ExportKind::Component,
                    ExportKind::Instance => ExportKind::Instance,
                    ExportKind::Func => ExportKind::Func,
                    ExportKind::Value => ExportKind::Value,
                    ExportKind::Type => ExportKind::Type,
                };
                if actual_kind != expected_kind {
                    panic!(
                        "Export '{}' has kind {:?}, expected {:?}",
                        name, actual_kind, expected_kind
                    );
                }
                // For FromExports, we need to resolve the referenced index in *this* component
                // This is a bit tricky - we return a self-reference conceptually
                panic!(
                    "Cannot resolve InstanceExport through FromExports instance {} - not yet implemented",
                    instance_idx
                );
            }
            ComponentInstanceNode::Imported(_) => {
                panic!(
                    "Cannot resolve InstanceExport alias through imported instance {}",
                    instance_idx
                );
            }
            ComponentInstanceNode::Aliased(_) => {
                panic!(
                    "Cannot resolve InstanceExport alias through aliased instance {} - resolve the instance first",
                    instance_idx
                );
            }
        }
    }

    /// Resolve a core instance export to (module_idx, internal_index).
    /// Traces through the core instance to find the originating module.
    fn resolve_core_instance_export(
        &self,
        instance_idx: u32,
        name: &str,
        expected_kind: CoreExportKind,
    ) -> (u32, u32) {
        let resolved_instance = CoreInstanceNode::resolve(self, instance_idx);

        match resolved_instance {
            ResolvedCoreInstance::Instantiated { module_idx, .. } => {
                // Look up the export in the module
                let module_node = self
                    .modules
                    .get(module_idx)
                    .unwrap_or_else(|| panic!("Module index {} out of bounds", module_idx));

                match module_node {
                    ModuleNode::Defined { module } => {
                        // Access module exports directly
                        let export = module
                            .exports
                            .iter()
                            .find(|e| e.name == name)
                            .unwrap_or_else(|| {
                                panic!("Export '{}' not found in module {}", name, module_idx)
                            });

                        // Verify the export kind matches
                        let actual_kind = match export.kind {
                            ExternalKind::Func => CoreExportKind::Func,
                            ExternalKind::FuncExact => CoreExportKind::Func,
                            ExternalKind::Table => CoreExportKind::Table,
                            ExternalKind::Memory => CoreExportKind::Memory,
                            ExternalKind::Global => CoreExportKind::Global,
                            ExternalKind::Tag => CoreExportKind::Tag,
                        };
                        if actual_kind != expected_kind {
                            panic!(
                                "Export '{}' has kind {:?}, expected {:?}",
                                name, actual_kind, expected_kind
                            );
                        }

                        (module_idx, export.index)
                    }
                    ModuleNode::Imported(import_idx) => {
                        panic!(
                            "Cannot resolve core instance export through imported module (import_idx {}) at module_idx {}",
                            import_idx, module_idx
                        );
                    }
                    ModuleNode::Aliased(_) => {
                        panic!(
                            "Module {} is aliased but should have been resolved",
                            module_idx
                        );
                    }
                }
            }
            ResolvedCoreInstance::FromExports(exports) => {
                // Find the matching export and resolve it recursively
                let export = exports.iter().find(|e| e.name == name).unwrap_or_else(|| {
                    panic!(
                        "Export '{}' not found in FromExports core instance {}",
                        name, instance_idx
                    )
                });

                if export.kind != expected_kind {
                    panic!(
                        "Export '{}' has kind {:?}, expected {:?}",
                        name, export.kind, expected_kind
                    );
                }

                // The export.index refers to an item in the corresponding core index space
                // We need to resolve that item to trace back to the module
                match expected_kind {
                    CoreExportKind::Func => {
                        let resolved = CoreFuncNode::resolve(self, export.index);
                        match resolved {
                            ResolvedCoreFunc::FromModule {
                                module_idx,
                                func_idx,
                            } => (module_idx, func_idx),
                            ResolvedCoreFunc::Lowered { .. } => {
                                panic!(
                                    "FromExports core instance {} exports a lowered func, not a module func",
                                    instance_idx
                                );
                            }
                        }
                    }
                    CoreExportKind::Memory => {
                        let resolved = CoreMemoryNode::resolve(self, export.index);
                        (resolved.module_idx, resolved.memory_idx)
                    }
                    CoreExportKind::Table => {
                        let resolved = CoreTableNode::resolve(self, export.index);
                        (resolved.module_idx, resolved.table_idx)
                    }
                    CoreExportKind::Global => {
                        let resolved = CoreGlobalNode::resolve(self, export.index);
                        (resolved.module_idx, resolved.global_idx)
                    }
                    CoreExportKind::Tag => {
                        panic!("Tag resolution not yet implemented");
                    }
                }
            }
        }
    }
}

// =============================================================================
// Resolve Implementations
// =============================================================================

impl<'a> Resolve<'a> for ModuleNode<'a> {
    type Root = ResolvedModule<'a>;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.modules.get(idx) {
            Some(ModuleNode::Defined { module }) => ResolvedModule::Defined {
                module: module.clone(),
            },
            Some(ModuleNode::Imported(import_idx)) => {
                ResolvedModule::Imported(component.get_resolved_import(*import_idx))
            }
            Some(ModuleNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Module index {} out of bounds", idx),
        }
    }
}

impl<'a> ModuleNode<'a> {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedModule<'a> {
        match alias {
            AliasInfo::InstanceExport { instance_idx, name } => {
                let (nested_ref, export_idx) =
                    component.resolve_instance_export(*instance_idx, name, ExportKind::Module);
                let nested = nested_ref.borrow();
                ModuleNode::resolve(&nested, export_idx)
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                ModuleNode::resolve(&parent, *index)
            }
            AliasInfo::CoreInstanceExport { .. } => {
                panic!("Module cannot be aliased from CoreInstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for ComponentNode<'a> {
    type Root = ResolvedComponent<'a>;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.components.get(idx) {
            Some(ComponentNode::Defined { component: nested }) => ResolvedComponent::Defined {
                component: nested.clone(),
            },
            Some(ComponentNode::Imported(import_idx)) => {
                ResolvedComponent::Imported(component.get_resolved_import(*import_idx))
            }
            Some(ComponentNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Component index {} out of bounds", idx),
        }
    }
}

impl<'a> ComponentNode<'a> {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedComponent<'a> {
        match alias {
            AliasInfo::InstanceExport { instance_idx, name } => {
                let (nested_ref, export_idx) =
                    component.resolve_instance_export(*instance_idx, name, ExportKind::Component);
                let nested = nested_ref.borrow();
                ComponentNode::resolve(&nested, export_idx)
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                ComponentNode::resolve(&parent, *index)
            }
            AliasInfo::CoreInstanceExport { .. } => {
                panic!("Component cannot be aliased from CoreInstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for ComponentInstanceNode {
    type Root = ResolvedComponentInstance<'a>;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.instances.get(idx) {
            Some(ComponentInstanceNode::Instantiated {
                component_idx,
                args,
            }) => ResolvedComponentInstance::Instantiated {
                component_idx: *component_idx,
                args: args.clone(),
            },
            Some(ComponentInstanceNode::FromExports(exports)) => {
                ResolvedComponentInstance::FromExports(exports.clone())
            }
            Some(ComponentInstanceNode::Imported(import_idx)) => {
                ResolvedComponentInstance::Imported(component.get_resolved_import(*import_idx))
            }
            Some(ComponentInstanceNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Instance index {} out of bounds", idx),
        }
    }
}

impl<'a> ComponentInstanceNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedComponentInstance<'a> {
        match alias {
            AliasInfo::InstanceExport { instance_idx, name } => {
                let (nested_ref, export_idx) =
                    component.resolve_instance_export(*instance_idx, name, ExportKind::Instance);
                let nested = nested_ref.borrow();
                ComponentInstanceNode::resolve(&nested, export_idx)
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                ComponentInstanceNode::resolve(&parent, *index)
            }
            AliasInfo::CoreInstanceExport { .. } => {
                panic!("Component instance cannot be aliased from CoreInstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for ComponentFuncNode {
    type Root = ResolvedComponentFunc<'a>;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.funcs.get(idx) {
            Some(ComponentFuncNode::Lifted {
                core_func_idx,
                type_idx,
                options,
            }) => ResolvedComponentFunc::Lifted {
                core_func_idx: *core_func_idx,
                type_idx: *type_idx,
                options: options.clone(),
            },
            Some(ComponentFuncNode::Imported(import_idx)) => {
                ResolvedComponentFunc::Imported(component.get_resolved_import(*import_idx))
            }
            Some(ComponentFuncNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Func index {} out of bounds", idx),
        }
    }
}

impl<'a> ComponentFuncNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedComponentFunc<'a> {
        match alias {
            AliasInfo::InstanceExport { instance_idx, name } => {
                let (nested_ref, export_idx) =
                    component.resolve_instance_export(*instance_idx, name, ExportKind::Func);
                let nested = nested_ref.borrow();
                ComponentFuncNode::resolve(&nested, export_idx)
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                ComponentFuncNode::resolve(&parent, *index)
            }
            AliasInfo::CoreInstanceExport { .. } => {
                panic!("Component func cannot be aliased from CoreInstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for ValueNode {
    type Root = ResolvedValue<'a>;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.values.get(idx) {
            Some(ValueNode::Imported(import_idx)) => component.get_resolved_import(*import_idx),
            Some(ValueNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Value index {} out of bounds", idx),
        }
    }
}

impl<'a> ValueNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedValue<'a> {
        match alias {
            AliasInfo::InstanceExport { instance_idx, name } => {
                let (nested_ref, export_idx) =
                    component.resolve_instance_export(*instance_idx, name, ExportKind::Value);
                let nested = nested_ref.borrow();
                ValueNode::resolve(&nested, export_idx)
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                ValueNode::resolve(&parent, *index)
            }
            AliasInfo::CoreInstanceExport { .. } => {
                panic!("Value cannot be aliased from CoreInstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for TypeNode<'a> {
    type Root = ResolvedType<'a>;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.types.get(idx) {
            Some(TypeNode::Defined(def)) => ResolvedType::Defined(def.clone()),
            Some(TypeNode::Imported(import_idx)) => {
                ResolvedType::Imported(component.get_resolved_import(*import_idx))
            }
            Some(TypeNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Type index {} out of bounds", idx),
        }
    }
}

impl<'a> TypeNode<'a> {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedType<'a> {
        match alias {
            AliasInfo::InstanceExport { instance_idx, name } => {
                let (nested_ref, export_idx) =
                    component.resolve_instance_export(*instance_idx, name, ExportKind::Type);
                let nested = nested_ref.borrow();
                TypeNode::resolve(&nested, export_idx)
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                TypeNode::resolve(&parent, *index)
            }
            AliasInfo::CoreInstanceExport { .. } => {
                panic!("Type cannot be aliased from CoreInstanceExport");
            }
        }
    }
}

// =============================================================================
// Core Node Resolution
// =============================================================================

impl<'a> Resolve<'a> for CoreInstanceNode {
    type Root = ResolvedCoreInstance;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.core_instances.get(idx) {
            Some(CoreInstanceNode::Instantiated { module_idx, args }) => {
                ResolvedCoreInstance::Instantiated {
                    module_idx: *module_idx,
                    args: args.clone(),
                }
            }
            Some(CoreInstanceNode::FromExports(exports)) => {
                ResolvedCoreInstance::FromExports(exports.clone())
            }
            Some(CoreInstanceNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Core instance index {} out of bounds", idx),
        }
    }
}

impl<'a> CoreInstanceNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedCoreInstance {
        match alias {
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                CoreInstanceNode::resolve(&parent, *index)
            }
            AliasInfo::InstanceExport { .. } => {
                panic!("Core instance cannot be aliased from component InstanceExport");
            }
            AliasInfo::CoreInstanceExport { .. } => {
                panic!("Core instance cannot be aliased from CoreInstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for CoreFuncNode {
    type Root = ResolvedCoreFunc;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.core_funcs.get(idx) {
            Some(CoreFuncNode::Lowered { func_idx, options }) => ResolvedCoreFunc::Lowered {
                func_idx: *func_idx,
                options: options.clone(),
            },
            Some(CoreFuncNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Core func index {} out of bounds", idx),
        }
    }
}

impl<'a> CoreFuncNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedCoreFunc {
        match alias {
            AliasInfo::CoreInstanceExport { instance_idx, name } => {
                let (module_idx, func_idx) = component.resolve_core_instance_export(
                    *instance_idx,
                    name,
                    CoreExportKind::Func,
                );
                ResolvedCoreFunc::FromModule {
                    module_idx,
                    func_idx,
                }
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                CoreFuncNode::resolve(&parent, *index)
            }
            AliasInfo::InstanceExport { .. } => {
                panic!("Core func cannot be aliased from component InstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for CoreMemoryNode {
    type Root = ResolvedCoreMemory;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.core_memories.get(idx) {
            Some(CoreMemoryNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Core memory index {} out of bounds", idx),
        }
    }
}

impl<'a> CoreMemoryNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedCoreMemory {
        match alias {
            AliasInfo::CoreInstanceExport { instance_idx, name } => {
                let (module_idx, memory_idx) = component.resolve_core_instance_export(
                    *instance_idx,
                    name,
                    CoreExportKind::Memory,
                );
                ResolvedCoreMemory {
                    module_idx,
                    memory_idx,
                }
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                CoreMemoryNode::resolve(&parent, *index)
            }
            AliasInfo::InstanceExport { .. } => {
                panic!("Core memory cannot be aliased from component InstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for CoreTableNode {
    type Root = ResolvedCoreTable;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.core_tables.get(idx) {
            Some(CoreTableNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Core table index {} out of bounds", idx),
        }
    }
}

impl<'a> CoreTableNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedCoreTable {
        match alias {
            AliasInfo::CoreInstanceExport { instance_idx, name } => {
                let (module_idx, table_idx) = component.resolve_core_instance_export(
                    *instance_idx,
                    name,
                    CoreExportKind::Table,
                );
                ResolvedCoreTable {
                    module_idx,
                    table_idx,
                }
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                CoreTableNode::resolve(&parent, *index)
            }
            AliasInfo::InstanceExport { .. } => {
                panic!("Core table cannot be aliased from component InstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for CoreGlobalNode {
    type Root = ResolvedCoreGlobal;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.core_globals.get(idx) {
            Some(CoreGlobalNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Core global index {} out of bounds", idx),
        }
    }
}

impl<'a> CoreGlobalNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedCoreGlobal {
        match alias {
            AliasInfo::CoreInstanceExport { instance_idx, name } => {
                let (module_idx, global_idx) = component.resolve_core_instance_export(
                    *instance_idx,
                    name,
                    CoreExportKind::Global,
                );
                ResolvedCoreGlobal {
                    module_idx,
                    global_idx,
                }
            }
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                CoreGlobalNode::resolve(&parent, *index)
            }
            AliasInfo::InstanceExport { .. } => {
                panic!("Core global cannot be aliased from component InstanceExport");
            }
        }
    }
}

impl<'a> Resolve<'a> for CoreTypeNode {
    type Root = ResolvedCoreType;

    fn resolve(component: &Component<'a>, idx: u32) -> Self::Root {
        match component.core_types.get(idx) {
            Some(CoreTypeNode::Defined(def)) => ResolvedCoreType::Defined(def.clone()),
            Some(CoreTypeNode::Aliased(alias)) => Self::follow_alias(component, alias),
            None => panic!("Core type index {} out of bounds", idx),
        }
    }
}

impl<'a> CoreTypeNode {
    fn follow_alias(component: &Component<'a>, alias: &AliasInfo) -> ResolvedCoreType {
        match alias {
            AliasInfo::Outer { count, index } => {
                let parent_ref = component.get_parent_component(*count);
                let parent = parent_ref.borrow();
                CoreTypeNode::resolve(&parent, *index)
            }
            AliasInfo::InstanceExport { .. } => {
                panic!("Core type cannot be aliased from component InstanceExport");
            }
            AliasInfo::CoreInstanceExport { .. } => {
                panic!("Core type cannot be aliased from CoreInstanceExport");
            }
        }
    }
}

// =============================================================================
// Convenience methods on Component
// =============================================================================

impl<'a> Component<'a> {
    /// Resolve a module by index.
    pub fn resolve_module(&self, idx: u32) -> ResolvedModule<'a> {
        ModuleNode::resolve(self, idx)
    }

    /// Resolve a component by index.
    pub fn resolve_component(&self, idx: u32) -> ResolvedComponent<'a> {
        ComponentNode::resolve(self, idx)
    }

    /// Resolve a component instance by index.
    pub fn resolve_component_instance(&self, idx: u32) -> ResolvedComponentInstance<'a> {
        ComponentInstanceNode::resolve(self, idx)
    }

    /// Resolve a component function by index.
    pub fn resolve_component_func(&self, idx: u32) -> ResolvedComponentFunc<'a> {
        ComponentFuncNode::resolve(self, idx)
    }

    /// Resolve a value by index.
    pub fn resolve_value(&self, idx: u32) -> ResolvedValue<'a> {
        ValueNode::resolve(self, idx)
    }

    /// Resolve a type by index.
    pub fn resolve_type(&self, idx: u32) -> ResolvedType<'a> {
        TypeNode::resolve(self, idx)
    }

    /// Resolve a core instance by index.
    pub fn resolve_core_instance(&self, idx: u32) -> ResolvedCoreInstance {
        CoreInstanceNode::resolve(self, idx)
    }

    /// Resolve a core function by index.
    pub fn resolve_core_func(&self, idx: u32) -> ResolvedCoreFunc {
        CoreFuncNode::resolve(self, idx)
    }

    /// Resolve a core memory by index.
    pub fn resolve_core_memory(&self, idx: u32) -> ResolvedCoreMemory {
        CoreMemoryNode::resolve(self, idx)
    }

    /// Resolve a core table by index.
    pub fn resolve_core_table(&self, idx: u32) -> ResolvedCoreTable {
        CoreTableNode::resolve(self, idx)
    }

    /// Resolve a core global by index.
    pub fn resolve_core_global(&self, idx: u32) -> ResolvedCoreGlobal {
        CoreGlobalNode::resolve(self, idx)
    }

    /// Resolve a core type by index.
    pub fn resolve_core_type(&self, idx: u32) -> ResolvedCoreType {
        CoreTypeNode::resolve(self, idx)
    }

    // =========================================================================
    // Utility: Get all dependencies of an instance
    // =========================================================================

    /// Returns the module index and all argument indices for an instantiated core instance.
    pub fn core_instance_dependencies(&self, idx: u32) -> Option<(u32, Vec<u32>)> {
        match self.core_instances.get(idx)? {
            CoreInstanceNode::Instantiated { module_idx, args } => {
                let arg_indices: Vec<u32> = args.iter().map(|a| a.index).collect();
                Some((*module_idx, arg_indices))
            }
            _ => None,
        }
    }

    /// Returns the component index and all argument indices for an instantiated component instance.
    pub fn instance_dependencies(&self, idx: u32) -> Option<(u32, Vec<u32>)> {
        match self.instances.get(idx)? {
            ComponentInstanceNode::Instantiated {
                component_idx,
                args,
            } => {
                let arg_indices: Vec<u32> = args.iter().map(|a| a.index).collect();
                Some((*component_idx, arg_indices))
            }
            _ => None,
        }
    }
}
