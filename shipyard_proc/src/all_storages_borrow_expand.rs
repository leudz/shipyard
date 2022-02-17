use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, Result};

pub(crate) fn expand_all_storages_borrow(
    name: syn::Ident,
    generics: syn::Generics,
    data: syn::Data,
) -> Result<TokenStream> {
    let view_lifetime = generics.lifetimes().next().ok_or_else(|| {
        Error::new(
            name.span(),
            "views need a lifetime to borrow from the World",
        )
    })?;

    let fields = match data {
        syn::Data::Struct(data_struct) => data_struct.fields,
        _ => {
            return Err(Error::new(
                Span::call_site(),
                "AllStoragesBorrow can only be implemented on structs",
            ))
        }
    };

    let borrower = quote::format_ident!("{}Borrower", name);

    let borrower_generics = syn::Generics {
        lt_token: generics.lt_token,
        params: std::iter::FromIterator::from_iter(generics.params.clone().into_pairs().filter(
            |pair| match pair.value() {
                syn::GenericParam::Type(_) => true,
                syn::GenericParam::Lifetime(_) => false,
                syn::GenericParam::Const(_) => true,
            },
        )),
        gt_token: generics.gt_token,
        where_clause: generics
            .where_clause
            .as_ref()
            .map(|where_clause| syn::WhereClause {
                where_token: where_clause.where_token,
                predicates: std::iter::FromIterator::from_iter(
                    where_clause.predicates.clone().into_pairs().filter(|pair| {
                        match pair.value() {
                            syn::WherePredicate::Type(_) => true,
                            syn::WherePredicate::Lifetime(_) => false,
                            syn::WherePredicate::Eq(_) => true,
                        }
                    }),
                ),
            }),
    };

    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();
    let (_borrower_impl_generics, borrower_ty_generics, _borrower_where_clause) =
        borrower_generics.split_for_impl();

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
                            #field_name: <#field_type as ::shipyard::IntoBorrow>::Borrow::all_borrow(all_storages, last_run, current)?
                        )
                    }
                });

            Ok(quote!(
                impl #impl_generics ::shipyard::AllStoragesBorrow<#view_lifetime> for #borrower #borrower_ty_generics #where_clause {
                    fn all_borrow(all_storages: & #view_lifetime ::shipyard::AllStorages, last_run: Option<u32>, current: u32,) -> Result<Self::View, ::shipyard::error::GetStorage> {
                        Ok(#name {
                            #(#field),*
                        })
                    }
                }
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let all_storages_borrow = fields
                .unnamed
                .iter()
                .map(|field| {
                    let field_type = &field.ty;
                    quote!(<#field_type as ::shipyard::IntoBorrow>::Borrow::all_borrow(all_storages, last_run, current)?)
                });

            Ok(quote!(
                impl #impl_generics ::shipyard::AllStoragesBorrow<#view_lifetime> for #borrower #borrower_ty_generics #where_clause {
                    fn all_borrow(all_storages: & #view_lifetime ::shipyard::AllStorages, last_run: Option<u32>, current: u32) -> Result<Self::View, ::shipyard::error::GetStorage> {
                        Ok(#name(#(#all_storages_borrow),*))
                    }
                }
            ))
        }
        syn::Fields::Unit => Err(Error::new(
            Span::call_site(),
            "Unit struct cannot borrow from AllStorages",
        )),
    }
}
