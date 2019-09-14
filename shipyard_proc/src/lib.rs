extern crate proc_macro;
use quote::quote;

const MAX_TYPES: usize = 10;

#[allow(clippy::or_fun_call)]
#[proc_macro_attribute]
pub fn system(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let name = syn::parse_macro_input!(attr as syn::Ident);
    let mut run = syn::parse_macro_input!(item as syn::ItemFn);

    assert!(run.sig.ident == "run", "Systems have only one method: run.");
    assert!(
        run.sig.generics.params.is_empty() && run.sig.generics.where_clause.is_none(),
        "run should not take generic arguments nor where clause."
    );

    // checks if run returns a type other than ()
    match run.sig.output {
        syn::ReturnType::Type(_, type_info) => {
            if let syn::Type::Tuple(tuple) = &*type_info {
                if !tuple.elems.is_empty() {
                    panic!("run should not return anything.")
                }
            } else {
                panic!("run should not return anything.")
            }
        }
        syn::ReturnType::Default => {}
    }

    let body = &*run.block;
    let vis = run.vis;

    let mut data = Vec::with_capacity(run.sig.inputs.len());
    let mut binding = Vec::with_capacity(run.sig.inputs.len());

    run.sig.inputs.iter_mut().for_each(|arg| {
        if let syn::FnArg::Typed(syn::PatType { pat, ty, .. }) = arg {
            match **ty {
                syn::Type::Reference(ref mut reference) => {
                    // references are added a 'a lifetime if they don't have one
                    if let syn::Type::Path(path) = &*reference.elem {
                        // transform &Entities into Entites and &mut Entities into EntitiesMut
                        if path.path.segments.last().unwrap().ident == "Entities" {
                            if reference.mutability.is_none() {
                                **ty = quote!(::shipyard::Entities).into();
                            } else {
                                **ty = quote!(::shipyard::EntitiesMut).into();
                            }
                        } else {
                            reference.lifetime = reference.lifetime.clone().or(Some(syn::Lifetime::new(
                                "'a",
                                proc_macro::Span::call_site().into(),
                            )));
                        }
                    } else {
                        reference.lifetime = reference.lifetime.clone().or(Some(syn::Lifetime::new(
                            "'a",
                            proc_macro::Span::call_site().into(),
                        )));
                    }
                }
                syn::Type::Path(ref mut path) => {
                    let last = path.path.segments.last_mut().unwrap();
                    // Not has to be handled separately since its lifetime is inside the type
                    if last.ident == "Not" {
                        if let syn::PathArguments::AngleBracketed(inner_type) = &mut last.arguments
                        {
                            assert!(inner_type.args.len() == 1, "Not will only accept one type and nothing else.");
                            let arg = inner_type.args.iter_mut().next().unwrap();
                            if let syn::GenericArgument::Type(inner_type) = arg {
                                if let syn::Type::Reference(reference) = inner_type {
                                    reference.lifetime = reference.lifetime.clone().or(Some(syn::Lifetime::new(
                                        "'a",
                                        proc_macro::Span::call_site().into(),
                                    )));
                                } else {
                                    panic!("Not will only work with component storages refered by &T or &mut T.")
                                }
                            } else {
                                unreachable!()
                            }
                        }
                    }
                }
                _ => {
                    panic!(
                        "A system will only accept a type of this list:\n\n\
                            \t\t\t&T for an immutable reference to T storage\n\
                            \t\t\t&mut T for a mutable reference to T storage\n\
                            \t\t\t&Entities for an immutable reference to the entity storage\n\
                            \t\t\t&mut EntitiesMut for a mutable reference to the entity storage\n\
                            \t\t\tAllStorages for a mutable reference to the storage of all components\n\
                            \t\t\tThreadPool for an immutable reference to the rayon::ThreadPool used by the World"
                    );
                }
            }

            data.push((*ty).clone());
            binding.push((**pat).clone());
        } else {
            unreachable!()
        }
    });

    // make tuples MAX_TYPES len maximum to allow users to pass an infinite number of types
    while data.len() > MAX_TYPES {
        for i in 0..(data.len() / MAX_TYPES) {
            let ten = &data[(i * MAX_TYPES)..((i + 1) * MAX_TYPES)];
            *data[i] = quote!((#(#ten,)*)).into();
            data.drain((i + 1)..((i + 1) * MAX_TYPES));

            let ten = &binding[i..(i + 10)];
            binding[i] = quote!((#(#ten,)*)).into();
            binding.drain((i + 1)..((i + 1) * MAX_TYPES));
        }
    }

    (quote! {
        #vis struct #name;
        impl<'a> ::shipyard::System<'a> for #name {
            type Data = (#(#data,)*);
            fn run(&self, (#(#binding,)*): <Self::Data as ::shipyard::SystemData<'a>>::View) #body
        }
    })
    .into()
}
