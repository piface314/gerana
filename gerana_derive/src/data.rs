use std::collections::{BTreeSet, HashMap, HashSet};
use syn::{parenthesized, Token};

const EMPTY: &'static str = "";
const END: &'static str = "$";

pub struct ContextFreeGrammar<'r> {
    variables: HashSet<&'r str>,
    alphabet: HashSet<&'r str>,
    rules: &'r [Rule],
    start_variable: &'r str,
    rule_head_index: HashMap<&'r str, Vec<(usize, &'r Rule)>>,
}

impl<'r> ContextFreeGrammar<'r> {
    pub fn new(ast: &syn::DeriveInput, rules: &'r [Rule]) -> Result<Self, syn::Error> {
        let variables = rules
            .iter()
            .map(|r| r.head.as_str())
            .chain(rules.iter().flat_map(|r| {
                r.body.iter().filter_map(|e| match e {
                    Symbol::Var(v) => Some(v.name.as_str()),
                    _ => None,
                })
            }))
            .collect();
        let alphabet = rules
            .iter()
            .flat_map(|r| {
                r.body.iter().filter_map(|e| match e {
                    Symbol::Term(t) => Some(t.name.as_str()),
                    _ => None,
                })
            })
            .collect();
        let start_variable = rules
            .first()
            .ok_or_else(|| syn::Error::new_spanned(ast, "at least one rule must be specified"))?
            .head
            .as_str();
        let rule_head_index = Self::build_rule_head_index(rules);
        Ok(Self { variables, alphabet, rules, start_variable, rule_head_index })
    }

    fn build_rule_head_index(rules: &'r [Rule]) -> HashMap<&'r str, Vec<(usize, &'r Rule)>> {
        let mut rule_head_index: HashMap<&'r str, Vec<(usize, &'r Rule)>> = HashMap::new();
        for (i, r) in rules.iter().enumerate() {
            rule_head_index
                .entry(r.head.as_str())
                .or_default()
                .push((i, r));
        }
        rule_head_index
    }

    fn symbols(&'_ self) -> impl Iterator<Item = SymbolRef<'_>> {
        self.variables
            .iter()
            .copied()
            .map(SymbolRef::Var)
            .chain(self.alphabet.iter().copied().map(SymbolRef::Term))
    }
}

#[derive(Debug)]
pub struct MemoFirst<'r>(HashMap<&'r str, HashSet<&'r str>>);

impl<'r> MemoFirst<'r> {
    pub fn new(grammar: &ContextFreeGrammar<'r>) -> Result<Self, syn::Error> {
        let mut memo = HashMap::new();
        let mut visited = HashSet::new();
        for v in grammar.rule_head_index.keys() {
            Self::build_memo_first_r(v, None, &grammar.rule_head_index, &mut memo, &mut visited)?;
        }
        Ok(Self(memo))
    }

    fn build_memo_first_r(
        var: &'r str,
        var_ident: Option<&syn::Ident>,
        rule_head_index: &HashMap<&'r str, Vec<(usize, &'r Rule)>>,
        memo: &mut HashMap<&'r str, HashSet<&'r str>>,
        visited: &mut HashSet<usize>,
    ) -> Result<usize, syn::Error> {
        let mut checked_rules = 0;
        for (i, r) in rule_head_index
            .get(var)
            .ok_or_else(|| syn::Error::new_spanned(var_ident, "variable is not head of any rule"))?
        {
            if visited.contains(i) {
                continue;
            }
            checked_rules += 1;
            visited.insert(*i);
            let mut has_terminal = false;
            for x in r.body.iter() {
                match x {
                    Symbol::Term(t) => {
                        has_terminal = true;
                        memo.entry(var).or_default().insert(t.name.as_str());
                        break;
                    }
                    Symbol::Var(v) => {
                        let next_var = v.name.as_str();
                        if !memo.contains_key(next_var) {
                            let n = Self::build_memo_first_r(
                                next_var,
                                Some(&v.name_ident),
                                rule_head_index,
                                memo,
                                visited,
                            )?;
                            if n == 0 {
                                return Err(syn::Error::new_spanned(
                                    &v.name_ident,
                                    "grammar has infinite recursion",
                                ));
                            }
                        }
                        memo.entry(var).or_default();
                        let next_var_has_empty = memo
                            .get(next_var)
                            .expect("memo entry should be created")
                            .contains(EMPTY);
                        if !next_var_has_empty {
                            has_terminal = true;
                        }
                        if next_var == var {
                            if !next_var_has_empty {
                                break;
                            } else {
                                continue;
                            }
                        }
                        let [first_current, first_next] = memo.get_disjoint_mut([var, next_var]);
                        let first_current = first_current.expect("memo entry should be created");
                        let first_next = first_next.expect("memo entry should be created");
                        first_current.extend(first_next.iter().filter(|x| !x.is_empty()));
                        if !next_var_has_empty {
                            break;
                        }
                    }
                }
            }
            if !has_terminal {
                memo.entry(var).or_default().insert(EMPTY);
            }
        }
        Ok(checked_rules)
    }

