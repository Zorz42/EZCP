//! Procedural macros for [EZCP](https://docs.rs/ezcp), which re-exports them.
#![warn(missing_docs)]
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Index, parse_macro_input};

/// Derives `ToOutput` for a struct: its fields in order, one per line, skipping
/// fields that render empty.
#[proc_macro_derive(ToOutput)]
pub fn to_output_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return syn::Error::new(name.span(), "ToOutput can only be derived for structs").to_compile_error().into();
    };
    let fields = data.fields.iter().enumerate().map(|(index, field)| {
        let index = Index::from(index);
        field.ident.as_ref().map_or_else(|| quote!(self.#index), |ident| quote!(self.#ident))
    });
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Full paths, so `#[derive(ezcp::ToOutput)]` works without importing the trait.
    quote! {
        #[automatically_derived]
        impl #impl_generics ::ezcp::ToOutput for #name #ty_generics #where_clause {
            fn to_output(self) -> ::std::string::String {
                let mut res = ::std::string::String::new();
                #({
                    let field = ::ezcp::ToOutput::to_output(#fields);
                    if !field.is_empty() {
                        res.push_str(&field);
                        if !res.ends_with('\n') {
                            res.push('\n');
                        }
                    }
                })*
                res
            }
        }
    }
    .into()
}
