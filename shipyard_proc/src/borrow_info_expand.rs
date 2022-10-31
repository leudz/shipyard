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
                    if attr.path.is_ident("shipyard") {
                        match attr.parse_meta() {
                            Ok(syn::Meta::List(list)) => list.nested.into_iter().any(|meta| {
                                matches!(meta, syn::NestedMeta::Meta(syn::Meta::Path(path)) if path.is_ident("default"))
                            }),
                            _ => false,
                        }
                    } else {
                        false
                    }
                })
            });

            let field = field_type
                .zip(field_is_default)
                .map(|(field_type, field_is_default)| {
                    if field_is_default {
                        quote!(();)
                    } else {
                        quote!(
                            <#field_type>::borrow_info(info);
                        )
                    }
                });

            Ok(quote!(
                unsafe impl #impl_generics ::shipyard::BorrowInfo for #name #ty_generics #where_clause {
                    fn borrow_info(info: &mut Vec<::shipyard::info::TypeInfo>) {
                        #(#field)*
                    }
                }
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let field_type = fields.unnamed.iter().map(|field| &field.ty);

            Ok(quote!(
                unsafe impl #impl_generics ::shipyard::BorrowInfo for #name #ty_generics #where_clause {
                    fn borrow_info(info: &mut Vec<::shipyard::info::TypeInfo>) {
                        #(<#field_type>::borrow_info(info);)*
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
