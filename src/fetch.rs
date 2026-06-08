//! Repo-fetch fallback: pull a package's README straight from its GitHub repo.
//! Used until READMEs are embedded into bundles (the `unpin/readme/*` entries);
//! once they are, `bundle dump` wins and this path is only hit for packages
//! built before embedding landed.

/// Fetch `pkg`'s README. A bare name resolves to `unpins/<name>`; an explicit
/// `owner/repo` is used as-is. Hits the API's readme endpoint so it follows the
/// default branch and finds the file regardless of casing or extension.
pub fn repo_readme(pkg: &str) -> Result<String, String> {
    let (owner, repo) = split_repo(pkg);
    let url = format!("https://api.github.com/repos/{owner}/{repo}/readme");
    let mut req = minreq::get(&url)
        // `raw` media type returns the file bytes directly, not base64 JSON.
        .with_header("Accept", "application/vnd.github.raw+json")
        .with_header("User-Agent", "unpin-readme")
        .with_timeout(30);
    if let Some(tok) = token() {
        req = req.with_header("Authorization", format!("Bearer {tok}"));
    }

    let resp = req
        .send()
        .map_err(|e| format!("fetching {owner}/{repo} README: {e}"))?;
    match resp.status_code {
        200 => resp
            .as_str()
            .map(str::to_owned)
            .map_err(|e| format!("decoding README: {e}")),
        404 => Err(format!("no README found for {owner}/{repo}")),
        403 => Err(format!(
            "GitHub rate-limited the README fetch for {owner}/{repo} \
             (set GITHUB_TOKEN to raise the 60/h limit)"
        )),
        c => Err(format!("GitHub returned HTTP {c} for {owner}/{repo} README")),
    }
}

/// `owner/repo` → `(owner, repo)`; a bare name → `("unpins", name)`. Any
/// `@version` suffix is dropped — a README isn't versioned here.
fn split_repo(pkg: &str) -> (String, String) {
    let pkg = pkg.split('@').next().unwrap_or(pkg);
    match pkg.split_once('/') {
        Some((owner, repo)) => (owner.to_owned(), repo.to_owned()),
        None => ("unpins".to_owned(), pkg.to_owned()),
    }
}

/// GitHub token from the same env vars unpin honors, raising the API limit from
/// 60/h to 5000/h. Empty values are treated as unset.
fn token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::split_repo;

    #[test]
    fn bare_name_defaults_to_the_unpins_owner() {
        assert_eq!(split_repo("htop"), ("unpins".into(), "htop".into()));
    }

    #[test]
    fn explicit_owner_repo_is_kept() {
        assert_eq!(
            split_repo("BurntSushi/ripgrep"),
            ("BurntSushi".into(), "ripgrep".into())
        );
    }

    #[test]
    fn version_suffix_is_stripped() {
        assert_eq!(split_repo("htop@1.2.3"), ("unpins".into(), "htop".into()));
        assert_eq!(
            split_repo("owner/repo@v9"),
            ("owner".into(), "repo".into())
        );
    }
}
