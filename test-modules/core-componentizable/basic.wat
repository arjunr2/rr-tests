(module
    (type (;0;) (func (param i32) (result i32)))
    (type (;1;) (func (result i32)))
    (type (;2;) (func (param i32 i64) (result i32 i64 f32)))
    (func $main (export "main") (param i32) (result i32)
        local.get 0
        i64.const 314
        i32.wrap_i64
        i32.mul
        i32.const 4
        i32.add
    )
)
