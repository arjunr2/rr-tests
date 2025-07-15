#[allow(warnings)]
mod bindings;

use bindings::Guest;

use bindings::component::test_package::env::factors;

struct Component;

impl Guest for Component {
    /// Say hello!
    fn main(x: u32) -> u32 {
        (0..10000).map(|x| {
            let y = factors(x);
            y.iter().sum::<u32>()
        }).sum::<u32>() + x
    }
}

bindings::export!(Component with_types_in bindings);
