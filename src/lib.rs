use logos::Logos;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum ParseError<E> {
    #[error("invalid input {slice:?}")]
    Scan { slice: String },
    #[error("syntax error{}:\n{desc}", expected.as_ref().map(|e| format!(", expected {e}")).unwrap_or_default())]
    Syntax { desc: String, expected: Option<String> },
    #[error("{0}")]
    Other(#[source] E),
}

impl<E> ParseError<E> {
    pub fn scan(slice: impl AsRef<str>) -> Self {
        Self::Scan { slice: slice.as_ref().to_string() }
    }

    pub fn syntax_expecting(expected: &str, src: &str, i: usize) -> Self {
        Self::Syntax {
            desc: str_excerpt(10, i, src),
            expected: Some(expected.to_string()),
        }
    }

    pub fn syntax(src: &str, i: usize) -> Self {
        Self::Syntax { desc: str_excerpt(10, i, src), expected: None }
    }
}

fn str_excerpt(n: usize, index: usize, src: &str) -> String {
    let n_start = n / 2;
    let n_end = n - n_start;
    let mut start = index.saturating_sub(n_start); // i - st = nst
    let mut end = index.saturating_add(n_end).clamp(0, src.len());
    while start > 0 && !src.is_char_boundary(start) {
        start -= 1;
    }
    while end < src.len() && !src.is_char_boundary(end) {
        end += 1;
    }
    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if end < src.len() { "..." } else { "" };
    let padding = " ".repeat(
        prefix.len()
            + src[start..]
                .char_indices()
                .take_while(|(i, _)| *i < index - start)
                .count(),
    );
    let excerpt = format!("{prefix}{}{suffix}\n{padding}^", &src[start..end]);
    excerpt
}

pub trait Parser<'s> {
    type Token: Logos<'s, Source = str> + Describe + TryFrom<Self::Symbol>;
    type Symbol: From<Self::Token>;
    type Output;
    type Error: std::fmt::Display;

    fn new(source: &'s str) -> Self;
    fn parse(self) -> Result<Self::Output, ParseError<Self::Error>>;
    fn set_lexer(&mut self, i: usize);
    fn next_token(&mut self) -> Result<Option<Self::Token>, ParseError<Self::Error>>;
    fn slice(&self) -> &'s str;
    fn span(&self) -> std::ops::Range<usize>;
}

pub trait Describe {
    fn describe(variant: &'static str) -> &'static str;
}
