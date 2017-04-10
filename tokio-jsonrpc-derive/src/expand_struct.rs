use syn::{Body, DeriveInput, VariantData};

use quote::Tokens;

use null_body;

pub fn expand_struct(ast: &DeriveInput) -> Tokens {
    let fields = match ast.body {
        Body::Struct(VariantData::Struct(ref fields)) => fields,
        _ => unreachable!(),
    };

    let fields: Vec<String> = fields.iter()
        .filter(|x| x.ident.is_some())
        .map(|x| {
            format!("{}",
                    x.ident
                        .as_ref()
                        .unwrap())
        })
        .collect();

    // Used in the quasi-quotation below as `#name`
    let name = &ast.ident;

    // Helper is provided for handling complex generic types correctly and effortlessly
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let named_body = expand_struct_named();
    let positional_body = expand_struct_positional(fields);
    let null_body = null_body(ast);

    quote! {
        // The generated impl
        impl #impl_generics ::tokio_jsonrpc::server::FromParams for #name
            #ty_generics #where_clause
        {
            #[allow(unused_mut)]
            fn from_params(params: Option<::tokio_jsonrpc::Params>)
                -> Option<Result<#name, ::tokio_jsonrpc::RpcError>>
            {
                match params {
                    Some(::tokio_jsonrpc::Params::Positional(mut xs)) => {
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

fn expand_struct_positional(fields: Vec<String>) -> Tokens {
    quote! {
        // Turn the positions into a partial object
        // We can leverage any #[serde(default)] by letting serde do the work
        let mut object = ::tokio_jsonrpc::macro_exports::Map::new();

        #(
            if ! xs.is_empty() {
                 object.insert(#fields.to_owned(), xs.remove(0));
            }
        )*

        let value = ::tokio_jsonrpc::macro_exports::Value::Object(object);

        Some(::tokio_jsonrpc::macro_exports::from_value(value)
             .map_err(|err| ::tokio_jsonrpc::RpcError
                            ::parse_error(format!("{}", err))))
     }
}

fn expand_struct_named() -> Tokens {
    quote! {
        let value = ::tokio_jsonrpc::macro_exports::Value::Object(x);
        Some(::tokio_jsonrpc::macro_exports::from_value(value)
             .map_err(|err| ::tokio_jsonrpc::RpcError
                            ::parse_error(format!("{}", err))))
    }
}
