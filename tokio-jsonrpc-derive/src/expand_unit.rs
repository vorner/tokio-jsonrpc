use syn::DeriveInput;

use quote::Tokens;

use null_body;

pub fn expand_unit(ast: &DeriveInput) -> Tokens {
    // Used in the quasi-quotation below as `#name`
    let name = &ast.ident;

    // Helper is provided for handling complex generic types correctly and effortlessly
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();

    let body = body();
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
                    Some(params) => {
                        #body
                    },
                    None => {
                        #null_body
                    }
                }
            }
        }
    }
}

fn body() -> Tokens {
    quote! {
        if ! params.is_empty() {
            Some(Err(::tokio_jsonrpc::RpcError::invalid_params(
                Some("Received params but expected none".to_owned()))))
        } else {
            let value = ::tokio_jsonrpc::macro_exports::Value::Null;
            let x = ::tokio_jsonrpc::macro_exports::from_value(value)
                        .map_err(|err| ::tokio_jsonrpc::RpcError::parse_error(format!("{}", err)));
            Some(x)
        }
     }
}
