mod commands;
mod glyph;
mod model;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "lang",
    about = "Create and manipulate disposable languages as symbol graphs (RDF-triple-flavored)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new, empty language
    New { name: String },
    /// List all languages in this directory
    List,
    /// Add, list, or remove symbols
    Symbol {
        #[command(subcommand)]
        action: SymbolAction,
    },
    /// Add a (subject predicate object) triple relating two symbols
    Relate {
        lang: String,
        subject: String,
        predicate: String,
        object: String,
    },
    /// Derive a new symbol from two existing ones
    Compose {
        lang: String,
        a: String,
        b: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        glyph: Option<String>,
        #[arg(long)]
        gloss: Option<String>,
        #[arg(long, default_value = "blend")]
        op: String,
    },
    /// Query triples, optionally filtered by subject/predicate/object
    Query {
        lang: String,
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        predicate: Option<String>,
        #[arg(long)]
        object: Option<String>,
    },
    /// Show everything composed (transitively) from a symbol, as a tree
    Tree { lang: String, root: String },
    /// Copy a language under a new name
    Fork { lang: String, new_name: String },
    /// Compare two languages' symbols and triples
    Diff { lang_a: String, lang_b: String },
    /// Remove symbols with no relations at all
    Gc {
        lang: String,
        #[arg(long)]
        dry_run: bool,
    },
    /// Dump the language as turtle-ish RDF text
    Export { lang: String },
    /// Render, sheet, or preview generated glyph shapes
    Glyph {
        #[command(subcommand)]
        action: GlyphAction,
    },
}

#[derive(Subcommand)]
enum GlyphAction {
    /// Save one glyph as a standalone SVG file
    Render {
        lang: String,
        id: String,
        #[arg(long)]
        svg: String,
    },
    /// Save every generated glyph in the language as one SVG sheet
    Sheet {
        lang: String,
        #[arg(long)]
        svg: String,
    },
    /// Print the glyph on its 3x3 grid in the terminal
    Preview {
        lang: String,
        id: String,
        /// Show the finer braille sub-pixel rendering instead
        #[arg(long)]
        braille: bool,
    },
}

#[derive(Subcommand)]
enum SymbolAction {
    /// Add a symbol with a hand-typed glyph (literal text/unicode char)
    Add {
        lang: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        glyph: String,
        #[arg(long)]
        gloss: String,
    },
    /// Add a symbol whose glyph is generated, optionally shaped by a relation
    /// to an existing symbol (mirror = opposite, extend = more specific,
    /// reduce = more abstract). With none of those, it's pure seeded random.
    Generate {
        lang: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        gloss: String,
        #[arg(long)]
        mirror_of: Option<String>,
        #[arg(long, default_value = "horizontal")]
        axis: String,
        #[arg(long)]
        extend_from: Option<String>,
        #[arg(long)]
        reduce_from: Option<String>,
    },
    List {
        lang: String,
    },
    Rm {
        lang: String,
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::New { name } => commands::new(&name),
        Commands::List => commands::list(),
        Commands::Symbol { action } => match action {
            SymbolAction::Add { lang, id, glyph, gloss } => {
                commands::symbol_add(&lang, &id, &glyph, &gloss)
            }
            SymbolAction::Generate {
                lang,
                id,
                gloss,
                mirror_of,
                axis,
                extend_from,
                reduce_from,
            } => commands::symbol_generate(
                &lang,
                &id,
                &gloss,
                mirror_of.as_deref(),
                &axis,
                extend_from.as_deref(),
                reduce_from.as_deref(),
            ),
            SymbolAction::List { lang } => commands::symbol_list(&lang),
            SymbolAction::Rm { lang, id } => commands::symbol_rm(&lang, &id),
        },
        Commands::Relate {
            lang,
            subject,
            predicate,
            object,
        } => commands::relate(&lang, &subject, &predicate, &object),
        Commands::Compose {
            lang,
            a,
            b,
            id,
            glyph,
            gloss,
            op,
        } => commands::compose(&lang, &a, &b, &id, glyph.as_deref(), gloss, &op),
        Commands::Query {
            lang,
            subject,
            predicate,
            object,
        } => commands::query(
            &lang,
            subject.as_deref(),
            predicate.as_deref(),
            object.as_deref(),
        ),
        Commands::Tree { lang, root } => commands::tree(&lang, &root),
        Commands::Fork { lang, new_name } => commands::fork(&lang, &new_name),
        Commands::Diff { lang_a, lang_b } => commands::diff(&lang_a, &lang_b),
        Commands::Gc { lang, dry_run } => commands::gc(&lang, dry_run),
        Commands::Export { lang } => commands::export(&lang),
        Commands::Glyph { action } => match action {
            GlyphAction::Render { lang, id, svg } => commands::glyph_render(&lang, &id, &svg),
            GlyphAction::Sheet { lang, svg } => commands::glyph_sheet(&lang, &svg),
            GlyphAction::Preview { lang, id, braille } => {
                commands::glyph_preview(&lang, &id, braille)
            }
        },
    };

    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
