extern crate proc_macro;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_quote, Error, Ident, Result, Type};

const MAX_TYPES: usize = 10;

#[allow(clippy::or_fun_call)]
#[proc_macro_attribute]
pub fn system(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let name = syn::parse_macro_input!(attr as syn::Ident);
    let run = syn::parse_macro_input!(item as syn::ItemFn);
    expand_system(name, run)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

#[allow(clippy::cognitive_complexity)]
fn expand_system(name: syn::Ident, mut run: syn::ItemFn) -> Result<TokenStream> {
    if run.sig.ident != "run" {
        return Err(Error::new(
            Span::call_site(),
            "Systems have only one method: run",
        ));
    }
    if !run.sig.generics.params.is_empty() {
        return Err(Error::new_spanned(
            run.sig.generics,
            "run should not take generic arguments",
        ));
    }
    if run.sig.generics.where_clause.is_some() {
        return Err(Error::new_spanned(
            run.sig.generics.where_clause,
            "run should not take a where clause",
        ));
    }

    // checks if run returns a type other than ()
    let returns_something = match run.sig.output {
        syn::ReturnType::Type(_, ref type_info) => match **type_info {
            syn::Type::Tuple(ref tuple) => !tuple.elems.is_empty(),
            _ => true,
        },
        syn::ReturnType::Default => false,
    };
    if returns_something {
        return Err(Error::new_spanned(
            run.sig.output,
            "run should not return anything",
        ));
    }

    let body = &*run.block;
    let vis = run.vis;

    let mut data = Vec::with_capacity(run.sig.inputs.len());
    let mut binding = Vec::with_capacity(run.sig.inputs.len());

    let mut conflict: Conflict = Conflict::None;
    let mut shared_borrows: Vec<(usize, Type)> = Vec::with_capacity(run.sig.inputs.len());
    let mut exclusive_borrows: Vec<(usize, Type)> = Vec::with_capacity(run.sig.inputs.len());

    // have to make a copy in case we have to refer to modified types in errors
    let run_clone = run.sig.inputs.clone();
    run.sig.inputs.iter_mut().enumerate().try_for_each(|(i, arg)| {
        if let syn::FnArg::Typed(syn::PatType { pat, ty, .. }) = arg {
            match **ty {
                syn::Type::Reference(ref mut reference) => {
                    // references are added a 'sys lifetime if they don't have one
                    // if they have another lifetime, make it 'sys
                    if let syn::Type::Path(path) = &*reference.elem {
                        // transform &Entities into Entites and &mut Entities into EntitiesMut
                        if path.path.segments.last().unwrap().ident == "Entities" {
                            if reference.mutability.is_some() {
                                **ty = parse_quote!(::shipyard::EntitiesMut);
                                exclusive_borrows.push((i, parse_quote!(Entities)));
                            } else {
                                **ty = parse_quote!(::shipyard::Entities);
                                shared_borrows.push((i, parse_quote!(Entities)));
                            }

                            match &mut conflict {
                                Conflict::AllStorages(all_storages) => {
                                    conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                    return Ok(());
                                },
                                Conflict::LastStorage(last_storage) => {
                                    *last_storage = i;
                                },
                                Conflict::None => conflict = Conflict::LastStorage(i),
                                _ => {}
                            }
                        } else if path.path.segments.last().unwrap().ident == "AllStorages" {
                            if reference.mutability.is_none() {
                                return Err(Error::new_spanned(
                                    path,
                                    "You probably forgot a mut, &AllStorages isn't a valid storage access"
                                ));
                            } else {
                                **ty = parse_quote!(::shipyard::AllStorages);

                                match &mut conflict {
                                    Conflict::AllStorages(all_storages) => {
                                        conflict = Conflict::DoubleAllStorages([*all_storages, i]);
                                        return Ok(());
                                    },
                                    Conflict::LastStorage(storage) => {
                                        conflict = Conflict::StoragePlusAllStorages([*storage, i]);
                                        return Ok(());
                                    }
                                    Conflict::None => conflict = Conflict::AllStorages(i),
                                    _ => {}
                                }
                            }
                        } else if path.path.segments.last().unwrap().ident == "ThreadPool" {
                            if reference.mutability.is_none() {
                                **ty = parse_quote!(::shipyard::ThreadPool);
                            } else {
                                return Err(Error::new_spanned(
                                    path,
                                    "ThreadPool can't be accessed mutably but there's no need to, it's Sync and works perfectly with a shared access"
                                ));
                            }
                        } else {
                            reference.lifetime = parse_quote!('sys);

                            if reference.mutability.is_some() {
                                let mut ty_clone = reference.clone();
                                ty_clone.mutability = None;
                                exclusive_borrows.push((i, ty_clone.into()));
                            } else {
                                shared_borrows.push((i, (**ty).clone()));
                            }

                            match &mut conflict {
                                Conflict::AllStorages(all_storages) => {
                                    conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                    return Ok(());
                                },
                                Conflict::LastStorage(last_storage) => *last_storage = i,
                                Conflict::None => conflict = Conflict::LastStorage(i),
                                _ => {}
                            }
                        }
                    } else {
                        reference.lifetime = parse_quote!('sys);

                        if reference.mutability.is_some() {
                            let mut ty_clone = reference.clone();
                            ty_clone.mutability = None;
                            exclusive_borrows.push((i, ty_clone.into()));
                        } else {
                            shared_borrows.push((i, (**ty).clone()));
                        }
                        match &mut conflict {
                            Conflict::AllStorages(all_storages) => {
                                conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                return Ok(());
                            },
                            Conflict::LastStorage(last_storage) => *last_storage = i,
                            Conflict::None => conflict = Conflict::LastStorage(i),
                            _ => {}
                        }
                    }
                }
                syn::Type::Path(ref mut path) => {
                    let path_clone = path.clone();
                    let last = path.path.segments.last_mut().unwrap();
                    // Unique has to be handled separatly because the lifetime is inside it
                    if last.ident == "Unique" {
                        if let syn::PathArguments::AngleBracketed(inner_type) = &mut last.arguments
                        {
                            let inner_arg = inner_type.args.iter_mut().next().unwrap();
                            if let syn::GenericArgument::Type(inner_type) = inner_arg {
                                match inner_type {
                                    syn::Type::Reference(reference) => {
                                        reference.lifetime = parse_quote!('sys);

                                        if reference.mutability.is_some() {
                                            let mut ty_clone = reference.clone();
                                            ty_clone.mutability = None;
                                            exclusive_borrows.push((i, ty_clone.into()));
                                        } else {
                                            shared_borrows.push((i, inner_type.clone()));
                                        }

                                        match &mut conflict {
                                            Conflict::AllStorages(all_storages) => {
                                                conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                                return Ok(());
                                            },
                                            Conflict::LastStorage(last_storage) => *last_storage = i,
                                            Conflict::None => conflict = Conflict::LastStorage(i),
                                            _ => {}
                                        }
                                    },
                                    syn::Type::Path(ref mut path) => {
                                        let last = path.path.segments.last_mut().unwrap();
                                        if last.ident == "NonSend" {
                                            if let syn::PathArguments::AngleBracketed(inner_type) = &mut last.arguments
                                            {
                                                let inner_arg = inner_type.args.iter_mut().next().unwrap();
                                                if let syn::GenericArgument::Type(inner_type) = inner_arg {
                                                    if let syn::Type::Reference(reference) = inner_type {
                                                        reference.lifetime = parse_quote!('sys);

                                                        if reference.mutability.is_some() {
                                                            let mut ty_clone = reference.clone();
                                                            ty_clone.mutability = None;
                                                            exclusive_borrows.push((i, ty_clone.into()));
                                                        } else {
                                                            shared_borrows.push((i, inner_type.clone()));
                                                        }

                                                        match &mut conflict {
                                                            Conflict::AllStorages(all_storages) => {
                                                                conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                                                return Ok(());
                                                            },
                                                            Conflict::LastStorage(last_storage) => *last_storage = i,
                                                            Conflict::None => conflict = Conflict::LastStorage(i),
                                                            _ => {}
                                                        }
                                                    } else {
                                                        return Err(Error::new_spanned(
                                                            inner_type,
                                                            "NonSend will only work with component storages referred by &T or &mut T",
                                                        ));
                                                    }
                                                } else {
                                                    unreachable!()
                                                }
                                            }
                                        }
                                        else if last.ident == "NonSync" {
                                            if let syn::PathArguments::AngleBracketed(inner_type) = &mut last.arguments
                                            {
                                                let inner_arg = inner_type.args.iter_mut().next().unwrap();
                                                if let syn::GenericArgument::Type(inner_type) = inner_arg {
                                                    if let syn::Type::Reference(reference) = inner_type {
                                                        reference.lifetime = parse_quote!('sys);

                                                        if reference.mutability.is_some() {
                                                            let mut ty_clone = reference.clone();
                                                            ty_clone.mutability = None;
                                                            exclusive_borrows.push((i, ty_clone.into()));
                                                        } else {
                                                            shared_borrows.push((i, inner_type.clone()));
                                                        }

                                                        match &mut conflict {
                                                            Conflict::AllStorages(all_storages) => {
                                                                conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                                                return Ok(());
                                                            },
                                                            Conflict::LastStorage(last_storage) => *last_storage = i,
                                                            Conflict::None => conflict = Conflict::LastStorage(i),
                                                            _ => {}
                                                        }
                                                    } else {
                                                        return Err(Error::new_spanned(
                                                            inner_type,
                                                            "NonSync will only work with component storages referred by &T or &mut T",
                                                        ));
                                                    }
                                                } else {
                                                    unreachable!()
                                                }
                                            }
                                        }
                                        else if last.ident == "NonSendSync" {
                                            if let syn::PathArguments::AngleBracketed(inner_type) = &mut last.arguments
                                            {
                                                let inner_arg = inner_type.args.iter_mut().next().unwrap();
                                                if let syn::GenericArgument::Type(inner_type) = inner_arg {
                                                    if let syn::Type::Reference(reference) = inner_type {
                                                        reference.lifetime = parse_quote!('sys);

                                                        if reference.mutability.is_some() {
                                                            let mut ty_clone = reference.clone();
                                                            ty_clone.mutability = None;
                                                            exclusive_borrows.push((i, ty_clone.into()));
                                                        } else {
                                                            shared_borrows.push((i, inner_type.clone()));
                                                        }

                                                        match &mut conflict {
                                                            Conflict::AllStorages(all_storages) => {
                                                                conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                                                return Ok(());
                                                            },
                                                            Conflict::LastStorage(last_storage) => *last_storage = i,
                                                            Conflict::None => conflict = Conflict::LastStorage(i),
                                                            _ => {}
                                                        }
                                                    } else {
                                                        return Err(Error::new_spanned(
                                                            inner_type,
                                                            "NonSendSync will only work with component storages referred by &T or &mut T",
                                                        ));
                                                    }
                                                } else {
                                                    unreachable!()
                                                }
                                            }
                                        } else {
                                            return Err(Error::new_spanned(
                                                inner_type,
                                                "Unique will only work with component storages referred by &T or &mut T",
                                            ));
                                        }
                                    },
                                    _ => {
                                        return Err(Error::new_spanned(
                                            inner_type,
                                            "Unique will only work with component storages referred by &T or &mut T",
                                        ));
                                    }
                                }
                            } else {
                                unreachable!()
                            }
                        }
                    }
                    else if last.ident == "NonSend" {
                        if let syn::PathArguments::AngleBracketed(inner_type) = &mut last.arguments
                        {
                            let inner_arg = inner_type.args.iter_mut().next().unwrap();
                            if let syn::GenericArgument::Type(inner_type) = inner_arg {
                                if let syn::Type::Reference(reference) = inner_type {
                                    reference.lifetime = parse_quote!('sys);

                                    if reference.mutability.is_some() {
                                        let mut ty_clone = reference.clone();
                                        ty_clone.mutability = None;
                                        exclusive_borrows.push((i, ty_clone.into()));
                                    } else {
                                        shared_borrows.push((i, inner_type.clone()));
                                    }

                                    match &mut conflict {
                                        Conflict::AllStorages(all_storages) => {
                                            conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                            return Ok(());
                                        },
                                        Conflict::LastStorage(last_storage) => *last_storage = i,
                                        Conflict::None => conflict = Conflict::LastStorage(i),
                                        _ => {}
                                    }
                                } else {
                                    return Err(Error::new_spanned(
                                        inner_type,
                                        "NonSend will only work with component storages referred by &T or &mut T",
                                    ));
                                }
                            } else {
                                unreachable!()
                            }
                        }
                    }
                    else if last.ident == "NonSync" {
                        if let syn::PathArguments::AngleBracketed(inner_type) = &mut last.arguments
                        {
                            let inner_arg = inner_type.args.iter_mut().next().unwrap();
                            if let syn::GenericArgument::Type(inner_type) = inner_arg {
                                if let syn::Type::Reference(reference) = inner_type {
                                    reference.lifetime = parse_quote!('sys);

                                    if reference.mutability.is_some() {
                                        let mut ty_clone = reference.clone();
                                        ty_clone.mutability = None;
                                        exclusive_borrows.push((i, ty_clone.into()));
                                    } else {
                                        shared_borrows.push((i, inner_type.clone()));
                                    }

                                    match &mut conflict {
                                        Conflict::AllStorages(all_storages) => {
                                            conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                            return Ok(());
                                        },
                                        Conflict::LastStorage(last_storage) => *last_storage = i,
                                        Conflict::None => conflict = Conflict::LastStorage(i),
                                        _ => {}
                                    }
                                } else {
                                    return Err(Error::new_spanned(
                                        inner_type,
                                        "NonSync will only work with component storages referred by &T or &mut T",
                                    ));
                                }
                            } else {
                                unreachable!()
                            }
                        }
                    }
                    else if last.ident == "NonSendSync" {
                        if let syn::PathArguments::AngleBracketed(inner_type) = &mut last.arguments
                        {
                            let inner_arg = inner_type.args.iter_mut().next().unwrap();
                            if let syn::GenericArgument::Type(inner_type) = inner_arg {
                                if let syn::Type::Reference(reference) = inner_type {
                                    reference.lifetime = parse_quote!('sys);

                                    if reference.mutability.is_some() {
                                        let mut ty_clone = reference.clone();
                                        ty_clone.mutability = None;
                                        exclusive_borrows.push((i, ty_clone.into()));
                                    } else {
                                        shared_borrows.push((i, inner_type.clone()));
                                    }

                                    match &mut conflict {
                                        Conflict::AllStorages(all_storages) => {
                                            conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                            return Ok(());
                                        },
                                        Conflict::LastStorage(last_storage) => *last_storage = i,
                                        Conflict::None => conflict = Conflict::LastStorage(i),
                                        _ => {}
                                    }
                                } else {
                                    return Err(Error::new_spanned(
                                        inner_type,
                                        "NonSendSync will only work with component storages referred by &T or &mut T",
                                    ));
                                }
                            } else {
                                unreachable!()
                            }
                        }
                    } else if last.ident == "Entities" {
                        shared_borrows.push((i, (**ty).clone()));

                        match &mut conflict {
                            Conflict::AllStorages(all_storages) => {
                                conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                return Ok(());
                            },
                            Conflict::LastStorage(last_storage) => *last_storage = i,
                            Conflict::None => conflict = Conflict::LastStorage(i),
                            _ => {}
                        }
                    } else if last.ident == "EntitiesMut" {
                        let mut ty_clone = path_clone.clone();
                        ty_clone.path = Ident::new("Entities", last.ident.span()).into();
                        exclusive_borrows.push((i, ty_clone.into()));

                        match &mut conflict {
                            Conflict::AllStorages(all_storages) => {
                                conflict = Conflict::AllStoragesPlusStorage([*all_storages, i]);
                                return Ok(());
                            },
                            Conflict::LastStorage(last_storage) => *last_storage = i,
                            Conflict::None => conflict = Conflict::LastStorage(i),
                            _ => {}
                        }
                    } else if last.ident == "AllStorages" {
                        match &mut conflict {
                            Conflict::AllStorages(all_storages) => {
                                conflict = Conflict::DoubleAllStorages([*all_storages, i]);
                                return Ok(());
                            },
                            Conflict::LastStorage(storage) => {
                                conflict = Conflict::StoragePlusAllStorages([*storage, i]);
                                return Ok(());
                            }
                            Conflict::None => conflict = Conflict::AllStorages(i),
                            _ => {}
                        }
                    }
                }
                _ => {
                    return Err(Error::new_spanned(
                        ty,
                        "A system will only accept a type of this list:\n\
                            \t\t- &T for an immutable reference to T storage\n\
                            \t\t- &mut T for a mutable reference to T storage\n\
                            \t\t- &Entities for an immutable reference to the entity storage\n\
                            \t\t- &mut EntitiesMut for a mutable reference to the entity storage\n\
                            \t\t- &mut AllStorages for a mutable reference to the storage of all components\n\
                            \t\t- &ThreadPool for an immutable reference to the rayon::ThreadPool used by the World",
                    ));
                }
            }

            data.push(ty.clone());
            binding.push((**pat).clone());
            Ok(())
        } else {
            unreachable!()
        }
    })?;

    match conflict {
        Conflict::AllStoragesPlusStorage([first, second])
        | Conflict::StoragePlusAllStorages([first, second]) => {
            let first = &run_clone[first];
            let second = &run_clone[second];
            return Err(Error::new_spanned(quote!(#first, #second), "Cannot borrow AllStorages and a storage at the same time, this includes entities.\n       You can borrow the storage from AllStorages inside the system instead."));
        }
        Conflict::DoubleAllStorages([first, second]) => {
            let first = &run.sig.inputs[first];
            let second = &run.sig.inputs[second];
            return Err(Error::new_spanned(
                quote!(#first, #second),
                "Cannot borrow AllStorages twice.",
            ));
        }
        _ => {}
    }

    for (i, &(index, ref type_name)) in exclusive_borrows.iter().enumerate() {
        for &(index2, ref type_name2) in exclusive_borrows[i..].iter().skip(1) {
            if type_name == type_name2 {
                let (first, second) = (&run_clone[index], &run_clone[index2]);

                return Err(Error::new_spanned(
                    quote!(#first, #second),
                    "Cannot borrow the same storage exclusively twice.",
                ));
            }
        }

        for &(index2, ref type_name2) in &shared_borrows {
            if type_name == type_name2 {
                let (first, second) = if index < index2 {
                    (&run_clone[index], &run_clone[index2])
                } else {
                    (&run_clone[index2], &run_clone[index])
                };

                return Err(Error::new_spanned(
                    quote!(#first, #second),
                    "Cannot borrow again a storage already borrowed exclusively, you may want to remove the shared borrow.",
                ));
            }
        }
    }

    // make tuples MAX_TYPES len maximum to allow users to pass an infinite number of types
    while data.len() > MAX_TYPES {
        for i in 0..(data.len() / MAX_TYPES) {
            let ten = &data[(i * MAX_TYPES)..((i + 1) * MAX_TYPES)];
            data[i] = parse_quote!((#(#ten,)*));
            data.drain((i + 1)..((i + 1) * MAX_TYPES));

            let ten = &binding[i..(i + 10)];
            binding[i] = parse_quote!((#(#ten,)*));
            binding.drain((i + 1)..((i + 1) * MAX_TYPES));
        }
    }

    Ok(quote! {
        #vis struct #name;
        impl<'sys> ::shipyard::System<'sys> for #name {
            type Data = (#(#data,)*);
            fn run((#(#binding,)*): (#(<#data as ::shipyard::SystemData<'sys>>::View,)*)) #body
        }
    })
}

enum Conflict {
    LastStorage(usize),
    AllStorages(usize),
    DoubleAllStorages([usize; 2]),
    AllStoragesPlusStorage([usize; 2]),
    StoragePlusAllStorages([usize; 2]),
    None,
}
