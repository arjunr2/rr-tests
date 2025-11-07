#[allow(warnings)]
mod bindings;

use bindings::component::test_package::env::{process_data, DataPayload, PaymentMethod};
use bindings::Guest;

struct Component;

impl Guest for Component {
    fn main(data: DataPayload) -> String {
        // Call the imported process-data function
        let processed = process_data(&data);

        // Create an encoded representation based on the variant type
        let encoding = match processed {
            DataPayload::Order(order) => {
                format!(
                    "ORDER|id:{}|amount:{:.2}|items:[{}]|ship_addr:{}|{}|{}|{}|{}",
                    order.order_id,
                    order.total_amount,
                    order.items.join(","),
                    order.shipping_address.street,
                    order.shipping_address.city,
                    order.shipping_address.state,
                    order.shipping_address.country,
                    order.shipping_address.postal_code,
                )
            }
            DataPayload::AddressUpdate(addr) => {
                format!(
                    "ADDRESS|street:{}|city:{}|state:{}|country:{}|postal:{}",
                    addr.street, addr.city, addr.state, addr.country, addr.postal_code,
                )
            }
            DataPayload::SimpleMessage(msg) => {
                format!("MESSAGE|{}", msg)
            }
            DataPayload::NumericData(nums) => {
                format!(
                    "NUMERIC|count:{}|values:[{}]",
                    nums.len(),
                    nums.iter()
                        .map(|n| n.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            }
            DataPayload::Payment(payment) => match payment {
                PaymentMethod::CreditCard(card) => format!("PAYMENT|credit-card:{}", card),
                PaymentMethod::Paypal(email) => format!("PAYMENT|paypal:{}", email),
                PaymentMethod::BankTransfer => format!("PAYMENT|bank-transfer"),
                PaymentMethod::Cash => format!("PAYMENT|cash"),
            },
        };

        encoding
    }
}

bindings::export!(Component with_types_in bindings);
