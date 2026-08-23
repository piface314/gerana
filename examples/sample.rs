use gerana::{Describe, Parser};
use gerana_derive::Parser;
use logos::{Lexer, Logos};
use std::error::Error;

#[derive(Debug, Clone, Logos)]
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
    #[regex("[a-z][a-z0-9-]*|`([^`]|``)*`", unescape_ident, ignore(case))]
    Id(String),
}

impl Describe for SampleToken {
    fn describe(variant: &'static str) -> &'static str {
        match variant {
            "Plus" => "`+`",
            "Times" => "`x`",
            "ParenOp" => "`(`",
            "ParenCl" => "`)`",
            "Id" => "an identifier",
            _ => "",
        }
    }
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

#[derive(Debug)]
enum SampleSymbol {
    E(Expr),
    T(Expr),
    F(Expr),
    Token(SampleToken),
}

impl From<SampleToken> for SampleSymbol {
    fn from(value: SampleToken) -> Self {
        Self::Token(value)
    }
}

impl TryFrom<SampleSymbol> for SampleToken {
    type Error = ();
    fn try_from(value: SampleSymbol) -> Result<Self, Self::Error> {
        match value {
            SampleSymbol::Token(t) => Ok(t),
            _ => Err(()),
        }
    }
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
#[gerana(output = Expr)]
#[rule(E => E(a) :Plus T(b) { a + b } )]
#[rule(E => T(a) { a } )]
#[rule(T => T(a) :Times F(b) { a * b } )]
#[rule(T => F(a) { a } )]
#[rule(F => :ParenOp E(a) :ParenCl { a } )]
#[rule(F => :Id(a) { Expr::Id(a) } )]
struct SampleParser<'s> {
    symbol_stack: Vec<SampleSymbol>,
    state_stack: Vec<usize>,
    lexer: Lexer<'s, SampleToken>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let e1 = SampleParser::new("(a + b) * c").parse()?;
    println!("{e1:?}");
    let e2 = SampleParser::new("a + b * c").parse()?;
    println!("{e2:?}");
    let e3 = SampleParser::new("a + b d").parse().inspect_err(|e| eprintln!("{e}"));
    println!("{e3:?}");
    Ok(())
}
