use gerana::{Parser, Symbol};
use gerana_derive::{Parser, Terminal, Variable};
use logos::{Lexer, Logos};
use std::error::Error;

#[derive(Debug, Clone, Logos, Terminal)]
#[logos(skip r"[ \t]+")]
enum SampleToken {
    #[token("+")]
    Plus,
    #[token("*")]
    Times,
    #[token("(")]
    ParenOp,
    #[token(")")]
    ParenCl,
    #[gerana(desc = "an identifier")]
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

#[derive(Debug, Variable)]
enum SampleVar {
    E(Expr),
    T(Expr),
    F(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Id(String),
}

impl std::ops::Add for Expr {
    type Output = Expr;
    fn add(self, rhs: Self) -> Self::Output {
        Expr::Add(self.into(), rhs.into())
    }
}

impl std::ops::Mul for Expr {
    type Output = Expr;
    fn mul(self, rhs: Self) -> Self::Output {
        Expr::Mul(self.into(), rhs.into())
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Add(a, b) => write!(f, "(+ {a} {b})"),
            Self::Mul(a, b) => write!(f, "(* {a} {b})"),
            Self::Id(id) => write!(f, "{id}"),
        }
    }
}

#[derive(Parser, Debug)]
#[rule(E => E(a) :Plus T(b) { a + b } )]
#[rule(E => T(a) { a } )]
#[rule(T => T(a) :Times F(b) { a * b } )]
#[rule(T => F(a) { a } )]
#[rule(F => :ParenOp E(a) :ParenCl { a } )]
#[rule(F => :Id(a) { Expr::Id(a) } )]
struct SampleParser<'s> {
    symbol_stack: Vec<Symbol<SampleVar, SampleToken>>,
    state_stack: Vec<usize>,
    lexer: Lexer<'s, SampleToken>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let e1 = SampleParser::new("(a + b) * c").parse()?;
    println!("{e1:?}");
    let e2 = SampleParser::new("a + b * c").parse()?;
    println!("{e2:?}");
    let e3 = SampleParser::new("a + b +")
        .parse()
        .inspect_err(|e| eprintln!("{e}"));
    println!("{e3:?}");
    Ok(())
}
