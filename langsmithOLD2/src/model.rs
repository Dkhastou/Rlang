use crate::glyph::Shape;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Language names end up in file paths and in shell code emitted by
/// `activate`, so keep them to a safe alphabet: letters, digits, `_`, `-`,
/// starting with a letter or digit.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A symbol's glyph is either hand-typed (a literal string, the old
/// behaviour) or generated (a structured stroke shape that can be
/// mirrored, blended, extended and rendered).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Glyph {
    Literal(String),
    Generated(Shape),
}

impl Glyph {
    /// A short, single-line representation for tables/trees/diffs: the
    /// literal text as typed, or a compact stroke-type inventory like
    /// "<2L 1A 1D>" for a generated shape.
    pub fn display_inline(&self) -> String {
        match self {
            Glyph::Literal(text) => text.clone(),
            Glyph::Generated(shape) => shape.summary(),
        }
    }

    /// The full multi-line glyph, drawn on its real 3x3 anchor grid.
    pub fn display_art(&self) -> String {
        match self {
            Glyph::Literal(text) => text.clone(),
            Glyph::Generated(shape) => shape.to_glyph_art(),
        }
    }

    pub fn as_shape(&self) -> Option<&Shape> {
        match self {
            Glyph::Generated(s) => Some(s),
            Glyph::Literal(_) => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Symbol {
    pub id: String,
    pub glyph: Glyph,
    pub gloss: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct Triple {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// A LangStore is a disposable "language": a bag of symbols plus a set of
/// (subject, predicate, object) triples relating them — the same shape as
/// an RDF graph, just without a real triple store or reasoner behind it yet.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct LangStore {
    pub name: String,
    pub symbols: HashMap<String, Symbol>,
    pub triples: Vec<Triple>,
}

impl LangStore {
    pub fn new(name: &str) -> Self {
        LangStore {
            name: name.to_string(),
            symbols: HashMap::new(),
            triples: Vec::new(),
        }
    }

    fn dir() -> PathBuf {
        PathBuf::from(".langsmith")
    }

    fn path_for(name: &str) -> PathBuf {
        Self::dir().join(format!("{}.json", name))
    }

    pub fn exists(name: &str) -> bool {
        Self::path_for(name).exists()
    }

    pub fn load(name: &str) -> Result<Self> {
        let path = Self::path_for(name);
        let data = fs::read_to_string(&path)
            .map_err(|_| anyhow!("language '{}' not found (expected {:?})", name, path))?;
        Ok(serde_json::from_str(&data)?)
    }

    pub fn save(&self) -> Result<()> {
        fs::create_dir_all(Self::dir())?;
        let data = serde_json::to_string_pretty(self)?;
        fs::write(Self::path_for(&self.name), data)?;
        Ok(())
    }

    pub fn list_all() -> Result<Vec<String>> {
        let dir = Self::dir();
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut names = vec![];
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if let Some(stem) = entry.path().file_stem() {
                names.push(stem.to_string_lossy().to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn require_symbol(&self, id: &str) -> Result<&Symbol> {
        self.symbols
            .get(id)
            .ok_or_else(|| anyhow!("no symbol '{}' in language '{}'", id, self.name))
    }
}
