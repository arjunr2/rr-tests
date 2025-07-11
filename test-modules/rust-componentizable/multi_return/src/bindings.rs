// Generated by `wit-bindgen` 0.41.0. DO NOT EDIT!
// Options used:
//   * runtime_path: "wit_bindgen_rt"
#[doc(hidden)]
#[allow(non_snake_case)]
pub unsafe fn _export_main_cabi<T: Guest>(arg0: i32) -> i32 {
    #[cfg(target_arch = "wasm32")] _rt::run_ctors_once();
    let result0 = T::main(arg0 as u32);
    _rt::as_i32(result0)
}
pub trait Guest {
    fn main(x: u32) -> u32;
}
#[doc(hidden)]
macro_rules! __export_world_my_world_cabi {
    ($ty:ident with_types_in $($path_to_types:tt)*) => {
        const _ : () = { #[unsafe (export_name = "main")] unsafe extern "C" fn
        export_main(arg0 : i32,) -> i32 { unsafe { $($path_to_types)*::
        _export_main_cabi::<$ty > (arg0) } } };
    };
}
#[doc(hidden)]
pub(crate) use __export_world_my_world_cabi;
#[rustfmt::skip]
#[allow(dead_code, clippy::all)]
pub mod component {
    pub mod test_package {
        #[allow(dead_code, async_fn_in_trait, unused_imports, clippy::all)]
        pub mod env {
            #[used]
            #[doc(hidden)]
            static __FORCE_SECTION_REF: fn() = super::super::super::__link_custom_section_describing_imports;
            use super::super::super::_rt;
            #[allow(unused_unsafe, clippy::all)]
            pub fn factors(x: u32) -> _rt::Vec<u32> {
                unsafe {
                    #[cfg_attr(target_pointer_width = "64", repr(align(8)))]
                    #[cfg_attr(target_pointer_width = "32", repr(align(4)))]
                    struct RetArea(
                        [::core::mem::MaybeUninit<
                            u8,
                        >; 2 * ::core::mem::size_of::<*const u8>()],
                    );
                    let mut ret_area = RetArea(
                        [::core::mem::MaybeUninit::uninit(); 2
                            * ::core::mem::size_of::<*const u8>()],
                    );
                    let ptr0 = ret_area.0.as_mut_ptr().cast::<u8>();
                    #[cfg(target_arch = "wasm32")]
                    #[link(wasm_import_module = "component:test-package/env")]
                    unsafe extern "C" {
                        #[link_name = "factors"]
                        fn wit_import1(_: i32, _: *mut u8);
                    }
                    #[cfg(not(target_arch = "wasm32"))]
                    unsafe extern "C" fn wit_import1(_: i32, _: *mut u8) {
                        unreachable!()
                    }
                    unsafe { wit_import1(_rt::as_i32(&x), ptr0) };
                    let l2 = *ptr0.add(0).cast::<*mut u8>();
                    let l3 = *ptr0
                        .add(::core::mem::size_of::<*const u8>())
                        .cast::<usize>();
                    let len4 = l3;
                    let result5 = _rt::Vec::from_raw_parts(l2.cast(), len4, len4);
                    result5
                }
            }
        }
    }
}
#[rustfmt::skip]
mod _rt {
    #![allow(dead_code, clippy::all)]
    pub use alloc_crate::vec::Vec;
    pub fn as_i32<T: AsI32>(t: T) -> i32 {
        t.as_i32()
    }
    pub trait AsI32 {
        fn as_i32(self) -> i32;
    }
    impl<'a, T: Copy + AsI32> AsI32 for &'a T {
        fn as_i32(self) -> i32 {
            (*self).as_i32()
        }
    }
    impl AsI32 for i32 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for u32 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for i16 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for u16 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for i8 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for u8 {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for char {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    impl AsI32 for usize {
        #[inline]
        fn as_i32(self) -> i32 {
            self as i32
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub fn run_ctors_once() {
        wit_bindgen_rt::run_ctors_once();
    }
    extern crate alloc as alloc_crate;
}
/// Generates `#[unsafe(no_mangle)]` functions to export the specified type as
/// the root implementation of all generated traits.
///
/// For more information see the documentation of `wit_bindgen::generate!`.
///
/// ```rust
/// # macro_rules! export{ ($($t:tt)*) => (); }
/// # trait Guest {}
/// struct MyType;
///
/// impl Guest for MyType {
///     // ...
/// }
///
/// export!(MyType);
/// ```
#[allow(unused_macros)]
#[doc(hidden)]
macro_rules! __export_my_world_impl {
    ($ty:ident) => {
        self::export!($ty with_types_in self);
    };
    ($ty:ident with_types_in $($path_to_types_root:tt)*) => {
        $($path_to_types_root)*:: __export_world_my_world_cabi!($ty with_types_in
        $($path_to_types_root)*);
    };
}
#[doc(inline)]
pub(crate) use __export_my_world_impl as export;
#[cfg(target_arch = "wasm32")]
#[unsafe(
    link_section = "component-type:wit-bindgen:0.41.0:component:test-package:my-world:encoded world"
)]
#[doc(hidden)]
#[allow(clippy::octal_escapes)]
pub static __WIT_BINDGEN_COMPONENT_TYPE: [u8; 242] = *b"\
\0asm\x0d\0\x01\0\0\x19\x16wit-component-encoding\x04\0\x07t\x01A\x02\x01A\x04\x01\
B\x03\x01py\x01@\x01\x01xy\0\0\x04\0\x07factors\x01\x01\x03\0\x1acomponent:test-\
package/env\x05\0\x01@\x01\x01xy\0y\x04\0\x04main\x01\x01\x04\0\x1fcomponent:tes\
t-package/my-world\x04\0\x0b\x0e\x01\0\x08my-world\x03\0\0\0G\x09producers\x01\x0c\
processed-by\x02\x0dwit-component\x070.227.1\x10wit-bindgen-rust\x060.41.0";
#[inline(never)]
#[doc(hidden)]
pub fn __link_custom_section_describing_imports() {
    wit_bindgen_rt::maybe_link_cabi_realloc();
}
