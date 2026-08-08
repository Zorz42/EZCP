use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, parse_macro_input};

#[proc_macro_derive(ToOutput)]
pub fn to_output_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Each field goes on its own line. A field that produced nothing is skipped
    // entirely, so an empty collection does not turn into a blank line.
    let field_output = |accessor: proc_macro2::TokenStream| {
        quote! {
            {
                let field = #accessor.to_output();
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
                let field_calls = fields_named.named.into_iter().map(|field| {
                    let field_name = field.ident.expect("named fields always have an identifier");
                    field_output(quote! { self.#field_name })
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
        // Report through the compiler rather than panicking, so the user gets an
        // error pointing at their type instead of a proc macro backtrace.
        _ => {
            return syn::Error::new(name.span(), "ToOutput can only be derived for structs").to_compile_error().into();
        }
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
