use core::ops::Range;
use logos::Logos;
use std::error::Error;
use thiserror::Error;

/// Error type returned by the parser.
///
/// Besides the two default error variants, the `Other` variant is specific to the
/// generated parser.
#[derive(Debug, Clone, Error)]
pub enum ParseError<E> {
    /// A scan error happens when the lexer encounters invalid input in the source.
    #[error("invalid input at {at:?}")]
    Scan { at: Range<usize> },
    /// A syntax error happens when the parser encounters no valid action for an input token.
    #[error("syntax error at {at:?}{}", Self::display_expected(&expected))]
    Syntax { at: Range<usize>, expected: Vec<&'static str> },
    /// A custom error that may be emitted by the semantic actions of the parser.
    #[error("{0}")]
    Other(#[source] E),
}

/// A default error type for the [ParseError::Other] variant, when the parser
/// has no fallible semantic action, and thus does not need to configure a
/// custom error type.
#[derive(Error, Clone, Copy, Debug)]
#[error("this error should never happen")]
pub struct NoError;

impl<E> ParseError<E> {
    pub fn scan(at: Range<usize>) -> Self {
        Self::Scan { at }
    }

    pub fn syntax_expecting(
        expected: impl IntoIterator<Item = &'static str>,
        at: Range<usize>,
    ) -> Self {
        Self::Syntax { at, expected: expected.into_iter().collect() }
    }

    pub fn syntax(at: Range<usize>) -> Self {
        Self::Syntax { at, expected: Default::default() }
    }

    fn display_expected(expected: &[&'static str]) -> String {
        if expected.is_empty() {
            String::new()
        } else if expected.len() == 1 {
            format!(", expected {}", expected[0])
        } else {
            let last = expected.last().unwrap();
            format!(
                ", expected {} or {last}",
                &expected[0..expected.len() - 1].join(", ")
            )
        }
    }
}

/// Trait implemented for a generated parser.
/// 
/// Use the #[derive(Parser)] attribute on your struct. It must contain three named fields:
/// - A `lexer: logos::Lexer<'s, T>`, where `T` defines [Self::Terminal];
/// - A `symbol_stack: Vec<Symbol<V, T>>`, where `V` defines [Self::Variable];
/// - A `state_stack: Vec<usize>`;
/// 
/// Each of these fields may have another name if they have an attribute that define their role,
/// i.e., `#[gerana(symbol_stack)]`, `#[gerana(state_stack)]` and `#[gerana(lexer)]`.
/// 
/// ## Grammar
/// 
/// To define grammar rules for the new parser, each rule must be specified by the `#[rule(...)]`
/// attribute on the struct. Symbols inside the rule can be either variables or terminals, where
/// variables are represented by identifiers that should match a [Self::Variable] variant, and
/// terminals are represented by a colon followed by an identifier that should match a [Self::Terminal]
/// variant. E.g., `E` is a variable while `:Plus` is a terminal.
/// 
/// Variables inside the body of a production rule must be followed by a parenthesized 
/// binding, so that whatever has been produced by that variable in a previous reduction can be 
/// referenced and used inside the semantic action for that rule. Variables at the head position
/// have no binding. Terminals may or may not have a following binding. Use a binding if you need 
/// to extract data from the token.
/// 
/// After the rule body, a semantic action must be specified, which defines what is produced
/// by the head when the rule is reduced. The semantic action must be an expression inside brackets.
/// 
/// The head of the first rule is defined as the starting variable for the grammar.
/// 
/// ### Example grammar
/// 
/// ```
/// #[rule(E => E(a) :Plus T(b) { a + b } )]
/// #[rule(E => T(a) { a } )]
/// #[rule(T => T(a) :Times F(b) { a * b } )]
/// #[rule(T => F(a) { a } )]
/// #[rule(F => :ParenOp E(a) :ParenCl { a } )]
/// #[rule(F => :Id(a) { Expr::Id(a) } )]
/// ```
/// 
/// ## Lexers
/// 
/// You may define multiple lexers, and use the [Self::set_lexer] method inside semantic actions
/// to change which one is currently being used. In multi-lexer parsers, the struct must contain
/// a `current_lexer: usize` field, and they are identified by the order they appear in the struct,
/// with the first lexer set as the default.
/// 
/// The tokens produced by the remaining lexers must be convertible to the token type of the default
/// lexer. That is, for every lexer of type `logos::Lexer<'s, T>`, [Self::Terminal] has to implement
/// `From<T>`.
pub trait Parser<'s> {
    type Terminal: Logos<'s, Source = str> + Terminal;
    type Variable: Variable;
    type Error: Error;

    fn new(source: &'s str) -> Self;
    fn parse(self) -> Result<<Self::Variable as Variable>::Output, ParseError<Self::Error>>;
    fn set_lexer(&mut self, i: usize);
    fn next_token(&mut self) -> Result<Option<Self::Terminal>, ParseError<Self::Error>>;
    fn slice(&self) -> &'s str;
    fn span(&self) -> std::ops::Range<usize>;
}

#[derive(Debug, Clone)]
pub enum Symbol<V, T> {
    Var(V),
    Term(T),
}

pub trait Terminal {
    fn describe(variant: &'static str) -> &'static str;
}

pub trait Variable {
    type Output;
}
