use syn::DeriveInput;

use quote::Tokens;

use null_body;

pub fn expand_newtype(ast: &DeriveInput) -> Tokens {
    // Used in the quasi-quotation below as `#name`
    let name = &ast.ident;

    // Helper is provided for handling complex generic types correctly and effortlessly
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let named_body = named_body();
    let positional_body = positional_body();
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

fn positional_body() -> Tokens {
    quote! {
        let value = xs.remove(0);

        Some(::tokio_jsonrpc::macro_exports::from_value(value)
             .map_err(|err| ::tokio_jsonrpc::RpcError
                            ::parse_error(format!("{}", err))))
     }
}

fn named_body() -> Tokens {
    quote! {
        // Take the first value or null if not present
        let value = ::std::iter::IntoIterator::into_iter(x)
                        .next()
                        .map(|(_,v)| v)
                        .unwrap_or(::tokio_jsonrpc::macro_exports::Value::Null);
        Some(::tokio_jsonrpc::macro_exports::from_value(value)
             .map_err(|err| ::tokio_jsonrpc::RpcError
                            ::parse_error(format!("{}", err))))
    }
}
