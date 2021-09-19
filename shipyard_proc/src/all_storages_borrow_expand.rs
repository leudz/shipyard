use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Error, Result};

pub(crate) fn expand_all_storages_borrow(
    name: syn::Ident,
    generics: syn::Generics,
    data: syn::Data,
) -> Result<TokenStream> {
    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();

    let view_lifetime = generics
        .params
        .first()
        .and_then(|generic| match generic {
            syn::GenericParam::Type(_) => None,
            syn::GenericParam::Lifetime(lifetime) => Some(&lifetime.lifetime),
            syn::GenericParam::Const(_) => None,
        })
        .ok_or(Error::new(
            Span::call_site(),
            "views need a lifetime to borrow from the World",
        ))?;

    let borrower_generics = generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(generic) => Some(generic),
            syn::GenericParam::Lifetime(_) | syn::GenericParam::Const(_) => None,
        })
        .collect::<Vec<_>>();

    let borrower_generics_idents = borrower_generics.iter().map(|generic| &generic.ident);

    let shipyard_name = crate_name("shipyard").map_err(|_| {
        Error::new(
            Span::call_site(),
            "shipyard needs to be present in `Cargo.toml`",
        )
    })?;

    let shipyard_name: syn::Ident = match shipyard_name {
        FoundCrate::Itself => quote::format_ident!("shipyard"),
        FoundCrate::Name(name) => quote::format_ident!("{}", name),
    };

    let fields = match data {
        syn::Data::Struct(data_struct) => data_struct.fields,
        _ => {
            return Err(Error::new(
                Span::call_site(),
                "System can only be implemented on structs",
            ))
        }
    };

    let borrower = quote::format_ident!("{}Borrower", name);

    match fields {
        syn::Fields::Named(fields) => {
            let field_name = fields.named.iter().map(|field| &field.ident);

            Ok(quote!(
                impl #impl_generics ::#shipyard_name::AllStoragesBorrow<#view_lifetime> for #borrower < #(#borrower_generics_idents)* > #where_clause {
                    fn all_borrow(all_storages: & #view_lifetime ::#shipyard_name::AllStorages) -> Result<Self::View, ::#shipyard_name::error::GetStorage> {
                        Ok(#name {
                            #(#field_name: all_storages.borrow()?),*
                        })
                    }
                }
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let all_storages_borrow = fields
                .unnamed
                .iter()
                .map(|_| quote!(all_storages.borrow()?));

            Ok(quote!(
                impl #impl_generics ::#shipyard_name::AllStoragesBorrow<#view_lifetime> for #borrower < #(#borrower_generics_idents)* > #where_clause {
                    fn all_borrow(all_storages: & #view_lifetime ::#shipyard_name::AllStorages) -> Result<Self::View, ::#shipyard_name::error::GetStorage> {
                        Ok(#name(#(#all_storages_borrow),*))
                    }
                }
            ))
        }
        syn::Fields::Unit => Ok(quote!(
            unreachable!("Unit struct cannot borrow from World");
        )),
    }
}
