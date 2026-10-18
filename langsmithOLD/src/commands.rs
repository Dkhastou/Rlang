use crate::glyph::{Axis, Shape};
use crate::model::{Glyph, LangStore, Symbol, Triple};
use anyhow::{bail, Result};
use std::collections::HashSet;
use std::fs;

pub fn new(name: &str) -> Result<()> {
    if LangStore::exists(name) {
        bail!("language '{}' already exists", name);
    }
    LangStore::new(name).save()?;
    println!("created language '{}'", name);
    Ok(())
}

pub fn list() -> Result<()> {
    let names = LangStore::list_all()?;
    if names.is_empty() {
        println!("(no languages yet — try `lang new mylang`)");
        return Ok(());
    }
    for n in names {
        let store = LangStore::load(&n)?;
        println!(
            "{:<20} {} symbols, {} triples",
            n,
            store.symbols.len(),
            store.triples.len()
        );
    }
    Ok(())
}

pub fn symbol_add(lang: &str, id: &str, glyph: &str, gloss: &str) -> Result<()> {
    let mut store = LangStore::load(lang)?;
    if store.symbols.contains_key(id) {
        bail!("symbol '{}' already exists in '{}'", id, lang);
    }
    store.symbols.insert(
        id.to_string(),
        Symbol {
            id: id.to_string(),
            glyph: Glyph::Literal(glyph.to_string()),
            gloss: gloss.to_string(),
        },
    );
    store.save()?;
    println!("added symbol {} ({}  \"{}\")", id, glyph, gloss);
    Ok(())
}

/// The generator counterpart to `symbol_add`: instead of typing a glyph,
/// derive one from a stroke grammar, optionally shaped by a relation to an
/// existing symbol in the same language.
pub fn symbol_generate(
    lang: &str,
    id: &str,
    gloss: &str,
    mirror_of: Option<&str>,
    axis: &str,
    extend_from: Option<&str>,
    reduce_from: Option<&str>,
) -> Result<()> {
    let mut store = LangStore::load(lang)?;
    if store.symbols.contains_key(id) {
        bail!("symbol '{}' already exists in '{}'", id, lang);
    }

    let existing_shapes: Vec<Shape> = store
        .symbols
        .values()
        .filter_map(|s| s.glyph.as_shape())
        .cloned()
        .collect();

    let shape = if let Some(src) = mirror_of {
        let base = store
            .require_symbol(src)?
            .glyph
            .as_shape()
            .ok_or_else(|| anyhow::anyhow!("'{}' has no generated shape to mirror", src))?;
        let ax = match axis {
            "vertical" => Axis::Vertical,
            _ => Axis::Horizontal,
        };
        base.mirror(ax)
    } else if let Some(src) = extend_from {
        let base = store
            .require_symbol(src)?
            .glyph
            .as_shape()
            .ok_or_else(|| anyhow::anyhow!("'{}' has no generated shape to extend", src))?;
        base.extend(hash_key(id))
    } else if let Some(src) = reduce_from {
        let base = store
            .require_symbol(src)?
            .glyph
            .as_shape()
            .ok_or_else(|| anyhow::anyhow!("'{}' has no generated shape to reduce", src))?;
        base.reduce(hash_key(id))
    } else {
        Shape::generate(id, &existing_shapes)
    };

    println!("{}", shape.to_glyph_art());
    store.symbols.insert(
        id.to_string(),
        Symbol {
            id: id.to_string(),
            glyph: Glyph::Generated(shape),
            gloss: gloss.to_string(),
        },
    );
    store.save()?;
    println!("generated symbol {} (\"{}\")", id, gloss);
    Ok(())
}

