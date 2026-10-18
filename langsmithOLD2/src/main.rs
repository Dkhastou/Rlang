mod commands;
mod glyph;
mod model;
mod shell;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "langsmith",
    about = "Create and manipulate disposable languages as symbol graphs (RDF-triple-flavored)"
)]
struct Cli {
    /// Language to operate on; overrides the one set by `activate`
    #[arg(short, long, global = true, value_name = "NAME")]
    lang: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new, empty language
    New { name: String },
    /// List all languages in this directory
    List,
    /// Make a language the default for this shell and show it in the prompt
    Activate { name: String },
    /// Clear the active language and restore the prompt
    Deactivate,
    /// Print the shell function that makes activate/deactivate work
    ShellInit,
    /// Add, list, or remove symbols
    Symbol {
        #[command(subcommand)]
        action: SymbolAction,
    },
    /// Add a (subject predicate object) triple relating two symbols
    Relate {
        subject: String,
        predicate: String,
        object: String,
    },
    /// Derive a new symbol from two existing ones
    Compose {
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
        #[arg(long)]
        subject: Option<String>,
        #[arg(long)]
        predicate: Option<String>,
        #[arg(long)]
        object: Option<String>,
    },
    /// Show everything composed (transitively) from a symbol, as a tree
    Tree { root: String },
    /// Copy the language under a new name
    Fork { new_name: String },
    /// Compare the language against another (+ = only in OTHER, - = only here)
    Diff { other: String },
    /// Remove symbols with no relations at all
    Gc {
        #[arg(long)]
        dry_run: bool,
    },
    /// Dump the language as turtle-ish RDF text
    Export,
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
        id: String,
        #[arg(long)]
        svg: String,
    },
    /// Save every generated glyph in the language as one SVG sheet
    Sheet {
        #[arg(long)]
        svg: String,
    },
    /// Print the glyph on its 3x3 grid in the terminal
    Preview {
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
    List,
    Rm {
        id: String,
    },
}

fn run(cli: Cli) -> anyhow::Result<()> {
    // Resolved lazily so commands like `new`, `list` and `activate` don't
    // require a language to be selected.
    let explicit = cli.lang;
    let lang = || shell::resolve_lang(explicit.as_deref());

    match cli.command {
        Commands::New { name } => commands::new(&name),
        Commands::List => commands::list(),
        Commands::Activate { name } => shell::activate(&name),
        Commands::Deactivate => shell::deactivate(),
        Commands::ShellInit => {
            shell::shell_init();
            Ok(())
        }
        Commands::Symbol { action } => match action {
            SymbolAction::Add { id, glyph, gloss } => {
                commands::symbol_add(&lang()?, &id, &glyph, &gloss)
            }
            SymbolAction::Generate {
                id,
                gloss,
                mirror_of,
                axis,
                extend_from,
                reduce_from,
            } => commands::symbol_generate(
                &lang()?,
                &id,
                &gloss,
                mirror_of.as_deref(),
                &axis,
                extend_from.as_deref(),
                reduce_from.as_deref(),
            ),
            SymbolAction::List => commands::symbol_list(&lang()?),
            SymbolAction::Rm { id } => commands::symbol_rm(&lang()?, &id),
        },
        Commands::Relate {
            subject,
            predicate,
            object,
        } => commands::relate(&lang()?, &subject, &predicate, &object),
        Commands::Compose {
            a,
            b,
            id,
            glyph,
            gloss,
            op,
        } => commands::compose(&lang()?, &a, &b, &id, glyph.as_deref(), gloss, &op),
        Commands::Query {
            subject,
            predicate,
            object,
        } => commands::query(
            &lang()?,
            subject.as_deref(),
            predicate.as_deref(),
            object.as_deref(),
        ),
        Commands::Tree { root } => commands::tree(&lang()?, &root),
        Commands::Fork { new_name } => commands::fork(&lang()?, &new_name),
        Commands::Diff { other } => commands::diff(&lang()?, &other),
        Commands::Gc { dry_run } => commands::gc(&lang()?, dry_run),
        Commands::Export => commands::export(&lang()?),
        Commands::Glyph { action } => match action {
            GlyphAction::Render { id, svg } => commands::glyph_render(&lang()?, &id, &svg),
            GlyphAction::Sheet { svg } => commands::glyph_sheet(&lang()?, &svg),
            GlyphAction::Preview { id, braille } => {
                commands::glyph_preview(&lang()?, &id, braille)
            }
        },
    }
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
