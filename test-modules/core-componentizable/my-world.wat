(module
    (type (;0;) (func (param i32) (result i32)))
    (type (;1;) (func (result i32)))
    (import "component:test-package/env" "rand" (func $rand_import (type 1)))
    (func $main (export "main") (param i32) (result i32)
        local.get 0
        call $rand_import
        i32.add
    )
)
