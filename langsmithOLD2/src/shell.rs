use crate::model::{valid_name, LangStore};
use anyhow::{bail, Result};
use std::env;

/// The environment variable that holds the active language for this shell.
/// Env-based (like Python's VIRTUAL_ENV) so activation is per-terminal.
const ACTIVE_VAR: &str = "LANGSMITH_ACTIVE";

/// Set by the shell wrapper so the binary knows its stdout is going to be
/// `eval`ed rather than shown to a human.
const INTEGRATION_VAR: &str = "LANGSMITH_SHELL_INTEGRATION";

/// Explicit `--lang` wins; otherwise fall back to the activated language.
pub fn resolve_lang(explicit: Option<&str>) -> Result<String> {
    if let Some(l) = explicit {
        return Ok(l.to_string());
    }
    match env::var(ACTIVE_VAR) {
        Ok(l) if !l.is_empty() => Ok(l),
        _ => bail!(
            "no language selected: run `langsmith activate <name>` or pass --lang <name>"
        ),
    }
}

fn require_integration() -> Result<()> {
    if env::var(INTEGRATION_VAR).is_err() {
        bail!(
            "shell integration isn't installed, so I can't change your shell.\n\
             \n  Try it in this shell:   eval \"$(langsmith shell-init)\"\n  \
             Make it permanent:      add that line to ~/.bashrc or ~/.zshrc"
        );
    }
    Ok(())
}

/// A program can't modify its parent shell, so `activate` prints shell code
/// on stdout and the wrapper function (see `shell_init`) evals it. Human
/// messages go to stderr so they aren't swallowed by the eval.
pub fn activate(name: &str) -> Result<()> {
    if !valid_name(name) {
        bail!("invalid language name '{}'", name);
    }
    if !LangStore::exists(name) {
        bail!(
            "language '{}' not found in this directory (create it with `langsmith new {}`)",
            name,
            name
        );
    }
    require_integration()?;

    // Remember the original prompt once, so switching between languages
    // doesn't stack "(a) (b) $ " and deactivate restores it exactly.
    println!(
        "if [ -z \"${{_LANGSMITH_OLD_PS1+x}}\" ]; then _LANGSMITH_OLD_PS1=\"$PS1\"; fi\n\
         export {var}='{name}'\n\
         PS1=\"({name}) ${{_LANGSMITH_OLD_PS1}}\"",
        var = ACTIVE_VAR,
        name = name
    );
    eprintln!("activated '{}'", name);
    Ok(())
}

pub fn deactivate() -> Result<()> {
    require_integration()?;
    match env::var(ACTIVE_VAR) {
        Ok(name) if !name.is_empty() => {
            println!(
                "if [ -n \"${{_LANGSMITH_OLD_PS1+x}}\" ]; then PS1=\"$_LANGSMITH_OLD_PS1\"; unset _LANGSMITH_OLD_PS1; fi\n\
                 unset {}",
                ACTIVE_VAR
            );
            eprintln!("deactivated '{}'", name);
        }
        _ => eprintln!("no language is active"),
    }
    Ok(())
}

/// The wrapper function to `eval` from ~/.bashrc or ~/.zshrc. Only
/// `activate`/`deactivate` need it; everything else passes straight through.
pub fn shell_init() {
    println!(
        r#"langsmith() {{
  case "$1" in
    activate|deactivate)
      local __ls_out
      __ls_out="$(LANGSMITH_SHELL_INTEGRATION=1 command langsmith "$@")" || return $?
      eval "$__ls_out"
      ;;
    *)
      command langsmith "$@"
      ;;
  esac
}}"#
    );
}
