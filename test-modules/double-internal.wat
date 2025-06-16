(module
    (func $double_internal (param i32) (result i32)
        local.get 0
        i32.const 2
        i32.mul
    )
    (func $main (export "main") (param i32) (result i32)
        i32.const 42
        call $double_internal
    )
)
