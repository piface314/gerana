use crate::data::{
    ContextFreeGrammar, LrZeroAutomaton, MemoFirst, MemoFollow, Rule, RuleP, SlrAction, SlrTable,
    Symbol,
};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::Token;

pub fn derive_parser(ast: &syn::DeriveInput) -> syn::Result<TokenStream> {
    let data = match &ast.data {
        syn::Data::Struct(data) => Ok(data),
        _ => Err(syn::Error::new_spanned(
            ast,
            "expected struct with named fields",
        )),
    }?;
    let fields = match &data.fields {
        syn::Fields::Named(fields) => Ok(fields),
        _ => Err(syn::Error::new_spanned(
            ast,
            "expected struct with named fields",
        )),
    }?;
    let rules: Vec<Rule> = get_rules(ast)?;
    let grammar = ContextFreeGrammar::new(ast, rules.as_slice())?;
    let memo_first = MemoFirst::new(&grammar)?;
    let memo_follow = MemoFollow::new(&grammar, &memo_first)?;
    let lrz_automaton = LrZeroAutomaton::new(&grammar)?;
    let slr_table = SlrTable::new(&grammar, &memo_follow, lrz_automaton)?;

    let ty = &ast.ident;
    let generics = &ast.generics;
    let lt = get_lifetime_param(ast)?;

    let (lexers, token_ty) = get_lexers(fields)?;
    let lexers_init = lexers
        .iter()
        .map(|f| quote!( #f: ::logos::Lexer::new(source) ));
    let current_lexer_init = if lexers.len() > 1 {
        ensure_current_lexer_field(fields)?;
        Some(quote!( current_lexer: 0, ))
    } else {
        None
    };
    let (symbol_stack, symbol_ty) = get_symbol_stack(fields)?;
    let state_stack = get_state_stack(fields)?;

    let output_ty = if let Some(e) = get_declared_param_type(ast, "output")? {
        e.to_token_stream()
    } else {
        quote!(())
    };

    let error_ty = if let Some(e) = get_declared_param_type(ast, "error")? {
        e.to_token_stream()
    } else {
        quote!(::gerana::NoError)
    };

    let lexer_impls = implement_lexer_methods(&lexers, lt);
    let start_var = &grammar.rules[0].head_ident;

    let actions = slr_table.action.iter().map(|((s, t), action)| {
        let terminal = grammar.alphabet.get(t).copied();
        let (terminal_ident, tuple_pat) = if let Some(terminal) = terminal.as_ref() {
            if terminal.binding.is_some() {
                (Some(&terminal.name_ident), Some(quote!((_))))
            } else {
                (Some(&terminal.name_ident), None)
            }
        } else {
            (None, None)
        };
        match action {
            SlrAction::Accept => quote!((#s, None) => break,),
            SlrAction::Shift(s_new) => {
                quote! {
                    (#s, Some(#token_ty::#terminal_ident #tuple_pat)) => self.shift(&mut token, #s_new)?,
                }
            }
            SlrAction::Reduce(rule_index) => {
                let terminal_pat = if terminal.is_some() {
                    quote!(Some(#token_ty::#terminal_ident #tuple_pat))
                } else {
                    quote!(None)
                };
                quote! {
                    (#s, #terminal_pat) => self.reduce(#rule_index)?,
                }
            }
        }
    });

    let errors = error_arms(token_ty, &slr_table);

    let gotos = slr_table.goto.iter().map(|((i, v), j)| {
        let var_ident = grammar.variables[v];
        quote!((#i, #symbol_ty::#var_ident(_)) => #j)
    });

    let reductions = grammar.rules.iter().enumerate().map(|(rule_index, rule)| {
        let bindings = rule.body.iter().rev().map(|symbol| {
            match symbol {
                Symbol::Term(terminal) => {
                    if let Some(binding) = terminal.binding.as_ref() {
                        let terminal_ident = &terminal.name_ident;
                        quote! {
                            let #binding = match #token_ty::try_from(self.#symbol_stack.pop().unwrap()) {
                                Ok(#token_ty::#terminal_ident(x)) => x,
                                _ => unreachable!("invalid token"),
                            };
                        }
                    } else {
                        quote!(self.#symbol_stack.pop();)
                    }
                }
                Symbol::Var(variable) => {
                    let var_ident = &variable.name_ident;
                    let binding = &variable.binding;
                    quote! {
                        let #binding = match self.#symbol_stack.pop().unwrap() {
                            #symbol_ty::#var_ident(x) => x,
                            _ => unreachable!("invalid variable"),
                        };
                    }
                }
            }
        });
        let action = &rule.action;
        let head_ident = &rule.head_ident;
        quote! {
            #rule_index => {
                #(#bindings)*
                let output = #action;
                Ok(#symbol_ty::#head_ident(output))
            }
        }
    });

    Ok(quote! {
        impl #generics ::gerana::Parser<#lt> for #ty #generics {
            type Token = #token_ty;
            type Symbol = #symbol_ty;
            type Output = #output_ty;
            type Error = #error_ty;

            fn new(source: &#lt str) -> Self {
                Self {
                    #symbol_stack: Vec::new(),
                    #state_stack: vec![0],
                    #(#lexers_init),*,
                    #current_lexer_init
                }
            }

            fn parse(mut self) -> Result<Self::Output, ::gerana::ParseError<Self::Error>> {
                let mut token = self.next_token()?;
                while let Some(state) = self.#state_stack.last() {
                    match (state, token.as_ref()) {
                        #(#actions)*
                        #(#errors)*
                        _ => return Err(
                            ::gerana::ParseError::syntax(self.span())
                        ),
                    }
                }
                if let Some(#symbol_ty::#start_var(out)) = self.#symbol_stack.pop() {
                    Ok(out)
                } else {
                    Err(::gerana::ParseError::syntax(self.span()))
                }
            }

            #lexer_impls
        }

        impl #generics #ty #generics {
            fn goto(state: usize, symbol: &#symbol_ty) -> usize {
                match (state, symbol) {
                    #(#gotos),*,
                    _ => unreachable!("invalid state transition {state:?} {symbol:?}"),
                }
            }

            fn shift(&mut self, token: &mut Option<#token_ty>, next_state: usize) -> Result<(), ::gerana::ParseError<#error_ty>> {
                let t = ::std::mem::take(token).unwrap();
                *token = self.next_token()?;
                self.#symbol_stack.push(#symbol_ty::from(t));
                self.#state_stack.push(next_state);
                Ok(())
            }

            fn reduce(&mut self, rule_index: usize) -> Result<(), ::gerana::ParseError<#error_ty>> {
                let symbol = self.synthesize(rule_index)?;
                self.#state_stack.truncate(self.#symbol_stack.len() + 1);
                let state = self.#state_stack.last().unwrap();
                self.#state_stack.push(Self::goto(*state, &symbol));
                self.#symbol_stack.push(symbol);
                Ok(())
            }

            fn synthesize(&mut self, rule_index: usize) -> Result<#symbol_ty, ::gerana::ParseError<#error_ty>> {
                match rule_index {
                    #(#reductions)*
                    _ => unreachable!("invalid reduction"),
                }
            }
        }
    })
}

fn get_rules(ast: &syn::DeriveInput) -> syn::Result<Vec<Rule>> {
    ast.attrs
        .iter()
        .filter_map(|att| {
            if att.path().is_ident("rule") {
                let rule_p: syn::Result<RuleP> = att.parse_args();
                Some(rule_p.map(|r| r.into()))
            } else {
                None
            }
        })
        .collect()
}

fn get_lifetime_param(ast: &syn::DeriveInput) -> syn::Result<&syn::LifetimeParam> {
    ast.generics
        .lifetimes()
        .next()
        .ok_or_else(|| syn::Error::new_spanned(&ast.generics, "expected a lifetime parameter"))
}

fn get_lexers<'a>(
    fields: &'a syn::FieldsNamed,
) -> syn::Result<(Vec<&'a syn::Ident>, &'a syn::TypePath)> {
    let mut lexers = Vec::new();
    let mut token_ty = None;
    for f in fields.named.iter() {
        let f_ident = f.ident.as_ref().expect("struct with named field");
        if f_ident != "lexer" && !is_tagged_as(f, "lexer")? {
            continue;
        }
        if lexers.is_empty() {
            token_ty = Some(
                get_type_param_from_field(f, 1)
                    .ok_or_else(|| syn::Error::new_spanned(f, "cannot determine Token type"))?,
            );
        }
        lexers.push(f_ident);
    }
    if lexers.is_empty() {
        Err(syn::Error::new_spanned(fields, "a lexer must be specified by either naming a field as `lexer` or using `#[gerana(lexer)]`"))
    } else {
        Ok((lexers, token_ty.expect("previously checked")))
    }
}

fn iter_attrs<'a>(attrs: &'a Vec<syn::Attribute>) -> impl Iterator<Item = &'a syn::Attribute> {
    attrs.iter().filter(|att| att.path().is_ident("gerana"))
}

fn is_tagged_as(f: &syn::Field, tag: &'static str) -> syn::Result<bool> {
    for att in iter_attrs(&f.attrs) {
        if att
            .parse_args::<syn::Ident>()
            .map(|ident| ident == tag)
            .unwrap_or(false)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ensure_current_lexer_field<'a>(fields: &'a syn::FieldsNamed) -> syn::Result<()> {
    if fields
        .named
        .iter()
        .any(|f| f.ident.as_ref().expect("struct with named field") == "current_lexer")
    {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            fields,
            "when more than one lexer is used, struct must contain a `current_lexer: usize` field",
        ))
    }
}

fn get_symbol_stack<'a>(
    fields: &'a syn::FieldsNamed,
) -> syn::Result<(&'a syn::Ident, &'a syn::TypePath)> {
    let results: Vec<_> = fields
        .named
        .iter()
        .filter_map(|f| {
            let f_ident = f.ident.as_ref().expect("struct with named field");
            let is_stack = f_ident == "symbol_stack"
                || match is_tagged_as(f, "symbol_stack") {
                    Ok(b) => b,
                    Err(e) => return Some(Err(e)),
                };
            if is_stack {
                let ty = get_type_param_from_field(f, 0)?;
                Some(Ok((f_ident, ty)))
            } else {
                None
            }
        })
        .collect::<Result<_, _>>()?;
    if results.len() == 1 {
        Ok(results[0])
    } else if results.is_empty() {
        Err(syn::Error::new_spanned(
            fields,
            "a single symbol_stack must be defined",
        ))
    } else {
        Err(syn::Error::new_spanned(
            results[1].0,
            "a single symbol_stack must be defined",
        ))
    }
}

