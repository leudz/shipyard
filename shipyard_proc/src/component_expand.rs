use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, Result};

pub(crate) fn expand_component(
    name: syn::Ident,
    generics: syn::Generics,
    attribute_input: Option<&syn::Attribute>,
) -> Result<TokenStream> {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let tracking = if let Some(tracking_attr) = attribute_input {
        let tracking: syn::Ident = tracking_attr.parse_args().map_err(|_| {
            Error::new_spanned(
                &tracking_attr.tokens,
                "Track should be one of: Untracked, Insertion, Modification, Deletion, Removal or All.",
            )
        })?;

        let tracking_name = tracking.to_string();

        match tracking_name.as_str() {
            "Untracked" | "Insertion" | "Modification" | "Deletion" | "Removal" | "All" => {}
            _ => return Err(Error::new_spanned(
                &tracking,
                "Track should be one of: Untracked, Insertion, Modification, Deletion, Removal or All.",
            )),
        }

        quote!(#tracking)
    } else {
        quote!(Untracked)
    };

    Ok(quote!(
        impl #impl_generics ::shipyard::Component for #name #ty_generics #where_clause {
            type Tracking = ::shipyard::track::#tracking;
        }
    ))
}
