use wasmtime::component::bindgen;


bindgen!("root" in "test-modules/components/wit/complex-import-singlereturn.wit");

impl component::test_package::env::Host for () {
    fn double(&mut self, x: u32) -> u32 {
        x * 2
    }

    fn complex(&mut self, x: u32, y: u64) -> u64 {
        (x * x) as u64 + y
    }
}
