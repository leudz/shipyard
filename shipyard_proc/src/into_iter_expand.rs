use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    parse_quote, punctuated::Punctuated, spanned::Spanned, token::Comma, Error, ExprReference,
    Field, Ident, LitStr, Result,
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
    let iter_name = Ident::new(&format!("{}Shiperator", name_string), name_string.span());

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
            let mut contains_view_mut = false;
            for field in fields.named.iter() {
                if let syn::Type::Path(path) = &field.ty {
                    let field_attrs = field
                        .attrs
                        .iter()
                        .find(|attr| attr.path().is_ident("shipyard"));

                    let mut ignore = false;
                    let mut field_name_override = None;
                    field_attrs
                        .map(|attr| {
                            attr.parse_nested_meta(|meta| {
                                if meta.path.is_ident("item_field_skip") {
                                    ignore = true;

                                    Ok(())
                                } else if meta.path.is_ident("item_field_name") {
                                    let value = meta.value()?;
                                    let s: LitStr = value.parse()?;
                                    field_name_override = Some(Ident::new(&s.value(), field.ident.span()));

                                    Ok(())
                                } else {
                                    Err(Error::new(
                                        meta.path.span(),
                                        "Unknown attribute. Possible attribute: item_field_skip, item_field_name",
                                    ))
                                }
                            })
                        }).transpose()?;

                    if ignore {
                        continue;
                    }

                    let segment = path.path.segments.last().unwrap();

                    if segment.ident == "View" {
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
                        let trimmed_field_name = field_name_override.unwrap_or_else(|| {
                            let field_name = field.ident.as_ref().unwrap();
                            let field_name_string = field_name.to_string();
                            let trimmed = field_name_string.trim_start_matches("v_");

                            Ident::new(
                                if trimmed.len() < field_name_string.len() {
                                    trimmed
                                } else {
                                    field_name_string.as_str()
                                },
                                field_name.span(),
                            )
                        });
                        let item_ty = parse_quote!(#trimmed_field_name: <<&'__tmp shipyard::View<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoShiperator>::Shiperator as shipyard::iter::ShiperatorOutput>::Out);
                        item_fields.push(item_ty);

                        let abstract_view_ty =
                            parse_quote!(&'__tmp shipyard::View<'__view, #comp_ty, #tracking_ty>);
                        iter_fields.push(abstract_view_ty);

                        let field = parse_quote!(self.#field_name);
                        iter_fields_access.push(ExprReference {
                            attrs: Vec::new(),
                            and_token: parse_quote!(&),
                            mutability: None,
                            expr: Box::new(field),
                        });

                        iter_fields_variable.push(trimmed_field_name);
                    } else if segment.ident == "ViewMut" {
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

                        contains_view_mut = true;

                        let comp_ty = tys
                            .next()
                            .ok_or_else(|| Error::new(segment.span(), "Missing generic"))?;

                        let tracking_ty = tys.next();

                        let field_name = field.ident.clone().unwrap();
                        let trimmed_field_name = field_name_override.unwrap_or_else(|| {
                            let field_name = field.ident.as_ref().unwrap();
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
                        });
                        let item_ty = parse_quote!(#trimmed_field_name: <<&'__tmp mut shipyard::ViewMut<'__view, #comp_ty, #tracking_ty> as shipyard::iter::IntoShiperator>::Shiperator as shipyard::iter::ShiperatorOutput>::Out);
                        item_fields.push(item_ty);

                        let abtract_view_ty = parse_quote!(&'__tmp mut shipyard::ViewMut<'__view, #comp_ty, #tracking_ty>);
                        iter_fields.push(abtract_view_ty);

                        let field = parse_quote!(self.#field_name);
                        iter_fields_access.push(ExprReference {
                            attrs: Vec::new(),
                            and_token: parse_quote!(&),
                            mutability: Some(parse_quote!(mut)),
                            expr: Box::new(field),
                        });

                        iter_fields_variable.push(trimmed_field_name);
                    } else {
                        return Err(Error::new(field.ty.span(), "Field is not a view"));
                    }
                }
            }

            let r#mut = if contains_view_mut {
                Some(quote!(mut))
            } else {
                None
            };

            Ok(quote!(
                struct #item_name #iter_generics {
                    #item_fields
                }

                #[derive(Clone)]
                struct #iter_name #iter_generics (
                    <(#iter_fields) as shipyard::iter::IntoShiperator>::Shiperator
                );

                impl #iter_impl_generics shipyard::iter::IntoShiperator for &'__tmp #r#mut #name #ty_generics #where_clause {
                    type Shiperator = #iter_name #iter_ty_generics;

                    #[inline]
                    #[track_caller]
                    fn into_shiperator(self, storage_ids: &mut shipyard::ShipHashSet<shipyard::StorageId>) -> (Self::Shiperator, usize, shipyard::iter::RawEntityIdAccess) {
                        let (shiperator, end, entities) = (#iter_fields_access).into_shiperator(storage_ids);

                        (#iter_name (shiperator), end, entities)
                    }

                    #[inline]
                    fn can_captain() -> bool {
                        <(#iter_fields) as shipyard::iter::IntoShiperator>::can_captain()
                    }

                    #[inline]
                    fn can_sailor() -> bool {
                        <(#iter_fields) as shipyard::iter::IntoShiperator>::can_sailor()
                    }
                }

                impl #iter_impl_generics shipyard::iter::ShiperatorOutput for #iter_name #iter_ty_generics #iter_where_clause {
                    type Out = #item_name #iter_ty_generics;
                }

                impl #iter_impl_generics shipyard::iter::ShiperatorCaptain for #iter_name #iter_ty_generics #iter_where_clause {
                    #[inline]
                    unsafe fn get_captain_data(&self, index: usize) -> Self::Out {
                        let (#iter_fields_variable) = self.0.get_captain_data(index);

                        #item_name { #iter_fields_variable }
                    }

                    #[inline]
                    fn next_slice(&mut self) {
                        self.0.next_slice()
                    }

                    #[inline]
                    fn sail_time(&self) -> usize {
                        self.0.sail_time()
                    }

                    #[inline]
                    fn is_exact_sized(&self) -> bool {
                        self.0.is_exact_sized()
                    }

                    #[inline]
                    fn unpick(&mut self) {
                        self.0.unpick()
                    }
                }

                impl #iter_impl_generics shipyard::iter::ShiperatorSailor for #iter_name #iter_ty_generics #iter_where_clause {
                    type Index = <<(#iter_fields) as shipyard::iter::IntoShiperator>::Shiperator as shipyard::iter::ShiperatorSailor>::Index;

                    #[inline]
                    unsafe fn get_sailor_data(&self, index: Self::Index) -> Self::Out {
                        let (#iter_fields_variable) = self.0.get_sailor_data(index);

                        #item_name { #iter_fields_variable }
                    }

                    #[inline]
                    fn indices_of(&self, entity_id: shipyard::EntityId, index: usize) -> Option<Self::Index> {
                        self.0.indices_of(entity_id, index)
                    }

                    #[inline]
                    fn index_from_usize(index: usize) -> Self::Index {
                        <<(#iter_fields) as shipyard::iter::IntoShiperator>::Shiperator as shipyard::iter::ShiperatorSailor>::index_from_usize(index)
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

            let mut views = Vec::new();
            let mut views_ty = Vec::new();
            let mut contains_view_mut = false;

            for (field_index, field) in fields.unnamed.iter().enumerate() {
                if let syn::Type::Path(path) = &field.ty {
                    let segment = path.path.segments.last().unwrap();
                    let mut tys = match &segment.arguments {
                        syn::PathArguments::AngleBracketed(args) => args
                            .args
                            .iter()
                            .filter(|arg| matches!(arg, syn::GenericArgument::Type(_))),
                        _ => return Err(Error::new(segment.arguments.span(), "Unexpected syntax")),
                    };

                    let comp_ty = tys
                        .next()
                        .ok_or_else(|| Error::new(segment.span(), "Missing generic"))?;

                    let tracking_ty = tys.next();

                    let index = syn::Index::from(field_index);

                    if segment.ident == "View" {
                        views.push(quote!(&self.#index));
                        views_ty
                            .push(quote!(&'__tmp shipyard::View<'__view, #comp_ty, #tracking_ty>));
                    } else if segment.ident == "ViewMut" {
                        views.push(quote!(&mut self.#index));
                        views_ty.push(
                            quote!(&'__tmp mut shipyard::ViewMut<'__view, #comp_ty, #tracking_ty>),
                        );
                        contains_view_mut = true;
                    } else {
                        return Err(Error::new(field.ty.span(), "Field is not a view"));
                    }
                } else {
                    return Err(Error::new(field.ty.span(), "Field is not a view"));
                }
            }

            let tuple = quote!(
                (#(#views,)*)
            );
            let tuple_ty = quote!(
                (#(#views_ty,)*)
            );

            let r#mut = if contains_view_mut {
                Some(quote!(mut))
            } else {
                None
            };

            Ok(quote!(
                impl #iter_impl_generics shipyard::iter::IntoShiperator for &'__tmp #r#mut #name #ty_generics #where_clause {
                    type Shiperator = <#tuple_ty as shipyard::iter::IntoShiperator>::Shiperator;

                    #[inline]
                    #[track_caller]
                    fn into_shiperator(self, storage_ids: &mut shipyard::ShipHashSet<shipyard::StorageId>) -> (Self::Shiperator, usize, shipyard::iter::RawEntityIdAccess) {
                        #tuple.into_shiperator(storage_ids)
                    }

                    fn can_captain() -> bool {
                        <#tuple_ty as shipyard::iter::IntoShiperator>::can_captain()
                    }

                    fn can_sailor() -> bool {
                        <#tuple_ty as shipyard::iter::IntoShiperator>::can_sailor()
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
