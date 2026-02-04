//! Parsing logic to convert raw bytes into our Component IR.

use anyhow::{Result, bail};
use std::cell::RefCell;
use std::rc::Rc;
use wirm::wasmparser::{
    self, CanonicalOption, ComponentAlias, ComponentExport, ComponentExternalKind, ComponentImport,
    ComponentInstance, ComponentTypeRef, ExternalKind, Instance, Parser, Payload,
};

use crate::ir::{
    AliasInfo, Component, ComponentFuncNode, ComponentInstanceNode, ComponentNode, ComponentRef,
    ComponentTypeDef, CoreExportKind, CoreFuncNode, CoreGlobalNode, CoreInlineExport,
    CoreInstanceNode, CoreInstantiationArg, CoreMemoryNode, CoreTableNode, CoreTypeDef,
    CoreTypeNode, ExportKind, ExportNode, ImportInfo, InlineExport,
    InstantiationArg, InstantiationArgKind, ModuleNode, ParentScope, TypeNode, ValueNode,
};

/// Parse a WebAssembly Component from bytes into our IR.
///
/// Returns a ComponentRef (Rc<RefCell<Component>>) with nested components also wrapped.
/// Each component stores its parent chain as Weak references for Outer alias resolution.
pub fn parse_component<'a>(bytes: &'a [u8]) -> Result<ComponentRef<'a>> {
    parse_component_with_parents(bytes, vec![])
}

/// Parse a component with an explicit parent chain for Outer alias resolution.
fn parse_component_with_parents<'a>(
    bytes: &'a [u8],
    parents: Vec<ParentScope<'a>>,
) -> Result<ComponentRef<'a>> {
    let parser = Parser::new(0);

    // Create the component wrapped in Rc<RefCell> immediately
    let component_ref: ComponentRef = Rc::new(RefCell::new(Component {
        parents,
        ..Default::default()
    }));

    let mut depth: u32 = 0;

    for payload in parser.parse_all(bytes) {
        let payload = payload?;
        handle_payload(&component_ref, payload, bytes, &mut depth)?;
    }

    Ok(component_ref)
}

