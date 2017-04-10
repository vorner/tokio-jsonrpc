use syn::DeriveInput;

use quote::Tokens;

pub fn expand_tuple(_ast: &DeriveInput) -> Tokens {
    panic!("Tuples are not supported yet");
}
