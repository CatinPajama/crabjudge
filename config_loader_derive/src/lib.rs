use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(ConfigType)]
pub fn derive_config_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;

    let lower_name = name.to_string().to_lowercase();

    let expanded = quote! {
        impl ConfigType for #name {
            fn get_config_name() -> String {
                #lower_name.to_string()
            }
        }
    };

    expanded.into()
}
