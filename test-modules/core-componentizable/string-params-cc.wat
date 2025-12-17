(module
    (type (;0;) (func (param i32 i32 i32)))
    (type (;1;) (func (result i32)))
    (type (;2;) (func (param i32 i64) (result i64)))
    (import "component:test-package/env" "reverse-string" (func $reverse_import (type 0)))
    (global $bump (mut i32) (i32.const 4096))
    (func $main (export "main") (param i32 i32) (result i32)
        (local $str_ptr i32)
        i32.const 100
        local.set $str_ptr
        local.get 0
        local.get 1
        local.get $str_ptr
        call $reverse_import
        local.get $str_ptr
    )
    (func $realloc (param $old_ptr i32) (param $old_size i32) (param $align i32) (param $new_size i32) (result i32)
      (local $result i32)
      global.get $bump
      local.get $align
      i32.const 1
      i32.sub
      i32.add
      local.get $align
      i32.const 1
      i32.sub
      i32.const -1
      i32.xor
      i32.and
      local.set $result
      local.get $result
      local.get $new_size
      i32.add
      global.set $bump
      local.get $result
    )
    (export "cabi_realloc" (func $realloc))
    (memory (export "memory") 1)
)