fn get_state_stack<'a>(fields: &'a syn::FieldsNamed) -> syn::Result<&'a syn::Ident> {
    let results: Vec<_> = fields
        .named
        .iter()
        .filter_map(|f| {
            let f_ident = f.ident.as_ref().expect("struct with named field");
            let is_stack = f_ident == "state_stack"
                || match is_tagged_as(f, "state_stack") {
                    Ok(b) => b,
                    Err(e) => return Some(Err(e)),
                };
            if is_stack {
                Some(Ok(f_ident))
            } else {
                None
            }
        })
        .collect::<Result<_, _>>()?;
    if results.len() == 1 {
        Ok(results[0])
    } else if results.is_empty() {
        Err(syn::Error::new_spanned(
            fields,
            "a single state_stack must be defined",
        ))
    } else {
        Err(syn::Error::new_spanned(
            results[1],
            "a single state_stack must be defined",
        ))
    }
}

struct NamedParam {
    name: syn::Ident,
    #[allow(unused)]
    eq_token: Token![=],
    ty: syn::TypePath,
}

impl syn::parse::Parse for NamedParam {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let eq_token = input.parse()?;
        let ty = input.parse()?;
        Ok(Self { name, eq_token, ty })
    }
}

fn get_declared_param_type(
    ast: &syn::DeriveInput,
    param: &str,
) -> syn::Result<Option<syn::TypePath>> {
    for att in iter_attrs(&ast.attrs) {
        let np = att.parse_args::<NamedParam>()?;
        if np.name == param {
            return Ok(Some(np.ty));
        }
    }
    Ok(None)
}