    pub fn first(&self, seq: &'r [Symbol]) -> Result<HashSet<&'r str>, syn::Error> {
        let mut first_seq = HashSet::new();
        for x in seq {
            match x {
                Symbol::Term(t) => {
                    first_seq.insert(t.name.as_str());
                    break;
                }
                Symbol::Var(v) => {
                    let first_v = self.0.get(v.name.as_str()).ok_or_else(|| {
                        syn::Error::new_spanned(&v.name_ident, "variable is not head of any rule")
                    })?;
                    first_seq.extend(first_v.iter().filter(|t| **t != EMPTY));
                    if !first_v.contains(EMPTY) {
                        break;
                    }
                }
            }
        }
        Ok(first_seq)
    }
}

#[derive(Debug)]
pub struct MemoFollow<'r>(HashMap<&'r str, HashSet<&'r str>>);

impl<'r> MemoFollow<'r> {
    pub fn new(
        grammar: &ContextFreeGrammar<'r>,
        memo_first: &MemoFirst<'r>,
    ) -> Result<Self, syn::Error> {
        let mut memo = HashMap::new();
        for v in grammar.variables.iter().copied() {
            memo.insert(v, HashSet::new());
        }
        memo.entry(grammar.start_variable).or_default().insert(END);
        while let true = Self::build_memo_pass(grammar, &mut memo, memo_first)? {}
        Ok(Self(memo))
    }

    fn build_memo_pass(
        grammar: &ContextFreeGrammar<'r>,
        memo: &mut HashMap<&'r str, HashSet<&'r str>>,
        memo_first: &MemoFirst<'r>,
    ) -> Result<bool, syn::Error> {
        let mut added: usize = 0;
        for rule in grammar.rules {
            for (i, x) in rule.body.iter().enumerate() {
                match x {
                    Symbol::Term(_) => continue,
                    Symbol::Var(v) => {
                        if i + 1 == rule.body.len() {
                            added += Self::build_memo_follow_rule(memo, v, rule);
                            break;
                        }
                        let follow_var = memo.entry(v.name.as_str()).or_default();
                        let mut first_tail = memo_first.first(&rule.body[i + 1..])?;
                        let first_tail_has_empty = first_tail.remove(EMPTY);
                        for t in first_tail {
                            added += follow_var.insert(t) as usize;
                        }
                        if first_tail_has_empty {
                            added += Self::build_memo_follow_rule(memo, v, rule);
                        }
                    }
                }
            }
        }
        Ok(added > 0)
    }

    fn build_memo_follow_rule(
        memo: &mut HashMap<&'r str, HashSet<&'r str>>,
        var: &'r Variable,
        rule: &'r Rule,
    ) -> usize {
        if var.name == rule.head {
            return 0;
        }
        // SAFETY: keys disjointness previously checked
        let [follow_var, follow_head] =
            unsafe { memo.get_disjoint_unchecked_mut([var.name.as_str(), rule.head.as_str()]) };
        let follow_var = follow_var.expect("all variable memo sets should be created");
        let follow_head = follow_head.expect("all variable memo sets should be created");
        let mut added = 0;
        for t in follow_head.iter().copied() {
            added += follow_var.insert(t) as usize;
        }
        added
    }

