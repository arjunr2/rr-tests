#[allow(warnings)]
mod bindings;

use bindings::Guest;

use bindings::component::test_package::env::factors;

struct Component;

impl Guest for Component {
    /// Say hello!
    fn main(x: u32) -> u32 {
        let y = factors(x);
        y.iter().sum()
    }
}

bindings::export!(Component with_types_in bindings);
