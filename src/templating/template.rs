#[derive(PartialEq, Debug)]
pub enum SymbolInternal {
    Word(String),
    SubtemplateCall(String),
    VarBind(String),
    Template(Box<Template>),
}

#[derive(PartialEq, Debug)]
pub struct Symbol {
    pub(crate) symbol: SymbolInternal,
    pub(crate) optional: bool,
}
impl Symbol {
    pub fn new(symbol: SymbolInternal, optional: bool) -> Self {
        Self {
            symbol: symbol,
            optional: optional,
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct Clause {
    pub(crate) symbols: Vec<Symbol>,
}
impl Clause {
    pub fn new(symbols: Vec<Symbol>) -> Self {
        Self {
            symbols: symbols,
        }
    }
    pub fn single(sym: Symbol) -> Self {
        Self {
            symbols: vec![sym],
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct Template {
    pub(crate) clauses: Vec<Clause>,
}
impl Template {
    pub fn new(clauses: Vec<Clause>) -> Self {
        Self {
            clauses: clauses,
        }
    }
    pub fn single(c: Clause) -> Self {
        Self {
            clauses: vec![c],
        }
    }
}
