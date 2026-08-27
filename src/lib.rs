use core::ops::Range;
pub use gerana_derive::{Parser, Terminal, Variable};
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
/// Use the `#[derive(Parser)]` attribute on your struct. It must contain three named fields:
/// - A `lexer: logos::Lexer<'s, T>`, where `T` defines [Self::Terminal];
/// - A `symbol_stack: Vec<Symbol<V, T>>`, where `V` defines [Self::Variable];
/// - A `state_stack: Vec<usize>`.
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
/// a `current_lexer: usize` field, and each lexer is identified by the order it is defined in the
/// struct, with the first lexer set as the default.
///
/// The tokens produced by the remaining lexers must be convertible to the token type of the default
/// lexer. That is, for every lexer of type `logos::Lexer<'s, T>`, [Self::Terminal] has to implement
/// `From<T>`.
/// 
/// ## Errors and fallible semantic actions
/// 
/// If semantic actions deal with [Result] types, set the custom error type with
/// `#[gerana(error = E)]`, where `E: Into<ParseError<E>>`.
pub trait Parser<'s> {
    type Terminal: Logos<'s, Source = str> + Terminal;
    type Variable: Variable;
    type Error: Error;

    /// Creates a parser prepared to read from a specific source.
    /// 
    /// Currently, only [str] is supported as the source type.
    fn new(source: &'s str) -> Self;

    /// Runs the parser over the whole input source, returning the expected output on success.
    fn parse(self) -> Result<<Self::Variable as Variable>::Output, ParseError<Self::Error>>;

    /// Changes the lexer to be used to scan the next tokens.
    /// 
    /// This method can be used inside semantic actions to read different tokens from the input
    /// depending on the context of your parser.
    /// 
    /// ## Panics
    /// 
    /// Panics if `i` is outside the range of available lexers.
    fn set_lexer(&mut self, i: usize);

    /// Reads the next token from the input source.
    fn next_token(&mut self) -> Result<Option<Self::Terminal>, ParseError<Self::Error>>;

    /// The slice from the input source that corresponds to the last read token.
    fn slice(&self) -> &'s str;

    /// The range from the input source that corresponds to the last read token.
    fn span(&self) -> std::ops::Range<usize>;
}

/// An enum that unifies grammar variables and terminals in the same type.
/// 
/// This type is intended to be used in the [Parser] symbol stack (`Vec<Symbol<V, T>>`).
#[derive(Debug, Clone)]
pub enum Symbol<V, T> {
    Var(V),
    Term(T),
}

/// Trait for the tokens/terminals available in the grammar.
/// 
/// Currently, the only role of this trait is to help provide descriptive error messages
/// to [ParseError].
/// 
/// Use the `#[derive(Terminal)]` attribute to implement this trait for your Logos enum. The macro
/// will look for `#[token(...)]` attributes and automatically assign them as their description.
/// For other variants, you must provide a description through `#[gerana(desc = "...")]`.
pub trait Terminal {
    /// Given the name of a terminal variant, a description of the token should be returned.
    /// 
    /// ## Panics
    /// 
    /// Panics if `variant` is not a variant of the terminal enum. Parsers generated by
    /// `#[derive(Parser)]` will only call this method on terminal variants that are used in the
    /// grammar rules.
    fn describe(variant: &'static str) -> &'static str;
}

/// Trait for the variables available in the grammar.
/// 
/// Currently, the only role of this trait is to define the final output type of the parser,
/// which corresponds to the type stored in the variant of the enum that is used as the starting
/// variable of the grammar.
/// 
/// Use the `#[derive(Variable)]` attribute to implement this trait on your variable enum. The macro
/// will always assume the first variant is the grammar starting variable.
pub trait Variable {
    type Output;
}
