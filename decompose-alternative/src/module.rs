//! Module IR for parsing and re-encoding WebAssembly core modules.
//!
//! This module provides a `Module` struct that can parse module bytes using wasmparser
//! and re-encode them using wasm-encoder for round-trip support.

use anyhow::{Result, bail};
use std::borrow::Cow;

/// A parsed WebAssembly core module.
#[derive(Debug, Clone)]
pub struct Module {
    /// Type section - function signatures
    pub types: Vec<FuncType>,

    /// Import section - external dependencies
    pub imports: Vec<Import>,

    /// Function section - maps local function indices to type indices
    pub functions: Vec<u32>,

    /// Table section
    pub tables: Vec<TableType>,

    /// Memory section
    pub memories: Vec<MemoryType>,

    /// Global section
    pub globals: Vec<Global>,

    /// Export section
    pub exports: Vec<Export>,

    /// Start function index (optional)
    pub start: Option<u32>,

    /// Element section - table initialization
    pub elements: Vec<Element>,

    /// Code section - function bodies (stored as raw bytes for now)
    pub code: Vec<FunctionBody>,

    /// Data section - memory initialization
    pub data: Vec<DataSegment>,

    /// Data count (for validation)
    pub data_count: Option<u32>,

    /// Custom sections (preserved for round-trip)
    pub custom_sections: Vec<CustomSection>,
}

// =============================================================================
// Type definitions
// =============================================================================

#[derive(Debug, Clone)]
pub struct FuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
    V128,
    FuncRef,
    ExternRef,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub module: String,
    pub name: String,
    pub ty: ImportType,
}

#[derive(Debug, Clone)]
pub enum ImportType {
    Func(u32), // type index
    Table(TableType),
    Memory(MemoryType),
    Global(GlobalType),
    Tag(TagType),
}

