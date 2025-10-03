wasmtime_rr_tests::bin!(@uses);

fn main() -> Result<()> {
    component_run::<_, RunTy, (), ()>(
        ComponentFmt::Raw(
            r#"
                (component
                    (type $r (resource (rep i32)))
                    (core func $drop (canon resource.drop $r))

                    (core module $m
                        (import "" "drop" (func $drop (param i32)))

                        (func (export "main")
                        (call $drop (i32.const 0))
                        )
                    )
                    (core instance $i (instantiate $m
                        (with "" (instance
                        (export "drop" (func $drop))
                        ))
                    ))

                    (func (export "main") (canon lift (core func $i "main")))
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
