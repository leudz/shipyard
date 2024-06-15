extern crate proc_macro;

mod borrow_expand;
mod borrow_info_expand;
mod component_expand;
mod into_iter_expand;
mod label_expand;
mod world_borrow_expand;

use borrow_expand::expand_borrow;
use borrow_info_expand::expand_borrow_info;
use component_expand::{expand_component, expand_unique};
use into_iter_expand::expand_into_iter;
use label_expand::expand_label;
use world_borrow_expand::expand_world_borrow;

#[proc_macro_derive(Component, attributes(track))]
pub fn component(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    let attribute_input: Option<&syn::Attribute> = input
        .attrs
        .iter()
        .filter(|attr| match attr.style {
            syn::AttrStyle::Outer => true,
            syn::AttrStyle::Inner(_) => false,
        })
        .find(|attr| {
            attr.path()
                .get_ident()
                .map(|ident| ident == "track")
                .unwrap_or(false)
        });

    expand_component(name, generics, attribute_input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(Unique)]
pub fn unique(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    expand_unique(name, generics).into()
}

#[proc_macro_derive(WorldBorrow, attributes(shipyard))]
pub fn borrow(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let data = input.data;

    expand_world_borrow(name, generics, data)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(Borrow, attributes(shipyard))]
pub fn all_storages_borrow(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let data = input.data;

    expand_borrow(name, generics, data)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(BorrowInfo, attributes(shipyard))]
pub fn borrow_info(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let data = input.data;

    expand_borrow_info(name, generics, data)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

/// Requires `Hash`, `Debug`, `PartialEq`, `Clone`
#[proc_macro_derive(Label)]
pub fn label(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    expand_label(name, generics).into()
}

#[proc_macro_derive(IntoIter)]
pub fn into_iter(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let data = input.data;

    expand_into_iter(name, generics, data)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}
