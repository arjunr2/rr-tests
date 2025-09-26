use prime_factorization::Factorization;

impl component::test_package::env::Host for () {
    fn factors(&mut self, x: u32) -> Vec<u32> {
        Factorization::run(x).factors
    }
}

wasmtime_rr_tests::bin! {
    complex_params,
    "my-world" in "../test-modules/components/wit/complex_params.wit",
    "test-modules/components/complex_params.wat",
    MyWorld,
    main,
    (Vec<u32>,), (Vec<u32>,), ((0..10000).collect::<Vec<u32>>(),)
}
