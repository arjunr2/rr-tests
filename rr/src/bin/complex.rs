impl component::test_package::env::Host for () {
    fn double(&mut self, x: u32) -> u32 {
        x * 2
    }

    fn complex(&mut self, x: u32, y: u64) -> u64 {
        (x * x) as u64 + y
    }
}

wasmtime_rr_tests::bin! {
    complex,
    "root" in "../test-modules/components/wit/complex-singlereturn-indirect.wit",
    "test-modules/components/complex-singlereturn-indirect.wat",
    Root,
    main,
    (u32,), (u32,), (42,)
}
