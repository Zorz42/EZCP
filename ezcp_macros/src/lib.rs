use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, parse_macro_input};

#[proc_macro_derive(ToOutput)]
pub fn to_output_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let fields_output = match input.data {
        Data::Struct(data_struct) => match data_struct.fields {
            syn::Fields::Named(fields_named) => {
                let mut field_calls = Vec::new();
                for field in fields_named.named {
                    let field_name = field.ident.unwrap();
                    field_calls.push(quote! {
                        res.push_str(&self.#field_name.to_output());
                        if res.chars().last() != Some('\n') {
                            res.push('\n');
                        }
                    });
                }
                quote! { #(#field_calls)* }
            }
            syn::Fields::Unnamed(fields_unnamed) => {
                let mut field_calls = Vec::new();
                for (idx, _) in fields_unnamed.unnamed.into_iter().enumerate() {
                    let index = syn::Index::from(idx);
                    field_calls.push(quote! {
                        res.push_str(&self.#index.to_output());
                        if res.chars().last() != Some('\n') {
                            res.push('\n');
                        }
                    });
                }
                quote! { #(#field_calls)* }
            }
            syn::Fields::Unit => {
                quote! {}
            }
        },
        _ => panic!("ToOutput can only be derived for structs"),
    };

    let expanded = quote! {
        #[automatically_derived]
        impl #impl_generics ToOutput for #name #ty_generics #where_clause {
            fn to_output(self) -> String {
                let mut res = String::new();
                #fields_output
                res
            }
        }
    };

    TokenStream::from(expanded)
}
