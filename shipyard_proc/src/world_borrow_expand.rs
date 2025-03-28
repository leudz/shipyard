use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_quote, Error, Result};

pub(crate) fn expand_world_borrow(
    name: syn::Ident,
    generics: syn::Generics,
    data: syn::Data,
) -> Result<TokenStream> {
    let fields = match data {
        syn::Data::Struct(data_struct) => data_struct.fields,
        _ => {
            return Err(Error::new(
                Span::call_site(),
                "WorldBorrow can only be implemented on structs",
            ))
        }
    };

    let mut gat_generics = generics.clone();
    for generic in gat_generics.params.iter_mut() {
        if let syn::GenericParam::Lifetime(lifetime) = generic {
            lifetime.lifetime = parse_quote!('__view);
            break;
        }
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let (_, gat_ty_generics, _) = gat_generics.split_for_impl();

    match fields {
        syn::Fields::Named(fields) => {
            let field_name = fields.named.iter().map(|field| &field.ident);
            let field_type = fields.named.iter().map(|field| &field.ty);

            let field_is_default = fields.named.iter().map(|field| {
                field.attrs.iter().any(|attr| {
                    let mut is_default = false;

                    if attr.path().is_ident("shipyard") {
                        let _ = attr.parse_nested_meta(|meta| {
                            is_default = is_default || meta.path.is_ident("default");

                            Ok(())
                        });
                    }

                    is_default
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
                            #field_name: <#field_type as ::shipyard::borrow::WorldBorrow>::world_borrow(world, last_run, current)?
                        )
                    }
                });

            Ok(quote!(
                impl #impl_generics ::shipyard::borrow::WorldBorrow for #name #ty_generics #where_clause {
                    type WorldView<'__view> = #name #gat_ty_generics;

                    fn world_borrow<'__w>(world: & '__w ::shipyard::World, last_run: Option<::shipyard::tracking::TrackingTimestamp>, current: ::shipyard::tracking::TrackingTimestamp) -> core::result::Result<Self::WorldView<'__w>, ::shipyard::error::GetStorage> {
                        Ok(#name {
                            #(#field),*
                        })
                    }
                }
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let world_borrow = fields.unnamed.iter().map(|field| {
                let field_type = &field.ty;
                quote!(<#field_type as ::shipyard::borrow::WorldBorrow>::world_borrow(world, last_run, current)?)
            });

            Ok(quote!(
                impl #impl_generics ::shipyard::borrow::WorldBorrow for #name #ty_generics #where_clause {
                    type WorldView<'__view> = #name #gat_ty_generics;

                    fn world_borrow<'__w>(world: & '__w ::shipyard::World, last_run: Option<::shipyard::tracking::TrackingTimestamp>, current: ::shipyard::tracking::TrackingTimestamp) -> core::result::Result<Self::WorldView<'__w>, ::shipyard::error::GetStorage> {
                        Ok(#name(#(#world_borrow),*))
                    }
                }
            ))
        }
        syn::Fields::Unit => Err(Error::new(
            Span::call_site(),
            "Unit struct cannot borrow from World",
        )),
    }
}
