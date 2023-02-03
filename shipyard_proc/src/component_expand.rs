use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand_component(name: syn::Ident, generics: syn::Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote!(
        impl #impl_generics ::shipyard::Component for #name #ty_generics #where_clause {}
    )
}

pub(crate) fn expand_unique(name: syn::Ident, generics: syn::Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote!(
        impl #impl_generics ::shipyard::Unique for #name #ty_generics #where_clause {}
    )
}
