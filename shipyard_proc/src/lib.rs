extern crate proc_macro;

mod borrow_expand;
mod borrow_info_expand;
mod component_expand;
mod world_borrow_expand;

use borrow_expand::expand_borrow;
use borrow_info_expand::expand_borrow_info;
use component_expand::{expand_component, expand_unique};
use world_borrow_expand::expand_world_borrow;

#[proc_macro_derive(Component)]
pub fn component(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    let name = input.ident;
    let generics = input.generics;

    expand_component(name, generics).into()
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
