use crate::data::{ContextFreeGrammar, LrZeroAutomaton, MemoFirst, MemoFollow, Rule, RuleP, SlrTable};
use proc_macro2::TokenStream;
use quote::quote;
use std::io::Write;

pub fn derive_parser(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let mut rules: Vec<Rule> = Vec::with_capacity(ast.attrs.len());
    for att in ast.attrs.iter() {
        if att.path().is_ident("rule") {
            rules.push(att.parse_args::<RuleP>()?.into());
        }
    }
    let grammar = ContextFreeGrammar::new(ast, rules.as_slice())?;
    let memo_first = MemoFirst::new(&grammar)?;
    let memo_follow = MemoFollow::new(&grammar, &memo_first)?;
    let lrz_automaton = LrZeroAutomaton::new(&grammar)?;
    eprintln!("FIRST: {memo_first:#?}");
    eprintln!("FOLLOW: {memo_follow:#?}");
    eprintln!("LR(0):");
    lrz_automaton.display_with_grammar(&mut std::io::stderr(), &grammar).unwrap();
    let slr_table = SlrTable::new(&grammar, &memo_follow, lrz_automaton)?;
    eprintln!("SLR Table: {slr_table:#?}");
    let gen = quote! {};
    Ok(gen)
}
