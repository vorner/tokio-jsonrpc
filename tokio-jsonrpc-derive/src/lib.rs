extern crate proc_macro;
use proc_macro::TokenStream;

extern crate syn;

#[macro_use]
extern crate quote;

mod expand_enum;
mod expand_newtype;
mod expand_struct;
mod expand_tuple;
mod expand_unit;

use expand_enum::expand_enum;
use expand_newtype::expand_newtype;
use expand_struct::expand_struct;
use expand_tuple::expand_tuple;
use expand_unit::expand_unit;

use syn::{Attribute, DeriveInput, Ident, MetaItem, NestedMetaItem};

#[proc_macro_derive(Params)]
pub fn params(input: TokenStream) -> TokenStream {
    let source = input.to_string();

    // Parse the string representation into a syntax tree
    let ast = syn::parse_derive_input(&source).unwrap();

    // Build the output
    let expanded = expand_params(&ast);

    // Return the generated impl as a TokenStream
    expanded.parse().unwrap()
}

fn expand_params(ast: &syn::DeriveInput) -> quote::Tokens {
    match ast.body {
        syn::Body::Struct(syn::VariantData::Struct(_)) => expand_struct(ast),
        syn::Body::Struct(syn::VariantData::Tuple(ref fields)) if fields.len() == 1 => {
            expand_newtype(ast)
        },
        syn::Body::Struct(syn::VariantData::Tuple(ref fields)) if fields.len() > 1 => {
            expand_tuple(ast)
        },
        syn::Body::Struct(syn::VariantData::Unit) => expand_unit(ast),
        syn::Body::Enum(_) => expand_enum(ast),
        _ => unreachable!(),
    }
}

fn null_body(ast: &DeriveInput) -> quote::Tokens {
    fn is_default(attr: &Attribute) -> bool {
        if attr.name() != "serde" {
            return false;
        }

        match attr.value {
            MetaItem::List(_, ref xs) if xs.len() > 0 => {
                xs[0] == NestedMetaItem::MetaItem(MetaItem::Word(Ident::new("default")))
            },
            _ => false,
        }
    }
    let is_default = ast.attrs
        .iter()
        .any(is_default);

    if is_default {
        let name = &ast.ident;

        quote! {
            #[derive(Debug, Deserialize)]
            #[allow(non_camel_case_types)]
            struct __jsonrpc_params_derive_X {
                #[serde(default)]
                x: #name,
            }

            // Try to parse it as if we received no arguments, if we fail DO NOT error
            let value = ::tokio_jsonrpc::macro_exports::Value::Object(
                ::tokio_jsonrpc::macro_exports::Map::new()
            );
            match ::tokio_jsonrpc::macro_exports::from_value::<__jsonrpc_params_derive_X>(value) {
                Ok(x) => Some(Ok(x.x)),
                Err(_) => None,
            }
        }
    } else {
        quote! {
            // Try to parse it as if we received no arguments, if we fail DO NOT error
            let value = ::tokio_jsonrpc::macro_exports::Value::Null;
            match ::tokio_jsonrpc::macro_exports::from_value(value) {
                Ok(x) => Some(Ok(x)),
                Err(_) => None,
            }
        }
    }
}