fn handle_payload<'a>(
    component_ref: &ComponentRef<'a>,
    payload: Payload<'a>,
    bytes: &'a [u8],
    depth: &mut u32,
) -> Result<()> {
    use Payload::*;

    // Borrow the component mutably for modifications
    let mut component = component_ref.borrow_mut();

    match payload {
        Version { encoding, .. } => {
            // Only check encoding at the top level (depth 0)
            if *depth == 0 && encoding != wasmparser::Encoding::Component {
                bail!("Expected a Component, got a Module");
            }
        }

        // =================================================================
        // Component Type Section
        // =================================================================
        ComponentTypeSection(reader) => {
            for ty in reader {
                let ty = ty?;
                let node = parse_component_type(ty);
                component.types.push(node);
            }
        }

        // =================================================================
        // Component Import Section
        // =================================================================
        ComponentImportSection(reader) => {
            for import in reader {
                let import = import?;
                handle_component_import(&mut component, import);
            }
        }

        // =================================================================
        // Component Instance Section
        // =================================================================
        ComponentInstanceSection(reader) => {
            for instance in reader {
                let instance = instance?;
                let node = parse_component_instance(instance);
                component.instances.push(node);
            }
        }

        // =================================================================
        // Core Instance Section
        // =================================================================
        InstanceSection(reader) => {
            for instance in reader {
                let instance = instance?;
                let node = parse_core_instance(instance);
                component.core_instances.push(node);
            }
        }

        // =================================================================
        // Component Alias Section
        // =================================================================
        ComponentAliasSection(reader) => {
            for alias in reader {
                let alias = alias?;
                handle_component_alias(&mut component, alias);
            }
        }

        // =================================================================
        // Canonical Section (lift/lower)
        // =================================================================
        ComponentCanonicalSection(reader) => {
            for canon in reader {
                let canon = canon?;
                handle_canonical(&mut component, canon);
            }
        }

        // =================================================================
        // Component Export Section
        // =================================================================
        ComponentExportSection(reader) => {
            for export in reader {
                let export = export?;
                handle_component_export(&mut component, export);
            }
        }

        // =================================================================
        // Module Section (inline core module)
        // =================================================================
        ModuleSection {
            parser: _,
            unchecked_range,
        } => {
            // Increment depth to skip nested Version payloads
            *depth += 1;
            // Extract the module bytes from the range
            let module_bytes = &bytes[unchecked_range.start..unchecked_range.end];
            // Parse the module bytes into wirm Module IR
            let parsed_module = wirm::Module::parse(module_bytes, false, false)
                .map_err(|e| anyhow::anyhow!("Failed to parse module: {}", e))?;
            component.modules.push(ModuleNode::Defined {
                module: parsed_module,
            });
        }

        // =================================================================
        // Component Section (nested component)
        // =================================================================
        ComponentSection {
            parser: _,
            unchecked_range,
        } => {
            // Increment depth to skip nested Version payloads
            *depth += 1;
            let nested_bytes = &bytes[unchecked_range.start..unchecked_range.end];

            // Build parent chain: current component's parents + current component itself
            let mut new_parents = component.parents.clone();
            new_parents.push(ParentScope::Component(Rc::downgrade(component_ref)));

            // Drop the borrow before recursively parsing
            drop(component);

            // Parse nested component with parent chain
            let nested_ref = parse_component_with_parents(nested_bytes, new_parents)?;

            // Re-borrow to add the nested component
            component_ref
                .borrow_mut()
                .components
                .push(ComponentNode::Defined {
                    component: nested_ref,
                });
            return Ok(());
        }

        // =================================================================
        // Core Type Section
        // =================================================================
        CoreTypeSection(reader) => {
            for ty in reader {
                let _ty = ty?;
                // Simplified: just mark as defined
                component
                    .core_types
                    .push(CoreTypeNode::Defined(CoreTypeDef::Func));
            }
        }

        // Ignored payloads
        End { .. } => {
            // Decrement depth when exiting nested module/component
            *depth = depth.saturating_sub(1);
        }
        CustomSection { .. } => {}
        _ => {
            // Other payloads we don't handle yet
        }
    }

    Ok(())
}

// =============================================================================
// Import Handling
// =============================================================================

fn handle_component_import<'a>(component: &mut Component<'a>, import: ComponentImport<'a>) {
    let info = ImportInfo {
        name: import.name.0.to_string(),
        url: None,
    };

    // Add to the appropriate index space based on type
    match import.ty {
        ComponentTypeRef::Module(_) => {
            component.modules.push(ModuleNode::Imported(info));
        }
        ComponentTypeRef::Func(_) => {
            component.funcs.push(ComponentFuncNode::Imported(info));
        }
        ComponentTypeRef::Value(_) => {
            component.values.push(ValueNode::Imported(info));
        }
        ComponentTypeRef::Type(..) => {
            component.types.push(TypeNode::Imported(info));
        }
        ComponentTypeRef::Instance(_) => {
            component
                .instances
                .push(ComponentInstanceNode::Imported(info));
        }
        ComponentTypeRef::Component(_) => {
            component.components.push(ComponentNode::Imported(info));
        }
    };

    // Store the full import (name + type reference)
    component.imports.push(import);
}

// =============================================================================
// Alias Handling
// =============================================================================

fn handle_component_alias(component: &mut Component, alias: ComponentAlias) {
    match alias {
        ComponentAlias::InstanceExport {
            kind,
            instance_index,
            name,
        } => {
            let info = AliasInfo::InstanceExport {
                instance_idx: instance_index,
                name: name.to_string(),
            };
            push_alias_by_kind(component, kind, info);
        }
        ComponentAlias::CoreInstanceExport {
            kind,
            instance_index,
            name,
        } => {
            let info = AliasInfo::CoreInstanceExport {
                instance_idx: instance_index,
                name: name.to_string(),
            };
            push_core_alias_by_kind(component, kind, info);
        }
        ComponentAlias::Outer { kind, count, index } => {
            let info = AliasInfo::Outer { count, index };
            push_outer_alias_by_kind(component, kind, info);
        }
    }
}

