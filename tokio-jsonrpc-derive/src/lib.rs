extern crate proc_macro;
use proc_macro::TokenStream;

extern crate syn;

#[macro_use]
extern crate quote;

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
    let fields = match ast.body {
        syn::Body::Struct(ref data) => data.fields(),
        syn::Body::Enum(_) => panic!("#[derive(Params)] can only be used with structs"),
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

    let named_body = parse_named();
    let positional_body = parse_positional(fields);

    quote! {
        // The generated impl
        impl #impl_generics ::tokio_jsonrpc::message::FromParams for #name
            #ty_generics #where_clause
        {
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
                        // Review: should we try to parse an object where all fields are missing?
                        None
                    }
                }
            }
        }
    }
}

fn parse_positional(fields: Vec<String>) -> quote::Tokens {
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
             .map_err(|err| RpcError::parse_error(format!("{}", err))))
     }
}

fn parse_named() -> quote::Tokens {
    quote! {
        let value = ::tokio_jsonrpc::macro_exports::Value::Object(x);
        Some(::tokio_jsonrpc::macro_exports::from_value(value)
             .map_err(|err| ::tokio_jsonrpc::RpcError
                            ::parse_error(format!("{}", err))))
    }
}
