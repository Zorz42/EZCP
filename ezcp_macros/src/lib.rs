//! Procedural macros for [EZCP](https://docs.rs/ezcp), which re-exports them.
#![warn(missing_docs)]
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, parse_macro_input};

/// Derives `ToOutput` for a struct: its fields in order, one per line, skipping
/// fields that render empty.
#[proc_macro_derive(ToOutput)]
pub fn to_output_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let field_output = |accessor: proc_macro2::TokenStream| {
        quote! {
            {
                let field = ::ezcp::ToOutput::to_output(#accessor);
                if !field.is_empty() {
                    res.push_str(&field);
                    if !res.ends_with('\n') {
                        res.push('\n');
                    }
                }
            }
        }
    };

    let fields_output = match input.data {
        Data::Struct(data_struct) => match data_struct.fields {
            syn::Fields::Named(fields_named) => {
                let field_calls = fields_named.named.into_iter().filter_map(|field| {
                    // Always `Some` for named fields.
                    let field_name = field.ident?;
                    Some(field_output(quote! { self.#field_name }))
                });
                quote! { #(#field_calls)* }
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                let field_calls = (0..fields_unnamed.unnamed.len()).map(|idx| {
                    let index = syn::Index::from(idx);
                    field_output(quote! { self.#index })
                });
                quote! { #(#field_calls)* }
            }
            syn::Fields::Unit => quote! {},
        },
        _ => {
            return syn::Error::new(name.span(), "ToOutput can only be derived for structs").to_compile_error().into();
        }
    };

    // Full paths, so `#[derive(ezcp::ToOutput)]` works without importing the trait.
    let expanded = quote! {
        #[automatically_derived]
        impl #impl_generics ::ezcp::ToOutput for #name #ty_generics #where_clause {
            fn to_output(self) -> ::std::string::String {
                let mut res = ::std::string::String::new();
                #fields_output
                res
            }
        }
    };

    TokenStream::from(expanded)
}