fn get_type_param_from_field<'a>(f: &'a syn::Field, arg_index: usize) -> Option<&'a syn::TypePath> {
    let ty_path = match &f.ty {
        syn::Type::Path(ty_path) => Some(ty_path),
        _ => None,
    }?;
    let last_segment = if let Some(seg) = ty_path.path.segments.last() {
        Some(seg)
    } else {
        None
    }?;
    let args = match &last_segment.arguments {
        syn::PathArguments::AngleBracketed(args) => Some(&args.args),
        _ => None,
    }?;
    if let Some(syn::GenericArgument::Type(syn::Type::Path(ty))) = args.get(arg_index) {
        Some(ty)
    } else {
        None
    }
}

fn implement_lexer_methods<'a>(
    lexers: &[&'a syn::Ident],
    lt: &'a syn::LifetimeParam,
) -> TokenStream {
    if lexers.len() == 1 {
        let lexer = lexers[0];
        quote! {
            fn set_lexer(&mut self, i: usize) {
                if i > 0 {
                    unreachable!("parser has only 1 lexer")
                }
            }

            fn next_token(&mut self) -> Result<Option<Self::Token>, ::gerana::ParseError<Self::Error>> {
                self
                .#lexer
                .next()
                .map(|r| r.map_err(|_| ::gerana::ParseError::scan(self.span())))
                .transpose()
            }

            fn slice(&self) -> &#lt str {
                self.#lexer.slice()
            }

            fn span(&self) -> std::ops::Range<usize> {
                self.#lexer.span()
            }

        }
    } else {
        let mut set_lexer_arms = Vec::new();
        for (i, cur_lex) in lexers.iter().enumerate() {
            for (j, set_lex) in lexers.iter().enumerate() {
                if i == j {
                    continue;
                }
                let arm = quote! {
                (#i, #j) => self
                    .#set_lex
                    .bump(self.#cur_lex.span().end - self.#set_lex.span().end)
                };
                set_lexer_arms.push(arm);
            }
        }
        let next_token_lexer_arms = lexers.iter().enumerate().map(|(i, lexer)| {
            if i == 0 {
                quote! {
                    #i => self
                        .#lexer
                        .next()
                        .map(|r| r.map_err(|_| ::gerana::ParseError::scan(self.#lexer.span())))
                }
            } else {
                quote! {
                    #i => self
                        .#lexer
                        .next()
                        .map(|r|
                            r.map_err(|_| ::gerana::ParseError::scan(self.#lexer.span()))
                            .map(|t| t.into())
                        )
                }
            }
        });
        let slice_lexer_arms = lexers
            .iter()
            .enumerate()
            .map(|(i, lexer)| quote!( #i => self.#lexer.slice() ));
        let span_lexer_arms = lexers
            .iter()
            .enumerate()
            .map(|(i, lexer)| quote!( #i => self.#lexer.span() ));
        quote! {
            fn set_lexer(&mut self, i: usize) {
                if self.current_lexer == i {
                    return;
                }
                match (self.current_lexer, i) {
                    #(#set_lexer_arms),*,
                    _ => unreachable!("parser has no {i}-th lexer"),
                };
                self.current_lexer = i;
            }

            fn next_token(&mut self) -> Result<Option<Self::Token>, ::gerana::ParseError<Self::Error>> {
                let output = match self.current_lexer {
                    #(#next_token_lexer_arms),*,
                    _ => unreachable!("parser has no {}-th lexer", self.current_lexer),
                };
                output.transpose()
            }

            fn slice(&self) -> &#lt str {
                match self.current_lexer {
                    #(#slice_lexer_arms),*,
                    _ => unreachable!("parser has no {}-th lexer", self.current_lexer),
                }
            }

            fn span(&self) -> std::ops::Range<usize> {
                match self.current_lexer {
                    #(#span_lexer_arms),*,
                    _ => unreachable!("parser has no {}-th lexer", self.current_lexer),
                }
            }
        }
    }
}

fn error_arms<'r>(
    token_ty: &'r syn::TypePath,
    slr_table: &'r SlrTable<'r>,
) -> impl Iterator<Item = TokenStream> + 'r {
    slr_table.expected_inputs().map(move |(s, inputs)| {
        let expected = inputs.iter().copied().map(|input| {
            if input == "$" {
                quote!("end of input")
            } else {
                quote!(#token_ty::describe(#input))
            }
        });
        quote! {
            (#s, _) => {
                return Err(::gerana::ParseError::syntax_expecting([#(#expected),*], self.span()))
            }
        }
    })
}
