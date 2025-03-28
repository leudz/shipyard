use proc_macro2::TokenStream;
use quote::quote;

pub(crate) fn expand_label(name: syn::Ident, generics: syn::Generics) -> TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    quote!(
        impl #impl_generics ::shipyard::scheduler::Label for #name #ty_generics #where_clause {
            fn as_any(&self) -> &dyn ::core::any::Any {
                self
            }
            fn dyn_eq(&self, other: &dyn ::shipyard::scheduler::Label) -> bool {
                if let Some(other) = other.as_any().downcast_ref::<Self>() {
                    self == other
                } else {
                    false
                }
            }
            fn dyn_hash(&self, mut state: &mut dyn ::core::hash::Hasher) {
                ::core::hash::Hash::hash(self, &mut state);
            }
            fn dyn_clone(&self) -> Box<dyn ::shipyard::scheduler::Label> {
                Box::new(Clone::clone(self))
            }
            fn dyn_debug(&self, f: &mut ::core::fmt::Formatter<'_>) -> core::result::Result<(), core::fmt::Error> {
                ::core::fmt::Debug::fmt(self, f)
            }
        }
    )
}
