(module
    (type (;0;) (func (param i32) (result i32)))
    (type (;1;) (func (result i32)))
    (import "env" "double" (func $double_import (type 0)))
    (import "env" "rand" (func $rand (type 1)))
    (func $main (export "main") (param i32) (result i32)
        local.get 0
        call $rand
        call $double_import
        i32.add
    )
)
