(module
    (type (;0;) (func (param i32) (result i32)))
    (type (;1;) (func (result i32)))
    (type (;2;) (func (param i32 i64) (result i32 i64 f32)))
    (import "env" "double" (func $double_import (type 0)))
    (import "env" "complex" (func $complex_import (type 2)))
    (func $main (export "main") (param i32) (result i32)
        local.get 0
        call $double_import
        i64.const 314
        call $complex_import
        drop
        i32.wrap_i64
        i32.add
    )
)
