use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, punctuated::Punctuated, spanned::Spanned, token::Comma, Error, ExprReference,
    Field, Ident, Index, LitStr, Result,
};

pub(crate) fn expand_into_iter(
    name: syn::Ident,
    mut generics: syn::Generics,
    data: syn::Data,
    attrs: Vec<syn::Attribute>,
) -> Result<TokenStream> {
    let fields = match data {
        syn::Data::Struct(data_struct) => data_struct.fields,
        _ => {
            return Err(Error::new(
                Span::call_site(),
                "IntoIter can only be implemented on structs",
            ))
        }
    };

    let name_string = name.to_string();
    let iter_item_name = attrs
        .iter()
        .find(|attr| attr.path().is_ident("shipyard"))
        .map::<Result<String>, _>(|attr| {
            let mut item_name = String::new();
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("item_name") {
                    let value = meta.value()?;
                    let s: LitStr = value.parse()?;
                    item_name = s.value();

                    Ok(())
                } else {
                    Err(Error::new(
                        meta.path.span(),
                        "Unknown attribute. Possible attribute: item_name",
                    ))
                }
            })?;

            Ok(item_name)
        })
        .unwrap_or_else(|| {
            let mut iter_item_name = name_string.trim_end_matches("View").to_string();
            if iter_item_name.len() == name_string.len() {
                iter_item_name += "Item";
            }
            Ok(iter_item_name)
        })?;

    let item_name = Ident::new(&iter_item_name, name_string.span());
    let iter_name = Ident::new(&format!("{}Iter", name_string), name_string.span());
    let par_iter_name = Ident::new(&format!("{}ParIter", name_string), name_string.span());

    let Some(lifetime) = generics.lifetimes_mut().next() else {
        return Err(Error::new(generics.span(), "Views must have a lifetime"));
    };
    lifetime.lifetime = parse_quote!('__view);
    lifetime.bounds.push_value(parse_quote!('__tmp));

    let mut iter_generics = generics.clone();
    iter_generics
        .params
        .push(syn::GenericParam::Lifetime(parse_quote!('__tmp)));

    let (iter_impl_generics, iter_ty_generics, iter_where_clause) = iter_generics.split_for_impl();
    let (_impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    match fields {
        syn::Fields::Named(fields) => {
            if fields.named.is_empty() {
                return Err(Error::new(
                    fields.span(),
                    "Structs without fields cannot be iterated",
                ));
            }

            let mut item_fields = Punctuated::<Field, Comma>::new();
            let mut iter_fields = Punctuated::<Field, Comma>::new();
            let mut iter_fields_access = Punctuated::<ExprReference, Comma>::new();
            let mut iter_fields_variable = Punctuated::<Ident, Comma>::new();
            'field: for field in fields.named.iter() {
                if let syn::Type::Path(path) = &field.ty {
                    for segment in path.path.segments.iter() {
                        let segment_ident = segment.ident.to_string();

                        if segment_ident == "View" {
                            let mut tys = match &segment.arguments {
                                syn::PathArguments::AngleBracketed(args) => args
                                    .args
                                    .iter()
                                    .filter(|arg| matches!(arg, syn::GenericArgument::Type(_))),
                                _ => {
                                    return Err(Error::new(
                                        segment.arguments.span(),
                                        "Unexpected syntax",
                                    ));
                                }
                            };

                            let comp_ty = tys
                                .next()
                                .ok_or_else(|| Error::new(segment.span(), "Missing generic"))?;

                            let tracking_ty = tys.next();

                            let field_name = field.ident.clone().unwrap();
                            let trimmed_field_name = field
                                .attrs
                                .iter()
                                .find(|attr| attr.path().is_ident("shipyard"))
                                .map::<Result<Ident>, _>(|attr| {
                                    let mut item_name = String::new();
                                    attr.parse_nested_meta(|meta| {
                                        if meta.path.is_ident("item_field_name") {
                                            let value = meta.value()?;
                                            let s: LitStr = value.parse()?;
                                            item_name = s.value();

                                            Ok(())
                                        } else {
                                            Err(Error::new(
                                                meta.path.span(),
                                                "Unknown attribute. Possible attribute: item_field_name",
                                            ))
                                        }
                                    })?;

                                    Ok(Ident::new(&item_name, field_name.span()))
                                })
                                .unwrap_or_else(|| {
                                    let field_name = field.ident.as_ref().unwrap();
                                    let field_name_string = field_name.to_string();
                                    let trimmed = field_name_string.trim_start_matches("v_");

                                    Ok(Ident::new(
                                        if trimmed.len() < field_name_string.len() {
                                            trimmed
                                        } else {
                                            field_name_string.as_str()
                                        },
                                        field_name.span(),
                                    ))
                                })?;
                            let item_ty = parse_quote!(#trimmed_field_name: <<&'__tmp shipyard::View<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoAbstract>::AbsView as shipyard::iter::AbstractMut>::Out);
                            item_fields.push(item_ty);

                            let abstract_view_ty = parse_quote!(<&'__tmp shipyard::View<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoAbstract>::AbsView);
                            iter_fields.push(abstract_view_ty);

                            let field = parse_quote!(self.#field_name);
                            iter_fields_access.push(ExprReference {
                                attrs: Vec::new(),
                                and_token: parse_quote!(&),
                                mutability: None,
                                expr: Box::new(field),
                            });

                            iter_fields_variable.push(trimmed_field_name);

                            continue 'field;
                        } else if segment_ident == "ViewMut" {
                            let mut tys = match &segment.arguments {
                                syn::PathArguments::AngleBracketed(args) => args
                                    .args
                                    .iter()
                                    .filter(|arg| matches!(arg, syn::GenericArgument::Type(_))),
                                _ => {
                                    return Err(Error::new(
                                        segment.arguments.span(),
                                        "Unexpected syntax",
                                    ))
                                }
                            };

                            let comp_ty = tys
                                .next()
                                .ok_or_else(|| Error::new(segment.span(), "Missing generic"))?;

                            let tracking_ty = tys.next();

                            let field_name = field.ident.clone().unwrap();
                            let trimmed_field_name = {
                                let field_name = field.ident.clone().unwrap();
                                let field_name_string = field_name.to_string();
                                let trimmed = field_name_string.trim_start_matches("vm_");

                                Ident::new(
                                    if trimmed.len() < field_name_string.len() {
                                        trimmed
                                    } else {
                                        field_name_string.as_str()
                                    },
                                    field_name.span(),
                                )
                            };
                            let item_ty = parse_quote!(#trimmed_field_name: <<&'__tmp mut shipyard::ViewMut<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoAbstract>::AbsView as shipyard::iter::AbstractMut>::Out);
                            item_fields.push(item_ty);

                            let abtract_view_ty = parse_quote!(<&'__tmp mut shipyard::ViewMut<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoAbstract>::AbsView);
                            iter_fields.push(abtract_view_ty);

                            let field = parse_quote!(self.#field_name);
                            iter_fields_access.push(ExprReference {
                                attrs: Vec::new(),
                                and_token: parse_quote!(&),
                                mutability: Some(parse_quote!(mut)),
                                expr: Box::new(field),
                            });

                            iter_fields_variable.push(trimmed_field_name);

                            continue 'field;
                        }
                    }

                    return Err(Error::new(field.ty.span(), "Field is not a view"));
                } else {
                    return Err(Error::new(field.ty.span(), "Field is not a view"));
                }
            }

            Ok(quote!(
                struct #item_name #iter_generics {
                    #item_fields
                }

                struct #iter_name #iter_generics (
                    shipyard::iter::Iter<(
                        #iter_fields
                    )>
                );

                struct #par_iter_name #iter_generics (
                    shipyard::iter::ParIter<(
                        #iter_fields
                    )>
                );

                impl #iter_impl_generics shipyard::iter::IntoIter for &'__tmp mut #name #ty_generics #where_clause {
                    type IntoIter = #iter_name #iter_ty_generics;
                    type IntoParIter = #par_iter_name #iter_ty_generics;

                    fn iter(self) -> Self::IntoIter {
                        #iter_name((#iter_fields_access).iter())
                    }

                    fn iter_by<__D: 'static>(self) -> Self::IntoIter {
                        #iter_name((#iter_fields_access).iter_by::<__D>())
                    }

                    fn par_iter(self) -> Self::IntoParIter {
                        #par_iter_name((#iter_fields_access).par_iter())
                    }
                }

                impl #iter_impl_generics core::iter::Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                    type Item = #item_name #iter_ty_generics;

                    #[inline]
                    fn next(&mut self) -> Option<Self::Item> {
                        if let Some((#iter_fields_variable)) = core::iter::Iterator::next(&mut self.0) {
                            Some(#item_name { #iter_fields_variable })
                        } else {
                            None
                        }
                    }

                    #[inline]
                    fn size_hint(&self) -> (usize, core::option::Option<usize>) {
                        core::iter::Iterator::size_hint(&self.0)
                    }

                    #[inline]
                    fn fold<__B, __F>(self, init: __B, mut f: __F) -> __B
                    where
                        Self: Sized,
                        __F: FnMut(__B, Self::Item) -> __B,
                    {
                        core::iter::Iterator::fold(self.0, init, |init, (#iter_fields_variable)| {
                            f(init, #item_name { #iter_fields_variable })
                        })
                    }
                }

                impl #iter_impl_generics shipyard::iter::LastId for #iter_name #iter_ty_generics #iter_where_clause {
                    #[inline]
                    unsafe fn last_id(&self) -> shipyard::EntityId {
                        shipyard::iter::LastId::last_id(&self.0)
                    }

                    #[inline]
                    unsafe fn last_id_back(&self) -> shipyard::EntityId {
                        shipyard::iter::LastId::last_id_back(&self.0)
                    }
                }

                impl #iter_impl_generics shipyard::iter::__ParallelIterator for #par_iter_name #iter_ty_generics #iter_where_clause {
                    type Item = #item_name #iter_ty_generics;

                    #[inline]
                    fn drive_unindexed<__C>(self, consumer: __C) -> __C::Result
                    where
                        __C: shipyard::iter::__UnindexedConsumer<Self::Item>,
                    {
                        shipyard::iter::__ParallelIterator::drive_unindexed(
                            shipyard::iter::__ParallelIterator::map(
                                self.0,
                                |(#iter_fields_variable)| #item_name { #iter_fields_variable }
                            ),
                            consumer
                        )
                    }

                    #[inline]
                    fn opt_len(&self) -> core::option::Option<usize> {
                        shipyard::iter::__ParallelIterator::opt_len(&self.0)
                    }
                }
            ))
        }
        syn::Fields::Unnamed(fields) => {
            if fields.unnamed.is_empty() {
                return Err(Error::new(
                    fields.span(),
                    "Structs without fields cannot be iterated",
                ));
            }

            let mut item_fields = Punctuated::<Field, Comma>::new();
            let mut iter_fields = Punctuated::<Field, Comma>::new();
            let mut iter_fields_access = Punctuated::<ExprReference, Comma>::new();
            let mut iter_fields_variable = Punctuated::<Ident, Comma>::new();
            'field: for (field_index, field) in fields.unnamed.iter().enumerate() {
                if let syn::Type::Path(path) = &field.ty {
                    for segment in path.path.segments.iter() {
                        let segment_ident = segment.ident.to_string();

                        if segment_ident == "View" {
                            let mut tys = match &segment.arguments {
                                syn::PathArguments::AngleBracketed(args) => args
                                    .args
                                    .iter()
                                    .filter(|arg| matches!(arg, syn::GenericArgument::Type(_))),
                                _ => {
                                    return Err(Error::new(
                                        segment.arguments.span(),
                                        "Unexpected syntax",
                                    ))
                                }
                            };

                            let comp_ty = tys
                                .next()
                                .ok_or_else(|| Error::new(segment.span(), "Missing generic"))?;

                            let tracking_ty = tys.next();

                            let item_ty = parse_quote!(<<&'__tmp shipyard::View<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoAbstract>::AbsView as shipyard::iter::AbstractMut>::Out);
                            item_fields.push(item_ty);

                            let abstract_view_ty = parse_quote!(<&'__tmp shipyard::View<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoAbstract>::AbsView);
                            iter_fields.push(abstract_view_ty);

                            let index = Index {
                                index: field_index as u32,
                                span: Span::call_site(),
                            };
                            let field = parse_quote!(self.#index);
                            iter_fields_access.push(ExprReference {
                                attrs: Vec::new(),
                                and_token: parse_quote!(&),
                                mutability: None,
                                expr: Box::new(field),
                            });

                            iter_fields_variable.push(Ident::new(
                                &format!("field{}", field_index),
                                Span::call_site(),
                            ));

                            continue 'field;
                        } else if segment_ident == "ViewMut" {
                            let mut tys = match &segment.arguments {
                                syn::PathArguments::AngleBracketed(args) => args
                                    .args
                                    .iter()
                                    .filter(|arg| matches!(arg, syn::GenericArgument::Type(_))),
                                _ => {
                                    return Err(Error::new(
                                        segment.arguments.span(),
                                        "Unexpected syntax",
                                    ))
                                }
                            };

                            let comp_ty = tys
                                .next()
                                .ok_or_else(|| Error::new(segment.span(), "Missing generic"))?;

                            let tracking_ty = tys.next();

                            let item_ty = parse_quote!(<<&'__tmp mut shipyard::ViewMut<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoAbstract>::AbsView as shipyard::iter::AbstractMut>::Out);
                            item_fields.push(item_ty);

                            let abtract_view_ty = parse_quote!(<&'__tmp mut shipyard::ViewMut<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoAbstract>::AbsView);
                            iter_fields.push(abtract_view_ty);

                            let index = Index {
                                index: field_index as u32,
                                span: Span::call_site(),
                            };
                            let field = parse_quote!(self.#index);
                            iter_fields_access.push(ExprReference {
                                attrs: Vec::new(),
                                and_token: parse_quote!(&),
                                mutability: Some(parse_quote!(mut)),
                                expr: Box::new(field),
                            });

                            iter_fields_variable.push(Ident::new(
                                &format!("field{}", field_index),
                                Span::call_site(),
                            ));

                            continue 'field;
                        }
                    }

                    return Err(Error::new(field.ty.span(), "Field is not a view"));
                } else {
                    return Err(Error::new(field.ty.span(), "Field is not a view"));
                }
            }

            Ok(quote!(
                struct #item_name #iter_generics (
                    #item_fields
                );

                struct #iter_name #iter_generics (
                    shipyard::iter::Iter<(
                        #iter_fields
                    )>
                );

                struct #par_iter_name #iter_generics (
                    shipyard::iter::ParIter<(
                        #iter_fields
                    )>
                );

                impl #iter_impl_generics shipyard::iter::IntoIter for &'__tmp mut #name #ty_generics #where_clause {
                    type IntoIter = #iter_name #iter_ty_generics;
                    type IntoParIter = #par_iter_name #iter_ty_generics;

                    fn iter(self) -> Self::IntoIter {
                        #iter_name((#iter_fields_access).iter())
                    }

                    fn iter_by<__D: 'static>(self) -> Self::IntoIter {
                        #iter_name((#iter_fields_access).iter_by::<__D>())
                    }

                    fn par_iter(self) -> Self::IntoParIter {
                        #par_iter_name((#iter_fields_access).par_iter())
                    }
                }

                impl #iter_impl_generics core::iter::Iterator for #iter_name #iter_ty_generics #iter_where_clause {
                    type Item = #item_name #iter_ty_generics;

                    #[inline]
                    fn next(&mut self) -> Option<Self::Item> {
                        if let Some((#iter_fields_variable)) = core::iter::Iterator::next(&mut self.0) {
                            Some(#item_name(#iter_fields_variable))
                        } else {
                            None
                        }
                    }

                    #[inline]
                    fn size_hint(&self) -> (usize, core::option::Option<usize>) {
                        core::iter::Iterator::size_hint(&self.0)
                    }

                    #[inline]
                    fn fold<__B, __F>(self, init: __B, mut f: __F) -> __B
                    where
                        Self: Sized,
                        __F: FnMut(__B, Self::Item) -> __B,
                    {
                        core::iter::Iterator::fold(self.0, init, |init, (#iter_fields_variable)| {
                            f(init, #item_name(#iter_fields_variable))
                        })
                    }
                }

                impl #iter_impl_generics shipyard::iter::LastId for #iter_name #iter_ty_generics #iter_where_clause {
                    #[inline]
                    unsafe fn last_id(&self) -> shipyard::EntityId {
                        shipyard::iter::LastId::last_id(&self.0)
                    }

                    #[inline]
                    unsafe fn last_id_back(&self) -> shipyard::EntityId {
                        shipyard::iter::LastId::last_id_back(&self.0)
                    }
                }

                impl #iter_impl_generics shipyard::iter::__ParallelIterator for #par_iter_name #iter_ty_generics #iter_where_clause {
                    type Item = #item_name #iter_ty_generics;

                    #[inline]
                    fn drive_unindexed<__C>(self, consumer: __C) -> __C::Result
                    where
                        __C: shipyard::iter::__UnindexedConsumer<Self::Item>,
                    {
                        shipyard::iter::__ParallelIterator::drive_unindexed(
                            shipyard::iter::__ParallelIterator::map(
                                self.0,
                                |(#iter_fields_variable)| #item_name(#iter_fields_variable)
                            ),
                            consumer
                        )
                    }

                    #[inline]
                    fn opt_len(&self) -> core::option::Option<usize> {
                        shipyard::iter::__ParallelIterator::opt_len(&self.0)
                    }
                }
            ))
        }
        syn::Fields::Unit => Err(Error::new(
            Span::call_site(),
            "Unit structs cannot be iterated",
        )),
    }
}
