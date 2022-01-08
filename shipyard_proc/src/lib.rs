extern crate proc_macro;

mod all_storages_borrow_expand;
mod borrow_expand;
mod borrow_info_expand;
mod component_expand;

use all_storages_borrow_expand::expand_all_storages_borrow;
use borrow_expand::expand_borrow;
use borrow_info_expand::expand_borrow_info;
use component_expand::expand_component;

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
        .find(|attr| attr.path.get_ident().map(ToString::to_string) == Some("track".to_string()));

    expand_component(name, generics, attribute_input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(Borrow, attributes(shipyard))]
pub fn borrow(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let vis = input.vis;
    let data = input.data;

    expand_borrow(name, generics, vis, data)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[proc_macro_derive(AllStoragesBorrow, attributes(shipyard))]
pub fn all_storages_borrow(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;
    let data = input.data;

    expand_all_storages_borrow(name, generics, data)
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
