use prime_factorization::Factorization;

impl component::test_package::env::Host for () {
    fn factors(&mut self, x: u32) -> Vec<u32> {
        Factorization::run(x).factors
    }
}

wasmtime_rr_tests::bin! {
    multi_return,
    "my-world" in "../test-modules/components/wit/multi_return.wit",
    "test-modules/components/multi_return.wat",
    MyWorld
}