fn hash_key(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

pub fn symbol_list(lang: &str) -> Result<()> {
    let store = LangStore::load(lang)?;
    let mut ids: Vec<_> = store.symbols.keys().collect();
    ids.sort();
    for id in ids {
        let s = &store.symbols[id];
        match &s.glyph {
            Glyph::Literal(_) => {
                println!("{:<10} {:<8} {}", s.id, s.glyph.display_inline(), s.gloss);
            }
            Glyph::Generated(_) => {
                println!("{}  \"{}\"", s.id, s.gloss);
                println!("{}\n", s.glyph.display_art());
            }
        }
    }
    Ok(())
}

pub fn symbol_rm(lang: &str, id: &str) -> Result<()> {
    let mut store = LangStore::load(lang)?;
    if store.symbols.remove(id).is_none() {
        bail!("no symbol '{}' in '{}'", id, lang);
    }
    let before = store.triples.len();
    store
        .triples
        .retain(|t| t.subject != id && t.object != id);
    let removed = before - store.triples.len();
    store.save()?;
    println!("removed symbol '{}' ({} dangling triples also removed)", id, removed);
    Ok(())
}

pub fn relate(lang: &str, subject: &str, predicate: &str, object: &str) -> Result<()> {
    let mut store = LangStore::load(lang)?;
    store.require_symbol(subject)?;
    store.require_symbol(object)?;
    let t = Triple {
        subject: subject.to_string(),
        predicate: predicate.to_string(),
        object: object.to_string(),
    };
    if !store.triples.contains(&t) {
        store.triples.push(t);
        store.save()?;
    }
    println!("{} --{}--> {}", subject, predicate, object);
    Ok(())
}

pub fn compose(
    lang: &str,
    a: &str,
    b: &str,
    new_id: &str,
    glyph_override: Option<&str>,
    gloss: Option<String>,
    op: &str,
) -> Result<()> {
    let mut store = LangStore::load(lang)?;
    if store.symbols.contains_key(new_id) {
        bail!("symbol '{}' already exists", new_id);
    }
    let (ga, gb, new_glyph) = {
        let sa = store.require_symbol(a)?;
        let sb = store.require_symbol(b)?;
        let glyph = if let Some(g) = glyph_override {
            Glyph::Literal(g.to_string())
        } else {
            match (&sa.glyph, &sb.glyph) {
                (Glyph::Generated(sha), Glyph::Generated(shb)) => {
                    Glyph::Generated(sha.blend(shb))
                }
                (Glyph::Literal(ta), Glyph::Literal(tb)) => {
                    Glyph::Literal(format!("{}{}", ta, tb))
                }
                (Glyph::Generated(sh), Glyph::Literal(_))
                | (Glyph::Literal(_), Glyph::Generated(sh)) => Glyph::Generated(sh.clone()),
            }
        };
        (sa.gloss.clone(), sb.gloss.clone(), glyph)
    };
    let gloss = gloss.unwrap_or_else(|| format!("{}+{}", ga, gb));

    store.symbols.insert(
        new_id.to_string(),
        Symbol {
            id: new_id.to_string(),
            glyph: new_glyph,
            gloss,
        },
    );
    store.triples.push(Triple {
        subject: new_id.to_string(),
        predicate: "composed_from".into(),
        object: a.to_string(),
    });
    store.triples.push(Triple {
        subject: new_id.to_string(),
        predicate: "composed_from".into(),
        object: b.to_string(),
    });
    store.triples.push(Triple {
        subject: new_id.to_string(),
        predicate: "composed_via".into(),
        object: op.to_string(),
    });
    store.save()?;
    println!("composed {} from {} + {} (op={})", new_id, a, b, op);
    Ok(())
}

pub fn query(
    lang: &str,
    subject: Option<&str>,
    predicate: Option<&str>,
    object: Option<&str>,
) -> Result<()> {
    let store = LangStore::load(lang)?;
    let mut hits = 0;
    for t in &store.triples {
        if subject.map_or(true, |s| s == t.subject)
            && predicate.map_or(true, |p| p == t.predicate)
            && object.map_or(true, |o| o == t.object)
        {
            println!("{}  --{}-->  {}", t.subject, t.predicate, t.object);
            hits += 1;
        }
    }
    if hits == 0 {
        println!("(no matching triples)");
    }
    Ok(())
}

/// Prints descendants of `root` along composed_from edges (i.e. everything
/// transitively composed out of root), as an ascii tree.
pub fn tree(lang: &str, root: &str) -> Result<()> {
    let store = LangStore::load(lang)?;
    store.require_symbol(root)?;
    print_tree(&store, root, "", true);
    Ok(())
}

fn print_tree(store: &LangStore, node: &str, prefix: &str, is_last: bool) {
    let sym = store.symbols.get(node);
    let label = match sym {
        Some(s) => format!("{} {} \"{}\"", s.id, s.glyph.display_inline(), s.gloss),
        None => node.to_string(),
    };
    let branch = if prefix.is_empty() {
        "".to_string()
    } else if is_last {
        "└── ".to_string()
    } else {
        "├── ".to_string()
    };
    println!("{}{}{}", prefix, branch, label);

    let children: Vec<&str> = store
        .triples
        .iter()
        .filter(|t| t.predicate == "composed_from" && t.object == node)
        .map(|t| t.subject.as_str())
        .collect();

    let child_prefix = if prefix.is_empty() {
        "".to_string()
    } else if is_last {
        format!("{}    ", prefix)
    } else {
        format!("{}│   ", prefix)
    };
    let next_prefix = if prefix.is_empty() {
        "    ".to_string()
    } else {
        child_prefix
    };

    for (i, child) in children.iter().enumerate() {
        print_tree(store, child, &next_prefix, i == children.len() - 1);
    }
}

pub fn fork(lang: &str, new_name: &str) -> Result<()> {
    if LangStore::exists(new_name) {
        bail!("language '{}' already exists", new_name);
    }
    let mut store = LangStore::load(lang)?;
    store.name = new_name.to_string();
    store.save()?;
    println!("forked '{}' -> '{}'", lang, new_name);
    Ok(())
}

pub fn diff(lang_a: &str, lang_b: &str) -> Result<()> {
    let a = LangStore::load(lang_a)?;
    let b = LangStore::load(lang_b)?;

    let ids_a: HashSet<_> = a.symbols.keys().cloned().collect();
    let ids_b: HashSet<_> = b.symbols.keys().cloned().collect();

    for id in ids_b.difference(&ids_a) {
        let s = &b.symbols[id];
        println!("+ symbol {} {} \"{}\"", s.id, s.glyph.display_inline(), s.gloss);
    }
    for id in ids_a.difference(&ids_b) {
        let s = &a.symbols[id];
        println!("- symbol {} {} \"{}\"", s.id, s.glyph.display_inline(), s.gloss);
    }
    for id in ids_a.intersection(&ids_b) {
        let sa = &a.symbols[id];
        let sb = &b.symbols[id];
        // Glyph has no PartialEq (Shape carries a seed/stroke list); comparing
        // the debug representation is a cheap, good-enough structural check.
        let glyph_changed = format!("{:?}", sa.glyph) != format!("{:?}", sb.glyph);
        if glyph_changed || sa.gloss != sb.gloss {
            println!(
                "~ symbol {}: ({} \"{}\") -> ({} \"{}\")",
                id,
                sa.glyph.display_inline(),
                sa.gloss,
                sb.glyph.display_inline(),
                sb.gloss
            );
        }
    }

    let ta: HashSet<_> = a.triples.iter().cloned().collect();
    let tb: HashSet<_> = b.triples.iter().cloned().collect();
    for t in tb.difference(&ta) {
        println!("+ triple {} --{}--> {}", t.subject, t.predicate, t.object);
    }
    for t in ta.difference(&tb) {
        println!("- triple {} --{}--> {}", t.subject, t.predicate, t.object);
    }
    Ok(())
}

pub fn gc(lang: &str, dry_run: bool) -> Result<()> {
    let mut store = LangStore::load(lang)?;
    let referenced: HashSet<&str> = store
        .triples
        .iter()
        .flat_map(|t| vec![t.subject.as_str(), t.object.as_str()])
        .collect();

    let orphans: Vec<String> = store
        .symbols
        .keys()
        .filter(|id| !referenced.contains(id.as_str()))
        .cloned()
        .collect();

    if orphans.is_empty() {
        println!("nothing to collect — every symbol is referenced by some triple");
        return Ok(());
    }

    for id in &orphans {
        println!(
            "{} orphan symbol: {}",
            if dry_run { "would remove" } else { "removing" },
            id
        );
    }

    if !dry_run {
        for id in &orphans {
            store.symbols.remove(id);
        }
        store.save()?;
    }
    Ok(())
}

pub fn export(lang: &str) -> Result<()> {
    let store = LangStore::load(lang)?;
    println!("@prefix : <https://langsmith.local/{}#> .\n", store.name);

    let mut ids: Vec<_> = store.symbols.keys().collect();
    ids.sort();
    for id in ids {
        let s = &store.symbols[id];
        match &s.glyph {
            Glyph::Literal(text) => println!(
                ":{} a :Symbol ;\n    :glyphKind \"literal\" ;\n    :glyph \"{}\" ;\n    :gloss \"{}\" .\n",
                s.id, text, s.gloss
            ),
            Glyph::Generated(shape) => println!(
                ":{} a :Symbol ;\n    :glyphKind \"generated\" ;\n    :strokeCount {} ;\n    :glyphSeed {} ;\n    :gloss \"{}\" .\n",
                s.id, shape.strokes.len(), shape.seed, s.gloss
            ),
        }
    }
    for t in &store.triples {
        println!(":{} :{} :{} .", t.subject, t.predicate, t.object);
    }
    Ok(())
}

pub fn glyph_render(lang: &str, id: &str, out_path: &str) -> Result<()> {
    let store = LangStore::load(lang)?;
    let sym = store.require_symbol(id)?;
    let shape = sym
        .glyph
        .as_shape()
        .ok_or_else(|| anyhow::anyhow!("'{}' has a literal glyph, not a generated shape", id))?;
    fs::write(out_path, shape.to_svg())?;
    println!("wrote {}", out_path);
    Ok(())
}

/// A little "font sheet": every generated glyph in the language, laid out in
/// a grid with its id/gloss underneath -- the closest thing here to an
/// exportable specimen of the whole script at a glance.
pub fn glyph_sheet(lang: &str, out_path: &str) -> Result<()> {
    let store = LangStore::load(lang)?;
    let mut ids: Vec<&String> = store
        .symbols
        .keys()
        .filter(|id| store.symbols[*id].glyph.as_shape().is_some())
        .collect();
    ids.sort();

    if ids.is_empty() {
        bail!("no generated glyphs in '{}' to sheet", lang);
    }

    let cell = 160.0;
    let pad = 30.0;
    let cols = (ids.len() as f64).sqrt().ceil() as usize;
    let rows = (ids.len() + cols - 1) / cols;
    let width = cell * cols as f64 + 2.0 * pad;
    let height = cell * rows as f64 + 2.0 * pad;

    let mut body = String::new();
    for (i, id) in ids.iter().enumerate() {
        let s = &store.symbols[*id];
        let shape = s.glyph.as_shape().unwrap();
        let col = (i % cols) as f64;
        let row = (i / cols) as f64;
        let (ox, oy) = (pad + col * cell, pad + row * cell);
        body.push_str(&format!(
            "<g transform=\"translate({:.1},{:.1})\">\n{}</g>\n",
            ox, oy,
            shape.to_svg_fragment(cell - 30.0, 22.0)
        ));
        let gloss: String = if s.gloss.chars().count() > 16 {
            format!("{}…", s.gloss.chars().take(15).collect::<String>())
        } else {
            s.gloss.clone()
        };
        body.push_str(&format!(
            "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"13\" fill=\"black\" text-anchor=\"middle\" font-family=\"monospace\">{}</text>\n\
             <text x=\"{:.1}\" y=\"{:.1}\" font-size=\"10\" fill=\"#555\" text-anchor=\"middle\" font-family=\"monospace\">\"{}\"</text>\n",
            ox + cell / 2.0, oy + cell - 22.0, s.id,
            ox + cell / 2.0, oy + cell - 8.0, gloss
        ));
    }

    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\">\n<rect width=\"{w}\" height=\"{h}\" fill=\"white\"/>\n{body}</svg>\n",
        w = width,
        h = height,
        body = body
    );
    fs::write(out_path, svg)?;
    println!("wrote {} ({} glyphs)", out_path, ids.len());
    Ok(())
}

pub fn glyph_preview(lang: &str, id: &str, braille: bool) -> Result<()> {
    let store = LangStore::load(lang)?;
    let sym = store.require_symbol(id)?;
    match &sym.glyph {
        Glyph::Literal(text) => println!("{} is a literal glyph: {}", id, text),
        Glyph::Generated(shape) => {
            if braille {
                println!("{}", shape.to_braille(12, 12));
            } else {
                println!("{}", shape.to_glyph_art());
            }
        }
    }
    Ok(())
}

