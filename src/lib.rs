// pub use gerana_derive::ll1_parser;
use logos::Logos;
use std::error::Error;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum ParseError<E: Error> {
    #[error("invalid input {slice:?}")]
    Scan { slice: String },
    #[error("syntax error{}:\n{desc}", expected.as_ref().map(|e| format!(", expected {e}")).unwrap_or_default())]
    Syntax { desc: String, expected: Option<String> },
    #[error("{0}")]
    Other(#[source] E),
}

impl<E: Error> From<E> for ParseError<E> {
    fn from(e: E) -> Self {
        ParseError::Other(e)
    }
}

pub trait Parser<'s> {
    type Token: Logos<'s> + Describe;
    type Symbol;
    type Error: Error;

    fn new(source: &'s <Self::Token as Logos<'s>>::Source) -> Self;

    fn parse(self) -> Result<Self::Symbol, ParseError<Self::Error>>;
}

pub trait Describe {
    fn describe(variant: &'static str) -> &'static str;
}

#[cfg(test)]
mod tests {
    use logos::{Lexer, Logos};
    use gerana_derive::Parser;
    use crate::Parser;

    #[derive(Debug, Clone, Logos)]
    enum SampleToken {
        #[token("+")]
        Plus,
        #[token("*")]
        Times,
        #[token("(")]
        ParenOp,
        #[token(")")]
        ParenCl,
        #[regex("[a-z][a-z0-9-]*|`([^`]|``)*`", unescape_ident, ignore(case))]
        Id(String),
    }

    fn unescape_ident(lex: &Lexer<SampleToken>) -> String {
        let escaped = lex.slice().chars().next().is_some_and(|x| x == '`');
        if escaped {
            let span = lex.span();
            lex.source()[span.start + 1..span.end - 1].replace("``", "`")
        } else {
            lex.slice().to_string()
        }
    }

    enum SampleSymbol {
        None,
        E(Predicate),
        T(Predicate),
        F(Predicate),
        Id(Predicate),
    }

    #[derive(Debug, Clone, PartialEq)]
    pub enum Predicate {
        Add(Box<Predicate>, Box<Predicate>),
        Mul(Box<Predicate>, Box<Predicate>),
        Id(String),
    }

    impl std::ops::Add for Predicate {
        type Output = Predicate;
        fn add(self, rhs: Self) -> Self::Output {
            Predicate::Add(self.into(), rhs.into())
        }
    }

    impl std::ops::Mul for Predicate {
        type Output = Predicate;
        fn mul(self, rhs: Self) -> Self::Output {
            Predicate::Mul(self.into(), rhs.into())
        }
    }

    #[derive(Parser)]
    #[rule(E => E(a) :Plus T(b) { a + b } )]
    #[rule(E => T(a) { a } )]
    #[rule(T => T(a) :Times F(b) { a * b } )]
    #[rule(T => F(a) { a } )]
    #[rule(F => :ParenOp E(a) :ParenCl { a } )]
    #[rule(F => :Id(a) { Predicate::Id(a) } )]
    struct SampleParser<'s> {
        lexer: Lexer<'s, SampleToken>,
        symbol_stack: Vec<SampleSymbol>,
        state_stack: Vec<usize>,
    }

    #[test]
    fn test_sample_parser() {}
}
