use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Expr, ItemFn};

#[proc_macro_attribute]
pub fn tla_update(attr: TokenStream, item: TokenStream) -> TokenStream {
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

    let output = quote! {
        #(#attrs)* #vis #sig {
            // Fail the compilation if we're not in debug mode
            #[cfg(not(debug_assertions))]
            let i:u32 = "abc";
            with_tla_state(|state| {
                tla_instrumentation::log_method_call!(state, #arg, crate::tla::get_tla_globals);
            });
            #block
            with_tla_state(|state| {
                tla_instrumentation::log_method_return!(state);
            });
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
