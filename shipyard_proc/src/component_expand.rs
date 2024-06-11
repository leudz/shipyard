use proc_macro2::{Span, TokenStream, TokenTree};
use quote::quote;
use syn::{Error, Result};

pub(crate) fn expand_component(
    name: syn::Ident,
    generics: syn::Generics,
    attribute_input: Option<&syn::Attribute>,
) -> Result<TokenStream> {
    let tracking = if let Some(tracking_attr) = attribute_input {
        let mut track_insertion = false;
        let mut track_modification = false;
        let mut track_deletion = false;
        let mut track_removal = false;

        match &tracking_attr.meta {
            syn::Meta::List(list) => {
                for token in list.tokens.clone() {
                    if let TokenTree::Ident(ident) = token {
                        if ident == "Insertion" {
                            track_insertion = true;
                        } else if ident == "Modification" {
                            track_modification = true;
                        } else if ident == "Deletion" {
                            track_deletion = true;
                        } else if ident == "Removal" {
                            track_removal = true;
                        } else if ident == "All" {
                            track_insertion = true;
                            track_modification = true;
                            track_deletion = true;
                            track_removal = true;
                        } else {
                            return Err(Error::new_spanned(
                                &ident,
                                "Track should be either: Insertion, Modification, Deletion, Removal or All.",
                            ))
                        }
                    }
                }
            },
            _ => {
                return Err(Error::new_spanned(
                    &tracking_attr.meta,
                    "Track should be a list of either: Insertion, Modification, Deletion, Removal or All.",
                ))
            }
        };

        let tracking = match (
            track_insertion,
            track_modification,
            track_deletion,
            track_removal,
        ) {
            (true, true, true, true) => "All",
            (true, true, true, false) => "InsertionAndModificationAndDeletion",
            (true, true, false, true) => "InsertionAndModificationAndRemoval",
            (true, true, false, false) => "InsertionAndModification",
            (true, false, true, true) => "InsertionAndDeletionAndRemoval",
            (true, false, true, false) => "InsertionAndDeletion",
            (true, false, false, true) => "InsertionAndRemoval",
            (true, false, false, false) => "Insertion",
            (false, true, true, true) => "ModificationAndDeletionAndRemoval",
            (false, true, true, false) => "ModificationAndDeletion",
            (false, true, false, true) => "ModificationAndRemoval",
            (false, true, false, false) => "Modification",
            (false, false, true, true) => "DeletionAndRemoval",
            (false, false, true, false) => "Deletion",
            (false, false, false, true) => "Removal",
            (false, false, false, false) => "Untracked",
        };

        syn::Ident::new(tracking, Span::call_site())
    } else {
        syn::Ident::new("Untracked", Span::call_site())
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    Ok(quote!(
        impl #impl_generics ::shipyard::Component for #name #ty_generics #where_clause {
            type Tracking = ::shipyard::track::#tracking;
        }
    ))
}

pub(crate) fn expand_unique(name: syn::Ident, generics: syn::Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote!(
        impl #impl_generics ::shipyard::Unique for #name #ty_generics #where_clause {}
    )
}
