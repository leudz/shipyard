extern crate proc_macro;

use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Error, Result};

#[proc_macro_derive(Component, attributes(track))]
pub fn component(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    let attribute_input: Option<&syn::Attribute> = input
        .attrs
        .iter()
        .filter(|attr| match attr.style {
            syn::AttrStyle::Outer => true,
            syn::AttrStyle::Inner(_) => false,
        })
        .find(|attr| attr.path.get_ident().map(ToString::to_string) == Some("track".to_string()));

    expand_component(name, generics, attribute_input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn expand_component(
    name: syn::Ident,
    generics: syn::Generics,
    attribute_input: Option<&syn::Attribute>,
) -> Result<TokenStream> {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let tracking = if let Some(tracking_attr) = attribute_input {
        let tracking: syn::Ident = tracking_attr.parse_args().map_err(|_| {
            Error::new_spanned(
                &tracking_attr.tokens,
                "Track should be one of: Nothing, Insertion, Modification, Removal, All.",
            )
        })?;

        quote!(#tracking)
    } else {
        quote!(Nothing)
    };
    let shipyard_name = crate_name("shipyard").map_err(|_| {
        Error::new(
            Span::call_site(),
            "shipyard needs to be present in `Cargo.toml`",
        )
    })?;

    match shipyard_name {
        FoundCrate::Itself => Ok(quote!(
            impl #impl_generics ::shipyard::Component for #name #ty_generics #where_clause {
                type Tracking = ::shipyard::track::#tracking;
            }
        )),
        FoundCrate::Name(shipyard_name) => {
            let shipyard_name = syn::Ident::new(&shipyard_name, Span::call_site());

            Ok(quote!(
                impl #impl_generics #shipyard_name::Component for #name #ty_generics #where_clause {
                    type Tracking = #shipyard_name::track::#tracking;
                }
            ))
        }
    }
}
