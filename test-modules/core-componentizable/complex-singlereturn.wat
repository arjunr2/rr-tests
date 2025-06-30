(module
    (type (;0;) (func (param i32) (result i32)))
    (type (;1;) (func (result i32)))
    (type (;2;) (func (param i32 i64) (result i64)))
    (import "component:test-package/env" "double" (func $double_import (type 0)))
    (import "component:test-package/env" "complex" (func $complex_import (type 2)))
    (func $main (export "main") (param i32) (result i32)
        local.get 0
        call $double_import
        i64.const 314
        call $complex_import
        i32.wrap_i64
    )
)
