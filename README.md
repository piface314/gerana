# gerana

A basic SLR parser generator.

This crate was built for reusability in personal projects, and as an exercise.
It depends on [Logos](https://docs.rs/logos/latest/logos/) for lexical analysis, and provides
a derive macro with `#[rule]` attributes to generate parsers.

The crate name, gerana, is a portmanteau on the Portuguese words for "syntax parser generator"
("**ger**ador de **ana**lisador sintático").
