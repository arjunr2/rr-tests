use prime_factorization::Factorization;

impl component::test_package::env::Host for () {
    fn factors(&mut self, x: u32) -> Vec<u32> {
        Factorization::run(x).factors
    }
}

wasmtime_rr_tests::bin!(@uses);

bindgen!(
    "my-world" in "../test-modules/components/wit/multi_return.wit"
);

fn main() -> Result<()> {
    component_run::<_, RunTy, (u32,), (u32,)>(
        ComponentFmt::File("test-modules/components/multi_return.wat"),
        |mut linker| wasmtime_rr_tests::bin!(@add linker, MyWorld),
        RunMode::InstantiateAndCallOnce {
            name: "main",
            params: (42,),
        },
    )
}