fn push_alias_by_kind(component: &mut Component, kind: ComponentExternalKind, info: AliasInfo) {
    match kind {
        ComponentExternalKind::Module => {
            component.modules.push(ModuleNode::Aliased(info));
        }
        ComponentExternalKind::Func => {
            component.funcs.push(ComponentFuncNode::Aliased(info));
        }
        ComponentExternalKind::Value => {
            component.values.push(ValueNode::Aliased(info));
        }
        ComponentExternalKind::Type => {
            component.types.push(TypeNode::Aliased(info));
        }
        ComponentExternalKind::Instance => {
            component
                .instances
                .push(ComponentInstanceNode::Aliased(info));
        }
        ComponentExternalKind::Component => {
            component.components.push(ComponentNode::Aliased(info));
        }
    }
}

fn push_core_alias_by_kind(component: &mut Component, kind: ExternalKind, info: AliasInfo) {
    match kind {
        ExternalKind::Func | ExternalKind::FuncExact => {
            component.core_funcs.push(CoreFuncNode::Aliased(info));
        }
        ExternalKind::Table => {
            component.core_tables.push(CoreTableNode::Aliased(info));
        }
        ExternalKind::Memory => {
            component.core_memories.push(CoreMemoryNode::Aliased(info));
        }
        ExternalKind::Global => {
            component.core_globals.push(CoreGlobalNode::Aliased(info));
        }
        ExternalKind::Tag => {
            // Tags not tracked separately for now
        }
    }
}

fn push_outer_alias_by_kind(
    component: &mut Component,
    kind: wasmparser::ComponentOuterAliasKind,
    info: AliasInfo,
) {
    use wasmparser::ComponentOuterAliasKind::*;
    match kind {
        CoreModule => {
            component.modules.push(ModuleNode::Aliased(info));
        }
        CoreType => {
            component.core_types.push(CoreTypeNode::Aliased(info));
        }
        Type => {
            component.types.push(TypeNode::Aliased(info));
        }
        Component => {
            component.components.push(ComponentNode::Aliased(info));
        }
    }
}

// =============================================================================
// Canonical Function Handling
// =============================================================================

fn handle_canonical(component: &mut Component, canon: wasmparser::CanonicalFunction) {
    use wasmparser::CanonicalFunction::*;

    match canon {
        Lift {
            core_func_index,
            type_index,
            options,
        } => {
            let opts = parse_canon_options(&options);
            component.funcs.push(ComponentFuncNode::Lifted {
                core_func_idx: core_func_index,
                type_idx: type_index,
                options: opts,
            });
        }
        Lower {
            func_index,
            options,
        } => {
            let opts = parse_canon_options(&options);
            component.core_funcs.push(CoreFuncNode::Lowered {
                func_idx: func_index,
                options: opts,
            });
        }
        // Other canonical functions (resource operations, etc.) - ignore for now
        _ => {}
    }
}

fn parse_canon_options(options: &[wasmparser::CanonicalOption]) -> Vec<CanonicalOption> {
    options.to_vec()
}

// =============================================================================
// Export Handling
// =============================================================================

fn handle_component_export(component: &mut Component, export: ComponentExport) {
    let kind = match export.kind {
        ComponentExternalKind::Module => ExportKind::Module,
        ComponentExternalKind::Func => ExportKind::Func,
        ComponentExternalKind::Value => ExportKind::Value,
        ComponentExternalKind::Type => ExportKind::Type,
        ComponentExternalKind::Instance => ExportKind::Instance,
        ComponentExternalKind::Component => ExportKind::Component,
    };

    let node = ExportNode {
        name: export.name.0.to_string(),
        kind,
        index: export.index,
        ty: export.ty.map(|t| match t {
            ComponentTypeRef::Module(i) => i,
            ComponentTypeRef::Func(i) => i,
            ComponentTypeRef::Value(_) => 0, // Simplified
            ComponentTypeRef::Type(..) => 0,
            ComponentTypeRef::Instance(i) => i,
            ComponentTypeRef::Component(i) => i,
        }),
    };

    component.exports.insert(export.name.0.to_string(), node);
}

// =============================================================================
// Instance Parsing
// =============================================================================

