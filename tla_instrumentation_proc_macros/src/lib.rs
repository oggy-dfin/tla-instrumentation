use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Expr, ItemFn};

/// Used to annotate top-level methods (which de-facto start an update call)
#[proc_macro_attribute]
pub fn tla_update(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input tokens of the attribute and the function
    let input_fn = parse_macro_input!(item as ItemFn);
    // let arg = parse_macro_input!(attr as Expr);
    // Convert proc_macro::TokenStream to proc_macro2::TokenStream
    let attr2: TokenStream2 = attr.into();

    let mut modified_fn = input_fn.clone();

    // Deconstruct the function elements
    let ItemFn {
        attrs,
        vis,
        sig,
        block: _,
    } = input_fn;

    let mangled_name = syn::Ident::new(&format!("_tla_impl_{}", sig.ident), sig.ident.span());
    modified_fn.sig.ident = mangled_name.clone();

    // Creating the modified original function which calls f_impl
    let args: Vec<_> = sig
        .inputs
        .iter()
        .map(|arg| match arg {
            syn::FnArg::Typed(pat_type) => &*pat_type.pat,
            _ => panic!("Expected typed arguments in function signature."),
        })
        .collect();

    let asyncness = sig.asyncness;

    let output = if asyncness.is_some() {
        quote! {
            #modified_fn

            #(#attrs)* #vis #sig {
                // Fail the compilation if we're not in debug mode
                #[cfg(not(debug_assertions))]
                let i:u32 = "abc";

                let globals = tla_get_globals!();
                tla_instrumentation::tla_log_method_call!(#attr2, globals);
                let res = #mangled_name(#(#args),*).await;
                let globals = tla_get_globals!();
                tla_instrumentation::tla_log_method_return!(globals);
                res
            }
        }
    } else {
        quote! {
            #modified_fn

            #(#attrs)* #vis #sig {
                // Fail the compilation if we're not in debug mode
                #[cfg(not(debug_assertions))]
                let i:u32 = "abc";

                let globals = tla_get_globals!();
                tla_instrumentation::tla_log_method_call!(#attr2, globals);
                let res = #mangled_name(#(#args),*);
                let globals = tla_get_globals!();
                tla_instrumentation::tla_log_method_return!(globals);
                res
            }
        }
    };

    output.into()
}

#[proc_macro_attribute]
pub fn tla_update_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input tokens of the attribute and the function
    let input_fn = parse_macro_input!(item as ItemFn);
    // let arg = parse_macro_input!(attr as Expr);
    // Convert proc_macro::TokenStream to proc_macro2::TokenStream
    let attr2: TokenStream2 = attr.into();

    let mut modified_fn = input_fn.clone();

    // Deconstruct the function elements
    let ItemFn {
        attrs,
        vis,
        sig,
        block: _,
    } = input_fn;

    let mangled_name = syn::Ident::new(&format!("_tla_impl_{}", sig.ident), sig.ident.span());
    modified_fn.sig.ident = mangled_name.clone();

    // Creating the modified original function which calls f_impl
    let args: Vec<_> = sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Receiver(_) => None,
            syn::FnArg::Typed(pat_type) => Some(&*pat_type.pat),
        })
        .collect();

    let asyncness = sig.asyncness;

    let output = if asyncness.is_some() {
        quote! {
            #modified_fn

            #(#attrs)* #vis #sig {
                // Fail the compilation if we're not in debug mode
                #[cfg(not(debug_assertions))]
                let i:u32 = "abc";

                use std::cell::RefCell;
                use std::rc::Rc;

                let globals = tla_get_globals!(self);
                let raw_ptr = self as *const _;
                let snapshotter = Rc::new(move || { unsafe { tla_get_globals!(&*raw_ptr) } });
                let mut pinned = Box::pin(TLA_INSTRUMENTATION_STATE.scope(
                    tla_instrumentation::InstrumentationState::new(#attr2, globals, snapshotter),
                    async move {
                        let res = self.#mangled_name(#(#args),*).await;
                        let globals = tla_get_globals!(self);
                        let state: InstrumentationState = TLA_INSTRUMENTATION_STATE.get();
                        let mut handler_state = state.handler_state.borrow_mut();
                        let state_pair = tla_instrumentation::log_method_return(&mut handler_state, globals);
                        let mut state_pairs = state.state_pairs.borrow_mut();
                        state_pairs.push(state_pair);
                        res
                    }
                ));
                let res = pinned.as_mut().await;
                let trace = pinned.as_mut().take_value().expect("No TLA trace in the future!");
                let pairs = trace.state_pairs.borrow().clone();
                println!("State pairs in the expanded macro: {:?}", pairs);
                let mut traces = TLA_TRACES.write().unwrap();
                traces.extend(pairs);
                res
            }
        }
    } else {
        quote! {
            #modified_fn

            #(#attrs)* #vis #sig {
                // Fail the compilation if we're not in debug mode
                #[cfg(not(debug_assertions))]
                let i:u32 = "abc";

                let globals = tla_get_globals!();
                tla_instrumentation::tla_log_method_call!(#attr2, globals);
                let res = #mangled_name(#(#args),*);
                let globals = tla_get_globals!();
                tla_instrumentation::tla_log_method_return!(globals);
                res
            }
        }
    };

    output.into()
}

#[proc_macro_attribute]
pub fn tla_function(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input tokens of the attribute and the function
    let input_fn = parse_macro_input!(item as ItemFn);
    let arg = parse_macro_input!(attr as Expr);

    // Deconstruct the function elements
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = input_fn;

    // Generate the new function with the macro call inserted at the beginning
    let output = quote! {
        #(#attrs)* #vis #sig {
            // Fail the compilation if we're not in debug mode
            #[cfg(not(debug_assertions))]
            let i:u32 = "abc";
            crate::tla::with_tla_state(|state| {
                tla_instrumentation::log_fn_call!(state, #arg, crate::tla::tla_get_globals);
            });
            #block
            crate::tla::with_tla_state(|state| {
                tla_instrumentation::log_fn_return!(state);
            });
        }
    };

    output.into()
}
