extern crate proc_macro;

pub(crate) mod data;
mod expand;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Parser, attributes(rule, lexer, symbol_stack, state_stack, output, error))]
pub fn parser(input: TokenStream) -> TokenStream {
    let ast: DeriveInput = parse_macro_input!(input);
    expand::derive_parser(&ast)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