    pub fn follow(
        &'_ self,
        var: &str,
        var_ident: &syn::Ident,
    ) -> Result<&'_ HashSet<&'r str>, syn::Error> {
        self.0
            .get(var)
            .ok_or_else(|| syn::Error::new_spanned(var_ident, "something wrong with this variable"))
    }
}

#[derive(Debug)]
pub struct LrZeroAutomaton<'r> {
    states: Vec<LrZeroState>,
    goto: HashMap<(usize, SymbolRef<'r>), usize>,
}

#[derive(Debug)]
pub struct LrZeroState {
    items: BTreeSet<LrZeroItem>,
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct LrZeroItem {
    pointer: usize,
    rule: LrZeroRule,
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum LrZeroRule {
    Start,
    Regular(usize),
}

impl<'r> LrZeroAutomaton<'r> {
    pub const MAX_ITERATIONS: usize = 10_000;

    pub fn new(grammar: &'r ContextFreeGrammar<'r>) -> Result<Self, syn::Error> {
        let mut this = Self {
            states: vec![LrZeroState::closure([LrZeroItem::new_start()], grammar)],
            goto: Default::default(),
        };
        let mut states_set = HashSet::from([this.states[0].items.clone()]);
        let mut n_iterations = 0;
        loop {
            let n_start = this.states.len();
            for i in 0..n_start {
                for x in grammar.symbols() {
                    let goto = this.states[i].goto(x, grammar);
                    if !goto.is_empty() && !states_set.contains(&goto) {
                        let new_state_index = this.states.len();
                        this.states
                            .push(LrZeroState::closure(goto.clone(), grammar));
                        states_set.insert(goto);
                        this.goto.insert((i, x), new_state_index);
                    }
                }
            }
            if this.states.len() == n_start {
                return Ok(this);
            } else if n_iterations > Self::MAX_ITERATIONS {
                return Err(syn::Error::new_spanned(
                    None as Option<syn::Ident>,
                    "exceeded maximum iterations for grammar LR(0) automaton construction",
                ));
            }
            n_iterations += 1;
        }
    }

    pub fn display_with_grammar(
        &self,
        f: &mut impl std::io::Write,
        grammar: &'r ContextFreeGrammar<'r>,
    ) -> std::io::Result<()> {
        for (i, state) in self.states.iter().enumerate() {
            write!(f, "State #{i}:\n")?;
            for item in state.items.iter() {
                write!(f, "- ")?;
                item.display_with_grammar(f, grammar)?;
                write!(f, "\n")?;
            }
        }
        for ((i, x), j) in self.goto.iter() {
            write!(f, "GOTO({i}, {x}) = {j}\n",)?;
        }
        Ok(())
    }
}

impl LrZeroState {
    fn closure<'r>(
        items: impl IntoIterator<Item = LrZeroItem>,
        grammar: &ContextFreeGrammar<'r>,
    ) -> Self {
        let mut this = Self { items: items.into_iter().collect() };
        loop {
            let n_start = this.items.len();
            let mut new_items = Vec::new();
            for item in this.items.iter() {
                if let Some(SymbolRef::Var(v)) = item.next_symbol(grammar) {
                    let v_rules = grammar.rule_head_index.get(v).map(|r| r.iter());
                    for (i, _) in v_rules.into_iter().flatten() {
                        new_items.push(LrZeroItem::new_regular(*i));
                    }
                }
            }
            this.items.extend(new_items);
            if this.items.len() == n_start {
                return this;
            }
        }
    }

    fn goto<'r>(&self, x: SymbolRef<'r>, grammar: &ContextFreeGrammar<'r>) -> BTreeSet<LrZeroItem> {
        self.items
            .iter()
            .filter_map(|item| item.advanced(x, grammar))
            .collect()
    }
}

impl LrZeroItem {
    fn new_start() -> Self {
        Self { pointer: 0, rule: LrZeroRule::Start }
    }

    fn new_regular(rule_index: usize) -> Self {
        Self { pointer: 0, rule: LrZeroRule::Regular(rule_index) }
    }

