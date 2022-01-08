use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{Error, Result};

pub(crate) fn expand_borrow_info(
    name: syn::Ident,
    generics: syn::Generics,
    data: syn::Data,
) -> Result<TokenStream> {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

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

    match fields {
        syn::Fields::Named(fields) => {
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
                unsafe impl #impl_generics ::#shipyard_name::BorrowInfo for #name #ty_generics #where_clause {
                    fn borrow_info(info: &mut Vec<::#shipyard_name::info::TypeInfo>) {
                        #(#field)*
                    }
                }
            ))
        }
        syn::Fields::Unnamed(fields) => {
            let field_type = fields.unnamed.iter().map(|field| &field.ty);

            Ok(quote!(
                unsafe impl #impl_generics ::#shipyard_name::BorrowInfo for #name #ty_generics #where_clause {
                    fn borrow_info(info: &mut Vec<::#shipyard_name::info::TypeInfo>) {
                        #(<#field_type>::borrow_info(info);)*
                    }
                }
            ))
        }
        syn::Fields::Unit => Ok(quote!(
            unreachable!("Unit struct cannot borrow from World");
        )),
    }
}
