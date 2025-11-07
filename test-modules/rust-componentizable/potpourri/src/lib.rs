#[allow(warnings)]
mod bindings;

use bindings::component::test_package::env::{process_pot, PaymentMethod, PotPayload, PotRecord};
use bindings::Guest;

struct Component;

impl Guest for Component {
    fn main(p: PotRecord) -> String {
        // Call the imported process-pot function
        let processed = process_pot(&p);

        // Encode the pot-record into a compact string
        let tags = match processed.opt_tags {
            Some(ref v) => v.join(","),
            None => "".to_string(),
        };

        let payload_str = match processed.payload {
            PotPayload::Numbers(nums) => format!(
                "NUMS:[{}]",
                nums.iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            PotPayload::Small(s) => format!("SMALL:count={} ratio={:.3}", s.count, s.ratio),
            PotPayload::Payment(pm) => match pm {
                PaymentMethod::CreditCard(c) => format!("PAY:cc:{}", c),
                PaymentMethod::Paypal(e) => format!("PAY:pp:{}", e),
                PaymentMethod::Cash => "PAY:cash".to_string(),
            },
            PotPayload::Text(t) => format!("TEXT:{}", t),
        };

        let validation_str = match processed.validation {
            Ok(msg) => format!("OK:{}", msg),
            Err(err) => format!("ERR:{}", err),
        };

        format!(
            "POT|id:{}|name:{}|active:{}|score:{:.2}|tags:{}|{}|valid:{}",
            processed.id,
            processed.name,
            processed.active,
            processed.score,
            tags,
            payload_str,
            validation_str
        )
    }
}

bindings::export!(Component with_types_in bindings);
