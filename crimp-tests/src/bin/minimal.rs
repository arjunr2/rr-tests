crimp_tests::bin!(@uses);

fn main() -> Result<()> {
    component_run::<_, RunTy, (), (u32,)>(
        ComponentFmt::Raw(
            r#"
                (component
                    (core module $m
                        (func (export "main") (result i32)
                            i32.const 42
                        )
                    )
                    (core instance $i (instantiate $m))

                    (func (export "main") (result u32)
                        (canon lift (core func $i "main"))
                    )
                )
            "#,
        ),
        |_| Ok(()),
        RunMode::InstantiateAndCallOnce {
            name: "main",
            params: (),
        },
    )
}
