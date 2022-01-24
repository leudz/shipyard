use proc_macro2::{Span, TokenStream};
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
        .ok_or_else(|| {
            Error::new(
                Span::call_site(),
                "views need a lifetime to borrow from the World",
            )
        })?;

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

            let field_is_default = fields.named.iter().map(|field| {
                field.attrs.iter().any(|attr| {
                    if attr.path.is_ident("shipyard") {
                        match attr.parse_meta() {
                            Ok(syn::Meta::List(list)) => {
                                list.nested.into_iter().any(|meta| match meta {
                                    syn::NestedMeta::Meta(syn::Meta::Path(path))
                                        if path.is_ident("default") =>
                                    {
                                        true
                                    }
                                    _ => false,
                                })
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                })
            });

            let field = field_name
                .into_iter()
                .zip(field_type)
                .zip(field_is_default)
                .map(|((field_name, field_type), field_is_default)| {
                    if field_is_default {
                        quote!(
                            #field_name: core::default::Default::default()
                        )
                    } else {
                        quote!(
                            #field_name: <#field_type as ::shipyard::IntoBorrow>::Borrow::borrow(world, last_run, current)?
                        )
                    }
                });

            Ok(quote!(
                #vis struct #borrower < #(#borrower_generics)* >(::core::marker::PhantomData<(#(#borrower_generics_idents)*)>) #where_clause;

                impl #impl_generics ::shipyard::IntoBorrow for #name #ty_generics #where_clause {
                    type Borrow = #borrower< #(#borrower_generics_idents2)* >;
                }

                impl #impl_generics ::shipyard::Borrow<#view_lifetime> for #borrower < #(#borrower_generics_idents3)* > #where_clause {
                    type View = #name #ty_generics;

                    fn borrow(world: & #view_lifetime ::shipyard::World, last_run: Option<u32>, current: u32) -> Result<Self::View, ::shipyard::error::GetStorage> {
                        Ok(#name {
                            #(#field),*
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
                    quote!(<#field_type as ::shipyard::IntoBorrow>::Borrow::borrow(world, last_run, current)?)
                });

            Ok(quote!(
                #vis struct #borrower < #(#borrower_generics)* >(::core::marker::PhantomData<(#(#borrower_generics_idents)*)>) #where_clause;

                impl #impl_generics ::shipyard::IntoBorrow for #name #ty_generics #where_clause {
                    type Borrow = #borrower< #(#borrower_generics_idents2)* >;
                }

                impl #impl_generics ::shipyard::Borrow<#view_lifetime> for #borrower < #(#borrower_generics_idents3)* > #where_clause {
                    type View = #name #ty_generics;

                    fn borrow(world: & #view_lifetime ::shipyard::World, last_run: Option<u32>, current: u32) -> Result<Self::View, ::shipyard::error::GetStorage> {
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
