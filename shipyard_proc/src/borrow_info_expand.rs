use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Error, Result};

pub(crate) fn expand_borrow_info(
    name: syn::Ident,
    generics: syn::Generics,
    data: syn::Data,
) -> Result<TokenStream> {
    let fields = match data {
        syn::Data::Struct(data_struct) => data_struct.fields,
        _ => {
            return Err(Error::new(
                Span::call_site(),
                "BorrowInfo can only be implemented on structs",
            ))
        }
    };

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match fields {
        syn::Fields::Named(fields) => {
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

            let field_info = field_type.clone().zip(field_is_default.clone()).map(
                |(field_type, field_is_default)| {
                    if field_is_default {
                        quote!(();)
                    } else {
                        quote!(
                            <#field_type>::borrow_info(info);
                        )
                    }
                },
            );
            let field_tracking =
                field_type
                    .zip(field_is_default)
                    .map(|(field_type, field_is_default)| {
                        if field_is_default {
                            quote!(();)
                        } else {
                            quote!(
                                <#field_type>::enable_tracking(enable_tracking_fn);
                            )
                        }
                    });

            Ok(quote!(
                unsafe impl #impl_generics ::shipyard::borrow::BorrowInfo for #name #ty_generics #where_clause {
                    fn borrow_info(info: &mut Vec<::shipyard::scheduler::info::TypeInfo>) {
                        #(#field_info)*
                    }
                    fn enable_tracking(
                        enable_tracking_fn: &mut Vec<fn(&::shipyard::AllStorages) -> core::result::Result<(), ::shipyard::error::GetStorage>>,
                    ) {
                        #(#field_tracking)*
                    }
                }
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let field_type = fields.unnamed.iter().map(|field| &field.ty);
            let field_type_clone = field_type.clone();

            Ok(quote!(
                unsafe impl #impl_generics ::shipyard::borrow::BorrowInfo for #name #ty_generics #where_clause {
                    fn borrow_info(info: &mut Vec<::shipyard::scheduler::info::TypeInfo>) {
                        #(<#field_type_clone>::borrow_info(info);)*
                    }
                    fn enable_tracking(
                        enable_tracking_fn: &mut Vec<fn(&::shipyard::AllStorages) -> core::result::Result<(), ::shipyard::error::GetStorage>>,
                    ) {
                        #(<#field_type>::enable_tracking(enable_tracking_fn);)*
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
