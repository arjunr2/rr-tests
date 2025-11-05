//#![no_std]
//extern crate alloc;
//use alloc::format;
//use alloc::string::{String, ToString};
//use alloc::vec::Vec;

#[allow(warnings)]
mod bindings;

use bindings::Guest;

use bindings::component::test_package::env::{process_profile, UserProfile};

//// Use wee_alloc as the global allocator
//#[global_allocator]
//static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;
//
//// Panic handler
//#[panic_handler]
//fn panic(_info: &core::panic::PanicInfo) -> ! {
//    loop {}
//}

struct Component;

impl Guest for Component {
    fn main(profile: UserProfile) -> String {
        // Call the imported process-profile function
        let processed = process_profile(&profile);

        // Create an encoded representation of the profile
        // Using a simple string encoding format
        let encoding = format!(
            "PROFILE_V1|{}|{}|{}|{}|{}|verified:{}|balance:{:.2}|points:{}|tier:{}|tags:[{}]|purchases:[{}]|prefs:[{}]",
            processed.id,
            processed.username,
            processed.email,
            processed.first_name,
            processed.last_name,
            processed.is_verified,
            processed.account_balance,
            processed.loyalty_points,
            processed.subscription_tier,
            processed.tags.join(","),
            processed.purchase_history.iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(","),
            processed.preferences.join(",")
        );

        encoding
    }
}

bindings::export!(Component with_types_in bindings);
