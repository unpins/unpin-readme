# unpin-readme

A single self-contained markdown reader, built natively for Linux, macOS, and Windows — it renders the README that [unpins](https://unpins.org) programs carry inside themselves, paged in your terminal. It is the helper behind the `unpin readme` verb, fetched on demand and never placed on `PATH`.

[![CI](https://github.com/unpins/unpin-readme/actions/workflows/unpin-readme.yml/badge.svg)](https://github.com/unpins/unpin-readme/actions)
![Linux](https://img.shields.io/badge/Linux-✓-success?logo=linux&logoColor=white)
![macOS](https://img.shields.io/badge/macOS-✓-success?logo=apple&logoColor=white)
![Windows](https://img.shields.io/badge/Windows-✓-success?logo=windows&logoColor=white)

Part of the [unpins](https://unpins.org) project.

## What it is

unpins binaries can embed their own README (`unpin/readme/README.md` in an appended ZIP). `unpin` itself knows nothing about markdown — it only exposes those bytes through `unpin bundle list|dump`. This package is the other half: it renders the markdown to styled terminal text and pages it in-process with [termimad](https://crates.io/crates/termimad)'s reflowing view — no external `less`, no companion files. Until READMEs are embedded, it falls back to fetching the README straight from the package's GitHub repo.

## Usage

```bash
unpin readme htop            # render unpins/htop's README
unpin readme BurntSushi/ripgrep   # any owner/repo
```

`unpin readme` fetches and runs this package on demand (cached after first use, never linked onto `PATH` — it's a verb, not a command you install, so it can never shadow anything). A bare name resolves to `unpins/<name>`; an explicit `owner/repo` is used as-is.

## Build locally

```bash
nix build github:unpins/unpin-readme
./result/bin/unpin-readme htop
echo '# hello' | ./result/bin/unpin-readme -   # render markdown from stdin
```

The first invocation will offer to add the [unpins.cachix.org](https://unpins.cachix.org) substituter so most pulls come pre-built.

## Build notes

- **It is just a Rust crate — no cosmo.** Unlike `unpin-man` (a C front-end that needs Cosmopolitan to get POSIX `fork`/`exec` on Windows), this renderer is pure Rust over [crossterm](https://crates.io/crates/crossterm), so it cross-compiles to **every** target the way `unpin` itself does: **Windows is a real mingw `.exe`**, the musl crosses come from rustup's `rust-std`, darwin links libiconv statically. This flake mirrors [unpins/unpin](https://github.com/unpins/unpin)'s `flake.nix`.
- **Data source.** Primary path is the embedded bundle via `unpin bundle dump <pkg> unpin/readme/README.md`; it finds unpin via `$UNPIN_SELF` (exported by `unpin run`/`unpin readme`), else `unpin` on `$PATH`. On a miss it fetches `…/repos/<owner>/<repo>/readme` from the GitHub API (honoring `GITHUB_TOKEN`/`GH_TOKEN`).
- **Embedded pager, not `less`.** termimad's `MadView` pages in-process — and re-runs the markdown formatter at the live width on resize, so text reflows instead of staying wrapped to the old width. Paging works identically on Windows and on minimal systems with no `less`. When stdout isn't a tty, it prints the rendered text and exits.
- **Raw HTML is a known rough edge.** termimad passes raw HTML blocks through literally (e.g. a `<div align="center">` banner), so HTML-heavy READMEs render noisily. Stripping HTML blocks before rendering is a planned polish.
- **`doCheck = false`.** The unit tests (`split_repo`) run in CI on the native target; the cross builds skip the test phase, matching `unpin`.
