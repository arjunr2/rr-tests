#[allow(warnings)]
mod bindings;

use bindings::Guest;

use bindings::component::test_package::env::factors;

struct Component;

impl Guest for Component {
    /// Say hello!
    fn main(x: Vec<u32>) -> Vec<u32> {
        x.into_iter().map(|val| {
            let y = factors(val);
            y.iter().sum::<u32>()
        }).collect::<Vec<u32>>()
    }
}

bindings::export!(Component with_types_in bindings);
