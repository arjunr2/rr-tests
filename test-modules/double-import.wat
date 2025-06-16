(module
    (type (;0;) (func (param i32) (result i32)))
    (import "env" "double" (func $double_import (type 0)))
    (func $main (export "main") (param i32) (result i32)
        local.get 0
        call $double_import
    )
)