fn parse_component_instance(instance: ComponentInstance) -> ComponentInstanceNode {
    match instance {
        ComponentInstance::Instantiate {
            component_index,
            args,
        } => {
            let args = args
                .iter()
                .map(|arg| InstantiationArg {
                    name: arg.name.to_string(),
                    kind: convert_component_arg_kind(arg.kind),
                    index: arg.index,
                })
                .collect();
            ComponentInstanceNode::Instantiated {
                component_idx: component_index,
                args,
            }
        }
        ComponentInstance::FromExports(exports) => {
            let exports = exports
                .iter()
                .map(|e| InlineExport {
                    name: e.name.0.to_string(),
                    kind: convert_export_kind(e.kind),
                    index: e.index,
                })
                .collect();
            ComponentInstanceNode::FromExports(exports)
        }
    }
}

fn parse_core_instance(instance: Instance) -> CoreInstanceNode {
    match instance {
        Instance::Instantiate { module_index, args } => {
            let args = args
                .iter()
                .map(|arg| CoreInstantiationArg {
                    name: arg.name.to_string(),
                    kind: convert_core_arg_kind(arg.kind),
                    index: arg.index,
                })
                .collect();
            CoreInstanceNode::Instantiated {
                module_idx: module_index,
                args,
            }
        }
        Instance::FromExports(exports) => {
            let exports = exports
                .iter()
                .map(|e| CoreInlineExport {
                    name: e.name.to_string(),
                    kind: convert_core_export_kind(e.kind),
                    index: e.index,
                })
                .collect();
            CoreInstanceNode::FromExports(exports)
        }
    }
}

// =============================================================================
// Type Parsing
// =============================================================================

fn parse_component_type(ty: wasmparser::ComponentType) -> TypeNode {
    use wasmparser::ComponentType::*;
    let def = match ty {
        Defined(_) => ComponentTypeDef::Defined,
        Func(_) => ComponentTypeDef::Func,
        Component(_) => ComponentTypeDef::Component,
        Instance(_) => ComponentTypeDef::Instance,
        Resource { .. } => ComponentTypeDef::Resource,
    };
    TypeNode::Defined(def)
}

// =============================================================================
// Kind Conversions
// =============================================================================

fn convert_component_arg_kind(kind: wasmparser::ComponentExternalKind) -> InstantiationArgKind {
    match kind {
        wasmparser::ComponentExternalKind::Module => InstantiationArgKind::Module,
        wasmparser::ComponentExternalKind::Func => InstantiationArgKind::Func,
        wasmparser::ComponentExternalKind::Value => InstantiationArgKind::Value,
        wasmparser::ComponentExternalKind::Type => InstantiationArgKind::Type,
        wasmparser::ComponentExternalKind::Instance => InstantiationArgKind::Instance,
        wasmparser::ComponentExternalKind::Component => InstantiationArgKind::Component,
    }
}

fn convert_export_kind(kind: wasmparser::ComponentExternalKind) -> ExportKind {
    match kind {
        wasmparser::ComponentExternalKind::Module => ExportKind::Module,
        wasmparser::ComponentExternalKind::Func => ExportKind::Func,
        wasmparser::ComponentExternalKind::Value => ExportKind::Value,
        wasmparser::ComponentExternalKind::Type => ExportKind::Type,
        wasmparser::ComponentExternalKind::Instance => ExportKind::Instance,
        wasmparser::ComponentExternalKind::Component => ExportKind::Component,
    }
}

fn convert_core_arg_kind(kind: wasmparser::InstantiationArgKind) -> CoreExportKind {
    match kind {
        wasmparser::InstantiationArgKind::Instance => CoreExportKind::Func, // Simplified
    }
}

fn convert_core_export_kind(kind: wasmparser::ExternalKind) -> CoreExportKind {
    match kind {
        wasmparser::ExternalKind::Func | wasmparser::ExternalKind::FuncExact => {
            CoreExportKind::Func
        }
        wasmparser::ExternalKind::Table => CoreExportKind::Table,
        wasmparser::ExternalKind::Memory => CoreExportKind::Memory,
        wasmparser::ExternalKind::Global => CoreExportKind::Global,
        wasmparser::ExternalKind::Tag => CoreExportKind::Tag,
    }
}
