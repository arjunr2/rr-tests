#![allow(unused)]

#[macro_export]
macro_rules! bin {
    (@uses) => {
        use anyhow::*;
        use std::error::Error;
        use wasmtime::component::{Component, Linker};
        use wasmtime::component::{ComponentNamedList, HasSelf, Instance, Lift, Lower, bindgen};
        use wasmtime::{Engine, Store};
        use crimp_tests::*;
    };

    (@add $linker:ident, $st:ident) => {
        $st::add_to_linker::<_, HasSelf<_>>(&mut $linker, |state| state)
    };
}
