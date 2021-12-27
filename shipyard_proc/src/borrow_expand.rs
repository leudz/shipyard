use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Error, Result};

pub(crate) fn expand_borrow(
    name: syn::Ident,
    generics: syn::Generics,
    vis: syn::Visibility,
    data: syn::Data,
) -> Result<TokenStream> {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

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
    let borrower_generics_idents2 = borrower_generics_idents.clone();
    let borrower_generics_idents3 = borrower_generics_idents.clone();

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
            let field_type = fields.named.iter().map(|field| &field.ty);

            Ok(quote!(
                #vis struct #borrower < #(#borrower_generics)* >(::core::marker::PhantomData<(#(#borrower_generics_idents)*)>) #where_clause;

                impl #impl_generics ::#shipyard_name::IntoBorrow for #name #ty_generics #where_clause {
                    type Borrow = #borrower< #(#borrower_generics_idents2)* >;
                }

                impl #impl_generics ::#shipyard_name::Borrow<#view_lifetime> for #borrower < #(#borrower_generics_idents3)* > #where_clause {
                    type View = #name #ty_generics;

                    fn borrow(world: & #view_lifetime ::#shipyard_name::World, last_run: Option<u32>, current: u32) -> Result<Self::View, ::#shipyard_name::error::GetStorage> {
                        Ok(#name {
                            #(#field_name: <#field_type as ::#shipyard_name::IntoBorrow>::Borrow::borrow(world, last_run, current)?),*
                        })
                    }
                }
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let world_borrow = fields
                .unnamed
                .iter()
                .map(|field| {
                    let field_type = &field.ty;
                    quote!(<#field_type as ::#shipyard_name::IntoBorrow>::Borrow::borrow(world, last_run, current)?)
                });

            Ok(quote!(
                #vis struct #borrower < #(#borrower_generics)* >(::core::marker::PhantomData<(#(#borrower_generics_idents)*)>) #where_clause;

                impl #impl_generics ::#shipyard_name::IntoBorrow for #name #ty_generics #where_clause {
                    type Borrow = #borrower< #(#borrower_generics_idents2)* >;
                }

                impl #impl_generics ::#shipyard_name::Borrow<#view_lifetime> for #borrower < #(#borrower_generics_idents3)* > #where_clause {
                    type View = #name #ty_generics;

                    fn borrow(world: & #view_lifetime ::#shipyard_name::World, last_run: Option<u32>, current: u32) -> Result<Self::View, ::#shipyard_name::error::GetStorage> {
                        Ok(#name(#(#world_borrow),*))
                    }
                }
            ))
        }
        syn::Fields::Unit => Ok(quote!(
            unreachable!("Unit struct cannot borrow from World");
        )),
    }
}
