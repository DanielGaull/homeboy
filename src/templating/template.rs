pub enum SymbolInternal {
    Word(String),
    SubtemplateCall(String),
    VarBind(String),
    Template(Box<Template>),
}

pub struct Symbol {
    pub(crate) symbol: SymbolInternal,
    pub(crate) optional: bool,
}

pub struct Template {
    pub(crate) symbols: Vec<Symbol>,
}
