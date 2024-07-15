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
        block,
    } = input_fn;

    let mangled_name = syn::Ident::new(&format!("_tla_impl_{}", sig.ident), sig.ident.span());
    modified_fn.sig.ident = mangled_name.clone();

    let asyncness = sig.asyncness;

    let output = if asyncness.is_some() {
        quote! {
            #modified_fn
            #(#attrs)* #vis #sig {
                // Fail the compilation if we're not in debug mode
                #[cfg(not(debug_assertions))]
                let i:u32 = "abc";

                async fn body() {
                    #block
                }
                tla_instrumentation::tla_log_method_call!(#attr2);
                let res = body().await;
                tla_instrumentation::tla_log_method_return!();
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

                let body = || {
                    #block
                };
                tla_instrumentation::tla_log_method_call!(#attr2);
                let res = body();
                tla_instrumentation::tla_log_method_return!();
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
                tla_instrumentation::log_fn_call!(state, #arg, crate::tla::get_tla_globals);
            });
            #block
            crate::tla::with_tla_state(|state| {
                tla_instrumentation::log_fn_return!(state);
            });
        }
    };

    output.into()
}