    fn next_symbol<'r>(&self, grammar: &ContextFreeGrammar<'r>) -> Option<SymbolRef<'r>> {
        match self.rule {
            LrZeroRule::Start => {
                if self.pointer == 0 {
                    Some(SymbolRef::Var(grammar.start_variable))
                } else {
                    None
                }
            }
            LrZeroRule::Regular(i) => {
                let rule = grammar.rules.get(i)?;
                rule.body.get(self.pointer).map(|s| s.into())
            }
        }
    }

    fn advanced<'r>(&self, x: SymbolRef<'r>, grammar: &ContextFreeGrammar<'r>) -> Option<Self> {
        match self.rule {
            LrZeroRule::Start => {
                if self.pointer == 0 && x == SymbolRef::Var(grammar.start_variable) {
                    Some(Self { pointer: 1, rule: LrZeroRule::Start })
                } else {
                    None
                }
            }
            LrZeroRule::Regular(i) => {
                let rule = grammar.rules.get(i)?;
                if x == SymbolRef::from(rule.body.get(self.pointer)?) {
                    Some(Self { pointer: self.pointer + 1, rule: self.rule })
                } else {
                    None
                }
            }
        }
    }

    fn is_start(&self) -> bool {
        match self.rule {
            LrZeroRule::Start => true,
            _ => false,
        }
    }

    fn grammar_rule<'r>(&self, grammar: &ContextFreeGrammar<'r>) -> Option<(usize, &'r Rule)> {
        match self.rule {
            LrZeroRule::Start => None,
            LrZeroRule::Regular(i) => grammar.rules.get(i).map(|r| (i, r)),
        }
    }

    fn display_with_grammar(
        &self,
        f: &mut impl std::io::Write,
        grammar: &ContextFreeGrammar<'_>,
    ) -> std::io::Result<()> {
        match self.rule {
            LrZeroRule::Start => {
                if self.pointer == 0 {
                    write!(f, "* => . {}", grammar.start_variable)?;
                } else {
                    write!(f, "* => {} .", grammar.start_variable)?;
                }
            }
            LrZeroRule::Regular(i) => {
                let rule = &grammar.rules[i];
                write!(f, "{} =>", rule.head)?;
                for (i, x) in rule.body.iter().enumerate() {
                    let x = SymbolRef::from(x);
                    let dot = if i == self.pointer { ". " } else { "" };
                    write!(f, " {dot}{x}")?;
                }
                if self.pointer == rule.body.len() {
                    write!(f, " .")?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SlrTable<'r> {
    action: HashMap<(usize, &'r str), SlrAction>,
    goto: HashMap<(usize, &'r str), usize>,
}

#[derive(Debug, Clone, Copy)]
pub enum SlrAction {
    Shift(usize),
    Reduce(usize),
    Accept,
}

impl<'r> SlrTable<'r> {
    pub fn new(
        grammar: &'r ContextFreeGrammar<'r>,
        memo_follow: &'r MemoFollow<'r>,
        lrz_automaton: LrZeroAutomaton<'r>,
    ) -> Result<Self, syn::Error> {
        let mut this = Self { action: Default::default(), goto: Default::default() };
        for (i, state) in lrz_automaton.states.into_iter().enumerate() {
            for item in state.items.into_iter() {
                match item.next_symbol(grammar) {
                    Some(x @ SymbolRef::Term(t)) => {
                        if let Some(j) = lrz_automaton.goto.get(&(i, x)) {
                            this.set_action(i, t, SlrAction::Shift(*j))?;
                        }
                    }
                    None if item.is_start() => {
                        this.set_action(i, END, SlrAction::Accept)?;
                    }
                    None => {
                        let (rule_index, rule) = item
                            .grammar_rule(grammar)
                            .expect("rule should exist in grammar");
                        for t in memo_follow.follow(&rule.head, &rule.head_ident)? {
                            this.set_action(i, t, SlrAction::Reduce(rule_index))?;
                        }
                    }
                    _ => {}
                }
            }
        }
        this.goto = lrz_automaton
            .goto
            .into_iter()
            .filter_map(|((i, x), j)| match x {
                SymbolRef::Var(x) => Some(((i, x), j)),
                _ => None,
            })
            .collect();
        Ok(this)
    }

    fn set_action(&mut self, i: usize, t: &'r str, action: SlrAction) -> Result<(), syn::Error> {
        match self.action.insert((i, t), action) {
            None => Ok(()),
            Some(prev_action) => {
                let msg = if prev_action.is_reduce() && action.is_reduce() {
                    "grammar contains reduce/reduce conflict"
                } else if prev_action.is_shift() || action.is_shift() {
                    "grammar contains shift/reduce conflict"
                } else {
                    "grammar contains unknown conflict"
                };
                Err(syn::Error::new_spanned(None as Option<syn::Ident>, msg))
            }
        }
    }
}

impl SlrAction {
    fn is_shift(&self) -> bool {
        match self {
            Self::Shift(_) => true,
            _ => false,
        }
    }

    fn is_reduce(&self) -> bool {
        match self {
            Self::Reduce(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct RuleP {
    head: syn::Ident,
    #[allow(unused)]
    arrow_token: Token![=>],
    body: Vec<SymbolP>,
    action: syn::ExprBlock,
}

#[derive(Debug)]
pub struct Rule {
    head: String,
    head_ident: syn::Ident,
    body: Vec<Symbol>,
    action: syn::ExprBlock,
}

impl From<RuleP> for Rule {
    fn from(value: RuleP) -> Self {
        Self {
            head: value.head.to_string(),
            head_ident: value.head,
            body: value.body.into_iter().map(|v| v.into()).collect(),
            action: value.action,
        }
    }
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} =>", self.head)?;
        for x in self.body.iter() {
            write!(f, " {x}")?;
        }
        Ok(())
    }
}

impl syn::parse::Parse for RuleP {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let head = input.parse()?;
        let arrow_token = input.parse()?;
        let mut body = Vec::new();
        loop {
            if input.lookahead1().peek(syn::token::Brace) {
                break;
            }
            if input.is_empty() {
                return Err(input.error("expected an action { ... } "));
            }
            body.push(input.parse::<SymbolP>()?);
        }
        let action = input.parse()?;
        Ok(Self { head, arrow_token, body, action })
    }
}

#[derive(Debug)]
pub enum SymbolP {
    Var(VariableP),
    Term(TerminalP),
}

#[derive(Debug)]
pub enum Symbol {
    Var(Variable),
    Term(Terminal),
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum SymbolRef<'r> {
    Var(&'r str),
    Term(&'r str),
}

impl<'r> std::fmt::Display for SymbolRef<'r> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let marker = if self.is_term() { ":" } else { "" };
        write!(f, "{marker}{}", self.as_ref())
    }
}

impl<'r> AsRef<str> for SymbolRef<'r> {
    fn as_ref(&self) -> &str {
        match self {
            Self::Term(x) => x,
            Self::Var(x) => x,
        }
    }
}

impl<'r> From<&'r Symbol> for SymbolRef<'r> {
    fn from(value: &'r Symbol) -> Self {
        match value {
            Symbol::Term(t) => SymbolRef::Term(&t.name),
            Symbol::Var(v) => SymbolRef::Var(&v.name),
        }
    }
}

impl<'r> SymbolRef<'r> {
    #[allow(unused)]
    fn is_var(&self) -> bool {
        match self {
            Self::Var(_) => true,
            _ => false,
        }
    }

    fn is_term(&self) -> bool {
        match self {
            Self::Term(_) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Var(v) => write!(f, "{}", v.name),
            Self::Term(t) => write!(f, ":{}", t.name),
        }
    }
}

impl From<SymbolP> for Symbol {
    fn from(value: SymbolP) -> Self {
        match value {
            SymbolP::Var(v) => Self::Var(v.into()),
            SymbolP::Term(t) => Self::Term(t.into()),
        }
    }
}

impl From<VariableP> for SymbolP {
    fn from(value: VariableP) -> Self {
        SymbolP::Var(value)
    }
}

impl From<TerminalP> for SymbolP {
    fn from(value: TerminalP) -> Self {
        SymbolP::Term(value)
    }
}

impl syn::parse::Parse for SymbolP {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let lookahead = input.lookahead1();
        let elem = if lookahead.peek(Token![:]) {
            input.parse::<TerminalP>()?.into()
        } else {
            input.parse::<VariableP>()?.into()
        };
        Ok(elem)
    }
}

#[derive(Debug)]
pub struct VariableP {
    name: syn::Ident,
    binding: Binding,
}

#[derive(Debug)]
pub struct Variable {
    name: String,
    name_ident: syn::Ident,
    binding: Binding,
}

impl From<VariableP> for Variable {
    fn from(value: VariableP) -> Self {
        Self {
            name: value.name.to_string(),
            name_ident: value.name,
            binding: value.binding.into(),
        }
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Variable {}

impl std::hash::Hash for Variable {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl syn::parse::Parse for VariableP {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let name: syn::Ident = input.parse()?;
        let binding = input.parse()?;
        Ok(Self { name, binding })
    }
}

#[derive(Debug)]
pub struct TerminalP {
    #[allow(unused)]
    marker_token: Token![:],
    name: syn::Ident,
    binding: Option<Binding>,
}

#[derive(Debug)]
pub struct Terminal {
    name: String,
    name_ident: syn::Ident,
    binding: Option<Binding>,
}

impl From<TerminalP> for Terminal {
    fn from(value: TerminalP) -> Self {
        Self {
            name: value.name.to_string(),
            name_ident: value.name,
            binding: value.binding.map(|b| b.into()),
        }
    }
}

impl syn::parse::Parse for TerminalP {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let marker_token = input.parse::<Token![:]>()?;
        let name: syn::Ident = input.parse()?;
        let binding = if input.lookahead1().peek(syn::token::Paren) {
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self { marker_token, name, binding })
    }
}

#[derive(Debug)]
pub struct Binding {
    #[allow(unused)]
    paren: syn::token::Paren,
    mut_token: Option<Token![mut]>,
    ident: syn::Ident,
}

impl syn::parse::Parse for Binding {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        let paren = parenthesized!(content in input);
        let mut_token = if content.lookahead1().peek(Token![mut]) {
            Some(content.parse()?)
        } else {
            None
        };
        let ident = content.parse()?;
        Ok(Self { paren, mut_token, ident })
    }
}
