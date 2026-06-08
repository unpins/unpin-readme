//! `unpin-readme` — the `unpin readme` helper verb. Renders an unpins program's
//! README markdown in the terminal, paged. Reached only via `unpin readme <pkg>`
//! and never linked onto PATH (see docs/helper-verbs.md); unpin's run-dispatch
//! resolves it as `unpins/unpin-readme` on a bare-name 404.
//!
//! unpin knows nothing about markdown. This is the other half: given a package,
//! it reads the embedded `unpin/readme/README.md` via the stable
//! `unpin bundle dump` interface (offline, fast), falling back to a fetch from
//! the upstream repo until READMEs are embedded — then renders and pages the
//! markdown with termimad's reflowing `MadView`.

use std::io::Read;
use std::process::{Command, ExitCode};

mod fetch;
mod render;

/// `--help` text. Leads with the fact that this is the `unpin readme` verb —
/// normally reached *through* unpin, not run on its own — since that is how
/// nearly everyone meets it.
const HELP: &str = "\
unpin-readme — render an unpins program's README in your terminal

This is the helper behind the `unpin readme` verb. You normally reach it
through unpin, which fetches and runs it on demand and never puts it on PATH:

    unpin readme <pkg>           render unpins/<pkg>'s README
    unpin readme <owner>/<repo>  render any GitHub repo's README

Run directly it behaves the same:

    unpin-readme <pkg>
    unpin-readme -               read markdown from stdin

Options:
    -h, --help     print this help and exit
    -V, --version  print version and exit

Pager keys: q quit · ↑/↓ j/k scroll · Space/b page · g/G top/bottom
";

fn main() -> ExitCode {
    // unpin execs us as `unpin-readme <pkg>` (whatever followed `unpin readme`).
    let Some(target) = std::env::args().nth(1) else {
        eprintln!("usage: unpin readme <pkg>");
        return ExitCode::from(2);
    };

    // Flags win over a package name (no real package is called `--help`).
    match target.as_str() {
        "-h" | "--help" => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        "-V" | "--version" => {
            println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    let md = match load(&target) {
        Ok(md) => md,
        Err(e) => {
            eprintln!("unpin-readme: {e}");
            return ExitCode::FAILURE;
        }
    };
    if md.trim().is_empty() {
        eprintln!("unpin-readme: {target} has no README");
        return ExitCode::FAILURE;
    }
    render::page(&md);
    ExitCode::SUCCESS
}

/// Resolve the markdown for `target`, cheapest source first:
/// `-` (stdin, for piping/testing), the package's embedded bundle, then a repo
/// fetch as a fallback.
fn load(target: &str) -> Result<String, String> {
    if target == "-" {
        let mut s = String::new();
        std::io::stdin()
            .read_to_string(&mut s)
            .map_err(|e| format!("reading stdin: {e}"))?;
        return Ok(s);
    }
    // 1) Embedded bundle — symmetric with man; the fast, offline path. Empty
    //    output (entry or whole bundle absent) just means "not embedded yet".
    if let Some(md) = bundle_dump(target)? {
        return Ok(md);
    }
    // 2) Fallback — fetch from the upstream repo.
    fetch::repo_readme(target)
}

/// Read `unpin/readme/README.md` out of the package's embedded bundle by
/// shelling back to `unpin bundle dump`. `Ok(None)` in every "no bundle here"
/// case so the caller falls through to the repo fetch: when the entry (or the
/// whole bundle) is absent (`unpin bundle dump` prints nothing and exits 0),
/// and when `unpin` itself isn't reachable (not installed, not on `PATH`, and
/// `$UNPIN_SELF` unset or pointing nowhere) — running us standalone is fine,
/// the network fetch is the safety net.
///
/// Prefer `$UNPIN_SELF` (exported by `unpin run`/`unpin readme` to the exact
/// running binary) over a bare `unpin` on `PATH` — the same handshake the man
/// front-end uses, so the right unpin is reached even when it isn't on `PATH`.
fn bundle_dump(pkg: &str) -> Result<Option<String>, String> {
    let unpin = std::env::var_os("UNPIN_SELF").unwrap_or_else(|| "unpin".into());
    let out = match Command::new(unpin)
        .args(["bundle", "dump", pkg, "unpin/readme/README.md"])
        .output()
    {
        Ok(out) => out,
        // unpin isn't there to ask — no embedded bundle is reachable, so let
        // the repo fetch take over instead of failing the whole command.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("running `unpin bundle dump`: {e}")),
    };
    if !out.status.success() || out.stdout.is_empty() {
        return Ok(None);
    }
    String::from_utf8(out.stdout)
        .map(Some)
        .map_err(|_| "embedded README is not valid UTF-8".to_owned())
}
