use syn::{Attribute, Body, ConstExpr, DeriveInput, Field, Ident, Variant, VariantData};

use quote::Tokens;

use null_body;

pub fn expand_enum(ast: &DeriveInput) -> Tokens {
    let variants = match ast.body {
        Body::Enum(ref variants) => variants,
        _ => unreachable!(),
    };

    // Used in the quasi-quotation below as `#name`
    let name = &ast.ident;

    // Helper is provided for handling complex generic types correctly and effortlessly
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let named_body = expand_enum_named();
    let positional_body = expand_enum_positional(&ast.ident, &ast.attrs, variants);
    let null_body = null_body(ast);

    quote! {
        // The generated impl
        impl #impl_generics ::tokio_jsonrpc::server::FromParams for #name
            #ty_generics #where_clause
        {
            fn from_params(params: Option<::tokio_jsonrpc::Params>)
                -> Option<Result<#name, ::tokio_jsonrpc::RpcError>>
            {
                match params {
                    Some(::tokio_jsonrpc::Params::Positional(xs)) => {
                        #positional_body
                    },
                    Some(::tokio_jsonrpc::Params::Named(x)) => {
                        #named_body
                    },
                    None => {
                        #null_body
                    }
                }
            }
        }
    }
}

// TODO: add support for tagged enunms
//
// Externally tagged enums look like:
//   {"CaseA": {"x": 1, "y": 2}}
//
// Internally tagged enums look like:
//   {"type": "CaseA", "x": 1, "y": 2}
//
// We could parse this ["CaseA", 1, 2] correctly.
// The convention is that the tag is at position 0
fn expand_enum_variant_struct(attrs: &Vec<Attribute>, fields: &Vec<Field>) -> Tokens {

    use syn::{Ident, MetaItem, NestedMetaItem};

    fn is_untagged(attr: &Attribute) -> bool {
        if attr.name() != "serde" {
            return false;
        }

        match attr.value {
            MetaItem::List(_, ref xs) if xs.len() > 0 => {
                xs[0] == NestedMetaItem::MetaItem(MetaItem::Word(Ident::new("untagged")))
            },
            _ => false,
        }
    }
    let is_untagged = attrs.iter().any(is_untagged);
    if !is_untagged {
        panic!("Only untagged enum are supported.\nConsider adding #[serde(untagged)]");
    }

    let fields: Vec<String> = fields.iter()
        .filter(|x| x.ident.is_some())
        .map(|x| {
            format!("{}",
                    x.ident
                        .as_ref()
                        .unwrap())
        })
        .collect();

    // We hit recursion limit if we try to put this inline
    let parse_error_body = quote! {
        ::tokio_jsonrpc::RpcError::parse_error(format!("{}", err))
    };

    quote!{
        // Turn the positions into a partial object
        // We can leverage any #[serde(default)] by letting serde do the work
        let mut object = ::tokio_jsonrpc::macro_exports::Map::new();

        // Review: do we have a way around this?
        let mut xs_cloned = xs.clone();

        #(
            if ! xs_cloned.is_empty() {
                 object.insert(#fields.to_owned(), xs_cloned.remove(0));
            }
        )*

        let value = ::tokio_jsonrpc::macro_exports::Value::Object(object);

        Some(::tokio_jsonrpc::macro_exports::from_value(value)
                                           .map_err(|err| #parse_error_body))
    }
}

// Review: can we add support for this?
fn expand_enum_variant_tuple(_fields: &Vec<Field>) -> Tokens {
    panic!("Tuple variants are not allowed");
}

fn expand_enum_variant_unit(enum_ident: &Ident, variant_ident: &Ident,
                            discriminant: &Option<ConstExpr>)
                            -> Tokens {

    // The discrimant body is expected to return Result<T, RpcError>
    let discriminant_body = if let &Some(ref discriminant) = discriminant {
        quote! {
            let value = xs[0].clone();
            let x = #discriminant;

            ::tokio_jsonrpc::macro_exports::from_value(value)
                .map_err(|err| ::tokio_jsonrpc::RpcError
                               ::parse_error(format!("{}", err)))
                .and_then(|y: isize|
                    if x == y {
                        Ok(#enum_ident::#variant_ident)
                    } else {
                        // forward the previous error
                        Err(err)
                    })
        }
    } else {
        quote! {
            // forward the previous error
            Err(err)
        }
    };

    quote!{
        // Parsing units only works if the array has one entry
        if xs.len() == 1 {
            // Note: we can't take ownership because multiple variants tries will need it
            let value = xs[0].clone();

            // Try to parse the variant with serde first,
            // if serde fails, try to see if it matches the discriminant
            Some(::tokio_jsonrpc::macro_exports::from_value(value)
                 .map_err(|err| ::tokio_jsonrpc::RpcError
                                ::parse_error(format!("{}", err)))
                 .or_else(|err| { #discriminant_body }))
        } else {
            None
        }
    }
}

fn expand_enum_positional(enum_ident: &Ident, attrs: &Vec<Attribute>, variants: &Vec<Variant>)
                          -> Tokens {
    let variants = variants.iter().map(|variant| match &variant.data {
                                           &VariantData::Struct(ref fields) => {
                                               expand_enum_variant_struct(attrs, fields)
                                           },
                                           &VariantData::Tuple(ref fields) => {
                                               expand_enum_variant_tuple(fields)
                                           },
                                           &VariantData::Unit => {
                                               expand_enum_variant_unit(enum_ident,
                                                                        &variant.ident,
                                                                        &variant.discriminant)
                                           },
                                       });
    quote! {
        #(
            let params = { #variants };
            // Return the first variant that deserializes succesfully
            if let Some(Ok(params)) = params {
                return Some(Ok(params));
            }
        )*

        // No variant matched, we can error
        let array = ::tokio_jsonrpc::macro_exports::Value::Array(xs);
        Some(Err(::tokio_jsonrpc::RpcError
                 ::parse_error(format!("unknown variant `{}`", array))))
    }
}

fn expand_enum_named() -> Tokens {
    quote! {
        let value = ::tokio_jsonrpc::macro_exports::Value::Object(x);
        Some(::tokio_jsonrpc::macro_exports::from_value(value)
             .map_err(|err| ::tokio_jsonrpc::RpcError
                            ::parse_error(format!("{}", err))))
    }
}
