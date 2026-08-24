extern crate proc_macro;

pub(crate) mod data;
mod expand;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Parser, attributes(rule, gerana))]
pub fn parser(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = parse_macro_input!(input);
    expand::derive_parser(&ast)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Variable, attributes(gerana))]
pub fn variable(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = parse_macro_input!(input);
    expand::derive_variable(&ast)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Terminal, attributes(gerana))]
pub fn terminal(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = parse_macro_input!(input);
    expand::derive_terminal(&ast)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
