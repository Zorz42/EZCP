//! The `ToOutput` derive of [EZCP](https://docs.rs/ezcp), which re-exports it.
use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Index, parse_macro_input};

/// Writes the fields in order, one per line, skipping those that write nothing.
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

    // Full paths, so the derive works without the trait in scope.
    quote! {
        #[automatically_derived]
        impl #impl_generics ::ezcp::ToOutput for #name #ty_generics #where_clause {
            #[allow(unused_variables)]
            fn write_output(self, out: &mut ::std::string::String) {
                #({
                    let start = out.len();
                    ::ezcp::ToOutput::write_output(#fields, out);
                    if out.len() > start && !out.ends_with('\n') {
                        out.push('\n');
                    }
                })*
            }
        }
    }
    .into()
}
