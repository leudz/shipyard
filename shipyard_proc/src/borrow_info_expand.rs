use crate::field_is_default;
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

    if matches!(fields, syn::Fields::Unit) {
        return Err(Error::new(
            Span::call_site(),
            "Unit struct cannot borrow from World",
        ));
    }

    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let field_types: Vec<_> = fields
        .iter()
        .filter(|field| !field_is_default(field))
        .map(|field| &field.ty)
        .collect();

    Ok(quote!(
        unsafe impl #impl_generics ::shipyard::borrow::BorrowInfo for #name #ty_generics #where_clause {
            fn borrow_info(info: &mut Vec<::shipyard::scheduler::info::TypeInfo>) {
                #(<#field_types>::borrow_info(info);)*
            }
            fn enable_tracking(
                enable_tracking_fn: &mut Vec<fn(&::shipyard::all_storages::AllStorages) -> core::result::Result<(), ::shipyard::error::GetStorage>>,
            ) {
                #(<#field_types>::enable_tracking(enable_tracking_fn);)*
            }
        }
    ))
}
