(module
    (type $double_type (func (param i32) (result i32)))
    (type (;1;) (func (result i32)))
    (type $complex_type (func (param i32 i64) (result i64)))
    (import "component:test-package/env" "double" (func $double_import (type $double_type)))
    (import "component:test-package/env" "complex" (func $complex_import (type $complex_type)))
    (func $main (export "main") (param i32) (result i32)
        local.get 0
        call $double_import
        i64.const 314
        i32.const 2
        call_indirect (type $complex_type)
        i32.wrap_i64
    )
    (table $function_table 3 3 funcref)
    (elem (i32.const 1) $double_import $complex_import)
)
