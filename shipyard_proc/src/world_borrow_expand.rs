use crate::field_is_default;
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

    let borrow_field = |field: &syn::Field| {
        if field_is_default(field) {
            quote!(core::default::Default::default())
        } else {
            let field_type = &field.ty;
            quote!(<#field_type as ::shipyard::borrow::WorldBorrow>::world_borrow(world, last_run, current)?)
        }
    };

    let construct = match fields {
        syn::Fields::Named(fields) => {
            let fields = fields.named.iter().map(|field| {
                let field_name = &field.ident;
                let value = borrow_field(field);
                quote!(#field_name: #value)
            });
            quote!(#name { #(#fields),* })
        }
        syn::Fields::Unnamed(fields) => {
            let fields = fields.unnamed.iter().map(borrow_field);
            quote!(#name(#(#fields),*))
        }
        syn::Fields::Unit => {
            return Err(Error::new(
                Span::call_site(),
                "Unit struct cannot borrow from World",
            ));
        }
    };

    Ok(quote!(
        impl #impl_generics ::shipyard::borrow::WorldBorrow for #name #ty_generics #where_clause {
            type WorldView<'__view> = #name #gat_ty_generics;

            fn world_borrow<'__w>(world: & '__w ::shipyard::World, last_run: Option<::shipyard::advanced::tracking::TrackingTimestamp>, current: ::shipyard::advanced::tracking::TrackingTimestamp) -> core::result::Result<Self::WorldView<'__w>, ::shipyard::error::GetStorage> {
                Ok(#construct)
            }
        }
    ))
}