#[derive(Debug, Clone, Copy)]
pub struct TableType {
    pub element_type: RefType,
    pub initial: u64,
    pub maximum: Option<u64>,
    pub table64: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefType {
    FuncRef,
    ExternRef,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryType {
    pub initial: u64,
    pub maximum: Option<u64>,
    pub memory64: bool,
    pub shared: bool,
    pub page_size_log2: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
pub struct GlobalType {
    pub val_type: ValType,
    pub mutable: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct TagType {
    pub func_type_idx: u32,
}

#[derive(Debug, Clone)]
pub struct Global {
    pub ty: GlobalType,
    /// Init expression stored as raw bytes
    pub init_expr: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Export {
    pub name: String,
    pub kind: ExportKind,
    pub index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportKind {
    Func,
    Table,
    Memory,
    Global,
    Tag,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub kind: ElementKind,
    pub items: ElementItems,
}

#[derive(Debug, Clone)]
pub enum ElementKind {
    Passive,
    Active {
        table_index: u32,
        offset_expr: Vec<u8>,
    },
    Declared,
}

#[derive(Debug, Clone)]
pub enum ElementItems {
    Functions(Vec<u32>),
    Expressions { ty: RefType, exprs: Vec<Vec<u8>> },
}

#[derive(Debug, Clone)]
pub struct FunctionBody {
    /// Local variable declarations: (count, type)
    pub locals: Vec<(u32, ValType)>,
    /// Raw bytes of the function body (instructions)
    pub body: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DataSegment {
    pub kind: DataKind,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum DataKind {
    Passive,
    Active {
        memory_index: u32,
        offset_expr: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
pub struct CustomSection {
    pub name: String,
    pub data: Vec<u8>,
}

// =============================================================================
// Parsing
// =============================================================================

impl Module {
    /// Parse a WebAssembly module from bytes.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        use wasmparser::{Parser, Payload};

        let mut module = Module {
            types: Vec::new(),
            imports: Vec::new(),
            functions: Vec::new(),
            tables: Vec::new(),
            memories: Vec::new(),
            globals: Vec::new(),
            exports: Vec::new(),
            start: None,
            elements: Vec::new(),
            code: Vec::new(),
            data: Vec::new(),
            data_count: None,
            custom_sections: Vec::new(),
        };

        let parser = Parser::new(0);
        for payload in parser.parse_all(bytes) {
            let payload = payload?;
            match payload {
                Payload::Version { encoding, .. } => {
                    if encoding != wasmparser::Encoding::Module {
                        bail!("Expected a Module, got a Component");
                    }
                }

                Payload::TypeSection(reader) => {
                    for rec_group in reader {
                        let rec_group = rec_group?;
                        // For now, handle simple function types only
                        for sub_type in rec_group.into_types() {
                            if let wasmparser::CompositeInnerType::Func(ft) =
                                &sub_type.composite_type.inner
                            {
                                module.types.push(FuncType {
                                    params: ft
                                        .params()
                                        .iter()
                                        .map(|t| convert_val_type(*t))
                                        .collect::<Result<_>>()?,
                                    results: ft
                                        .results()
                                        .iter()
                                        .map(|t| convert_val_type(*t))
                                        .collect::<Result<_>>()?,
                                });
                            }
                        }
                    }
                }

                Payload::ImportSection(reader) => {
                    for group in reader {
                        let group = group?;
                        // wasmparser 0.244 uses Imports enum with compact formats
                        match group {
                            wasmparser::Imports::Single(_, wp_import) => {
                                module.imports.push(Import {
                                    module: wp_import.module.to_string(),
                                    name: wp_import.name.to_string(),
                                    ty: convert_import_type(&wp_import.ty)?,
                                });
                            }
                            wasmparser::Imports::Compact1 {
                                module: mod_name,
                                items,
                            } => {
                                for item in items {
                                    let item = item?;
                                    module.imports.push(Import {
                                        module: mod_name.to_string(),
                                        name: item.name.to_string(),
                                        ty: convert_import_type(&item.ty)?,
                                    });
                                }
                            }
                            wasmparser::Imports::Compact2 {
                                module: mod_name,
                                ty,
                                names,
                            } => {
                                for name in names {
                                    let name = name?;
                                    module.imports.push(Import {
                                        module: mod_name.to_string(),
                                        name: name.to_string(),
                                        ty: convert_import_type(&ty)?,
                                    });
                                }
                            }
                        }
                    }
                }

                Payload::FunctionSection(reader) => {
                    for func in reader {
                        let func = func?;
                        module.functions.push(func);
                    }
                }

                Payload::TableSection(reader) => {
                    for table in reader {
                        let table = table?;
                        module.tables.push(convert_table_type(table.ty)?);
                    }
                }

                Payload::MemorySection(reader) => {
                    for memory in reader {
                        let memory = memory?;
                        module.memories.push(convert_memory_type(memory)?);
                    }
                }

                Payload::GlobalSection(reader) => {
                    for global in reader {
                        let global = global?;
                        module.globals.push(Global {
                            ty: GlobalType {
                                val_type: convert_val_type(global.ty.content_type)?,
                                mutable: global.ty.mutable,
                            },
                            init_expr: extract_const_expr(&global.init_expr),
                        });
                    }
                }

                Payload::ExportSection(reader) => {
                    for export in reader {
                        let export = export?;
                        module.exports.push(Export {
                            name: export.name.to_string(),
                            kind: convert_export_kind(export.kind),
                            index: export.index,
                        });
                    }
                }

                Payload::StartSection { func, .. } => {
                    module.start = Some(func);
                }

                Payload::ElementSection(reader) => {
                    for element in reader {
                        let element = element?;
                        module.elements.push(convert_element(element)?);
                    }
                }

                Payload::CodeSectionStart { count, .. } => {
                    module.code.reserve(count as usize);
                }

                Payload::CodeSectionEntry(body) => {
                    let locals_reader = body.get_locals_reader()?;
                    let mut locals = Vec::new();
                    for local in locals_reader {
                        let (count, ty) = local?;
                        locals.push((count, convert_val_type(ty)?));
                    }

                    // Get the raw body bytes
                    let body_bytes = body
                        .get_binary_reader()
                        .read_bytes(body.get_binary_reader().bytes_remaining())?;

                    module.code.push(FunctionBody {
                        locals,
                        body: body_bytes.to_vec(),
                    });
                }

                Payload::DataSection(reader) => {
                    for data in reader {
                        let data = data?;
                        module.data.push(convert_data_segment(data)?);
                    }
                }

                Payload::DataCountSection { count, .. } => {
                    module.data_count = Some(count);
                }

                Payload::CustomSection(reader) => {
                    module.custom_sections.push(CustomSection {
                        name: reader.name().to_string(),
                        data: reader.data().to_vec(),
                    });
                }

                Payload::End { .. } => {}

                _ => {
                    // Ignore other payloads (TagSection, etc.)
                }
            }
        }

        Ok(module)
    }

    /// Encode the module back to bytes.
    pub fn encode(&self) -> Vec<u8> {
        use wasm_encoder as enc;

        let mut module = enc::Module::new();

        // Type section
        if !self.types.is_empty() {
            let mut types = enc::TypeSection::new();
            for ft in &self.types {
                let params: Vec<_> = ft.params.iter().map(|t| encode_val_type(*t)).collect();
                let results: Vec<_> = ft.results.iter().map(|t| encode_val_type(*t)).collect();
                types.ty().function(params, results);
            }
            module.section(&types);
        }

        // Import section
        if !self.imports.is_empty() {
            let mut imports = enc::ImportSection::new();
            for import in &self.imports {
                let entity = encode_import_type(&import.ty);
                imports.import(&import.module, &import.name, entity);
            }
            module.section(&imports);
        }

        // Function section
        if !self.functions.is_empty() {
            let mut functions = enc::FunctionSection::new();
            for &type_idx in &self.functions {
                functions.function(type_idx);
            }
            module.section(&functions);
        }

        // Table section
        if !self.tables.is_empty() {
            let mut tables = enc::TableSection::new();
            for table in &self.tables {
                tables.table(encode_table_type(table));
            }
            module.section(&tables);
        }

        // Memory section
        if !self.memories.is_empty() {
            let mut memories = enc::MemorySection::new();
            for memory in &self.memories {
                memories.memory(encode_memory_type(memory));
            }
            module.section(&memories);
        }

        // Global section
        if !self.globals.is_empty() {
            let mut globals = enc::GlobalSection::new();
            for global in &self.globals {
                let gt = enc::GlobalType {
                    val_type: encode_val_type(global.ty.val_type),
                    mutable: global.ty.mutable,
                    shared: false,
                };
                globals.global(gt, &enc::ConstExpr::raw(global.init_expr.iter().copied()));
            }
            module.section(&globals);
        }

        // Export section
        if !self.exports.is_empty() {
            let mut exports = enc::ExportSection::new();
            for export in &self.exports {
                exports.export(&export.name, encode_export_kind(export.kind), export.index);
            }
            module.section(&exports);
        }

        // Start section
        if let Some(func_idx) = self.start {
            module.section(&enc::StartSection {
                function_index: func_idx,
            });
        }

        // Element section
        if !self.elements.is_empty() {
            let mut elements = enc::ElementSection::new();
            for elem in &self.elements {
                encode_element(&mut elements, elem);
            }
            module.section(&elements);
        }

        // Data count section (must come before code section if present)
        if let Some(count) = self.data_count {
            module.section(&enc::DataCountSection { count });
        }

        // Code section
        if !self.code.is_empty() {
            let mut code = enc::CodeSection::new();
            for func_body in &self.code {
                let mut func = enc::Function::new(
                    func_body
                        .locals
                        .iter()
                        .map(|(count, ty)| (*count, encode_val_type(*ty))),
                );
                func.raw(func_body.body.iter().copied());
                code.function(&func);
            }
            module.section(&code);
        }

        // Data section
        if !self.data.is_empty() {
            let mut data = enc::DataSection::new();
            for seg in &self.data {
                match &seg.kind {
                    DataKind::Passive => {
                        data.passive(seg.data.iter().copied());
                    }
                    DataKind::Active {
                        memory_index,
                        offset_expr,
                    } => {
                        data.active(
                            *memory_index,
                            &enc::ConstExpr::raw(offset_expr.iter().copied()),
                            seg.data.iter().copied(),
                        );
                    }
                }
            }
            module.section(&data);
        }

        // Custom sections
        for custom in &self.custom_sections {
            module.section(&enc::CustomSection {
                name: Cow::Borrowed(&custom.name),
                data: Cow::Borrowed(&custom.data),
            });
        }

        module.finish()
    }

    /// Build an import map: (module_name, member_name) -> import index
    pub fn import_map(&self) -> std::collections::HashMap<(&str, &str), u32> {
        self.imports
            .iter()
            .enumerate()
            .map(|(i, imp)| ((&imp.module[..], &imp.name[..]), i as u32))
            .collect()
    }

    /// Get the number of imported functions (for calculating local function indices).
    pub fn num_imported_funcs(&self) -> u32 {
        self.imports
            .iter()
            .filter(|i| matches!(i.ty, ImportType::Func(_)))
            .count() as u32
    }

    /// Get the number of imported tables.
    pub fn num_imported_tables(&self) -> u32 {
        self.imports
            .iter()
            .filter(|i| matches!(i.ty, ImportType::Table(_)))
            .count() as u32
    }

    /// Get the number of imported memories.
    pub fn num_imported_memories(&self) -> u32 {
        self.imports
            .iter()
            .filter(|i| matches!(i.ty, ImportType::Memory(_)))
            .count() as u32
    }

    /// Get the number of imported globals.
    pub fn num_imported_globals(&self) -> u32 {
        self.imports
            .iter()
            .filter(|i| matches!(i.ty, ImportType::Global(_)))
            .count() as u32
    }
}

// =============================================================================
// Conversion helpers (wasmparser -> our types)
// =============================================================================

fn convert_val_type(ty: wasmparser::ValType) -> Result<ValType> {
    match ty {
        wasmparser::ValType::I32 => Ok(ValType::I32),
        wasmparser::ValType::I64 => Ok(ValType::I64),
        wasmparser::ValType::F32 => Ok(ValType::F32),
        wasmparser::ValType::F64 => Ok(ValType::F64),
        wasmparser::ValType::V128 => Ok(ValType::V128),
        wasmparser::ValType::Ref(rt) => {
            if rt.is_func_ref() {
                Ok(ValType::FuncRef)
            } else if rt.is_extern_ref() {
                Ok(ValType::ExternRef)
            } else {
                bail!("Unsupported reference type: {:?}", rt)
            }
        }
    }
}

fn convert_import_type(ty: &wasmparser::TypeRef) -> Result<ImportType> {
    match ty {
        wasmparser::TypeRef::Func(idx) => Ok(ImportType::Func(*idx)),
        wasmparser::TypeRef::FuncExact(idx) => Ok(ImportType::Func(*idx)),
        wasmparser::TypeRef::Table(tt) => Ok(ImportType::Table(convert_table_type(*tt)?)),
        wasmparser::TypeRef::Memory(mt) => Ok(ImportType::Memory(convert_memory_type(*mt)?)),
        wasmparser::TypeRef::Global(gt) => Ok(ImportType::Global(GlobalType {
            val_type: convert_val_type(gt.content_type)?,
            mutable: gt.mutable,
        })),
        wasmparser::TypeRef::Tag(tt) => Ok(ImportType::Tag(TagType {
            func_type_idx: tt.func_type_idx,
        })),
    }
}

fn convert_table_type(tt: wasmparser::TableType) -> Result<TableType> {
    let element_type = if tt.element_type.is_func_ref() {
        RefType::FuncRef
    } else if tt.element_type.is_extern_ref() {
        RefType::ExternRef
    } else {
        bail!("Unsupported table element type: {:?}", tt.element_type)
    };
    Ok(TableType {
        element_type,
        initial: tt.initial,
        maximum: tt.maximum,
        table64: tt.table64,
    })
}

fn convert_memory_type(mt: wasmparser::MemoryType) -> Result<MemoryType> {
    Ok(MemoryType {
        initial: mt.initial,
        maximum: mt.maximum,
        memory64: mt.memory64,
        shared: mt.shared,
        page_size_log2: mt.page_size_log2,
    })
}

fn convert_export_kind(kind: wasmparser::ExternalKind) -> ExportKind {
    match kind {
        wasmparser::ExternalKind::Func | wasmparser::ExternalKind::FuncExact => ExportKind::Func,
        wasmparser::ExternalKind::Table => ExportKind::Table,
        wasmparser::ExternalKind::Memory => ExportKind::Memory,
        wasmparser::ExternalKind::Global => ExportKind::Global,
        wasmparser::ExternalKind::Tag => ExportKind::Tag,
    }
}

fn convert_element(elem: wasmparser::Element) -> Result<Element> {
    let kind = match elem.kind {
        wasmparser::ElementKind::Passive => ElementKind::Passive,
        wasmparser::ElementKind::Active {
            table_index,
            offset_expr,
        } => ElementKind::Active {
            table_index: table_index.unwrap_or(0),
            offset_expr: extract_const_expr(&offset_expr),
        },
        wasmparser::ElementKind::Declared => ElementKind::Declared,
    };

    let items = match elem.items {
        wasmparser::ElementItems::Functions(reader) => {
            let funcs: Vec<u32> = reader.into_iter().collect::<wasmparser::Result<_>>()?;
            ElementItems::Functions(funcs)
        }
        wasmparser::ElementItems::Expressions(rt, reader) => {
            let ty = if rt.is_func_ref() {
                RefType::FuncRef
            } else {
                RefType::ExternRef
            };
            let exprs: Vec<Vec<u8>> = reader
                .into_iter()
                .map(|e| e.map(|e| extract_const_expr(&e)))
                .collect::<wasmparser::Result<_>>()?;
            ElementItems::Expressions { ty, exprs }
        }
    };

    Ok(Element { kind, items })
}

fn convert_data_segment(data: wasmparser::Data) -> Result<DataSegment> {
    let kind = match data.kind {
        wasmparser::DataKind::Passive => DataKind::Passive,
        wasmparser::DataKind::Active {
            memory_index,
            offset_expr,
        } => DataKind::Active {
            memory_index,
            offset_expr: extract_const_expr(&offset_expr),
        },
    };

    Ok(DataSegment {
        kind,
        data: data.data.to_vec(),
    })
}

fn extract_const_expr(expr: &wasmparser::ConstExpr) -> Vec<u8> {
    expr.get_binary_reader()
        .read_bytes(expr.get_binary_reader().bytes_remaining())
        .map(|b| b.to_vec())
        .unwrap_or_default()
}

// =============================================================================
// Encoding helpers (our types -> wasm_encoder)
// =============================================================================

fn encode_val_type(ty: ValType) -> wasm_encoder::ValType {
    match ty {
        ValType::I32 => wasm_encoder::ValType::I32,
        ValType::I64 => wasm_encoder::ValType::I64,
        ValType::F32 => wasm_encoder::ValType::F32,
        ValType::F64 => wasm_encoder::ValType::F64,
        ValType::V128 => wasm_encoder::ValType::V128,
        ValType::FuncRef => wasm_encoder::ValType::FUNCREF,
        ValType::ExternRef => wasm_encoder::ValType::EXTERNREF,
    }
}

fn encode_import_type(ty: &ImportType) -> wasm_encoder::EntityType {
    match ty {
        ImportType::Func(idx) => wasm_encoder::EntityType::Function(*idx),
        ImportType::Table(tt) => wasm_encoder::EntityType::Table(encode_table_type(tt)),
        ImportType::Memory(mt) => wasm_encoder::EntityType::Memory(encode_memory_type(mt)),
        ImportType::Global(gt) => wasm_encoder::EntityType::Global(wasm_encoder::GlobalType {
            val_type: encode_val_type(gt.val_type),
            mutable: gt.mutable,
            shared: false,
        }),
        ImportType::Tag(tt) => wasm_encoder::EntityType::Tag(wasm_encoder::TagType {
            kind: wasm_encoder::TagKind::Exception,
            func_type_idx: tt.func_type_idx,
        }),
    }
}

fn encode_table_type(tt: &TableType) -> wasm_encoder::TableType {
    wasm_encoder::TableType {
        element_type: encode_ref_type(tt.element_type),
        table64: tt.table64,
        minimum: tt.initial,
        maximum: tt.maximum,
        shared: false,
    }
}

fn encode_memory_type(mt: &MemoryType) -> wasm_encoder::MemoryType {
    wasm_encoder::MemoryType {
        minimum: mt.initial,
        maximum: mt.maximum,
        memory64: mt.memory64,
        shared: mt.shared,
        page_size_log2: mt.page_size_log2,
    }
}

fn encode_ref_type(rt: RefType) -> wasm_encoder::RefType {
    match rt {
        RefType::FuncRef => wasm_encoder::RefType::FUNCREF,
        RefType::ExternRef => wasm_encoder::RefType::EXTERNREF,
    }
}

fn encode_export_kind(kind: ExportKind) -> wasm_encoder::ExportKind {
    match kind {
        ExportKind::Func => wasm_encoder::ExportKind::Func,
        ExportKind::Table => wasm_encoder::ExportKind::Table,
        ExportKind::Memory => wasm_encoder::ExportKind::Memory,
        ExportKind::Global => wasm_encoder::ExportKind::Global,
        ExportKind::Tag => wasm_encoder::ExportKind::Tag,
    }
}

fn encode_element(section: &mut wasm_encoder::ElementSection, elem: &Element) {
    use wasm_encoder::Elements;

    match &elem.kind {
        ElementKind::Passive => match &elem.items {
            ElementItems::Functions(funcs) => {
                section.passive(Elements::Functions(Cow::Borrowed(funcs.as_slice())));
            }
            ElementItems::Expressions { ty, exprs } => {
                let const_exprs: Vec<wasm_encoder::ConstExpr> = exprs
                    .iter()
                    .map(|e| wasm_encoder::ConstExpr::raw(e.iter().copied()))
                    .collect();
                section.passive(Elements::Expressions(
                    encode_ref_type(*ty),
                    Cow::Owned(const_exprs),
                ));
            }
        },
        ElementKind::Active {
            table_index,
            offset_expr,
        } => {
            let offset = wasm_encoder::ConstExpr::raw(offset_expr.iter().copied());
            match &elem.items {
                ElementItems::Functions(funcs) => {
                    section.active(
                        Some(*table_index),
                        &offset,
                        Elements::Functions(Cow::Borrowed(funcs.as_slice())),
                    );
                }
                ElementItems::Expressions { ty, exprs } => {
                    let const_exprs: Vec<wasm_encoder::ConstExpr> = exprs
                        .iter()
                        .map(|e| wasm_encoder::ConstExpr::raw(e.iter().copied()))
                        .collect();
                    section.active(
                        Some(*table_index),
                        &offset,
                        Elements::Expressions(encode_ref_type(*ty), Cow::Owned(const_exprs)),
                    );
                }
            }
        }
        ElementKind::Declared => match &elem.items {
            ElementItems::Functions(funcs) => {
                section.declared(Elements::Functions(Cow::Borrowed(funcs.as_slice())));
            }
            ElementItems::Expressions { ty, exprs } => {
                let const_exprs: Vec<wasm_encoder::ConstExpr> = exprs
                    .iter()
                    .map(|e| wasm_encoder::ConstExpr::raw(e.iter().copied()))
                    .collect();
                section.declared(Elements::Expressions(
                    encode_ref_type(*ty),
                    Cow::Owned(const_exprs),
                ));
            }
        },
    }
}
