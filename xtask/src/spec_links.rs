//! `cargo xtask spec-links`: checks that every relative link in the spec's Markdown files, and
//! every link from the std API doc into the spec, names an existing file, and that every
//! `#fragment` into a Markdown file names one of its headings.

use crate::spec_rules::report_located;
use quince_spec::links::{self, Link};
use quince_spec::{FileError, SPEC_DIR};
use std::collections::{BTreeSet, HashMap};
use std::path::Path;

/// Files outside `spec/` whose links into `spec/` are checked, relative to the repo root.
const LINKING_IN: [&str; 1] = ["std/API.md"];

pub fn check(root: &Path) -> Result<(), String> {
    let mut sources = Vec::new();
    markdown_files(root, SPEC_DIR, &mut sources)
        .map_err(|e| format!("can't read {SPEC_DIR}: {e}"))?;
    sources.sort();
    let mut anchors = Anchors::default();
    let mut errors = Vec::new();
    let mut checked = 0;
    let inside: Vec<_> = sources.iter().map(|path| (path.as_str(), false)).collect();
    let outside = LINKING_IN
        .iter()
        .filter(|path| root.join(path).is_file())
        .map(|path| (*path, true));
    for (path, only_into_spec) in inside.into_iter().chain(outside) {
        let text = std::fs::read_to_string(root.join(path))
            .map_err(|e| format!("can't read {path}: {e}"))?;
        for link in links::links(&text) {
            if link.is_external() {
                continue;
            }
            let target = links::resolve(path, link.path());
            if only_into_spec
                && !target
                    .as_deref()
                    .is_some_and(|t| t.starts_with(&format!("{SPEC_DIR}/")))
            {
                continue;
            }
            checked += 1;
            if let Some(message) = problem(root, path, &link, target, &mut anchors) {
                errors.push(FileError {
                    path: path.to_string(),
                    line: link.line,
                    message,
                });
            }
        }
    }
    report_located(&errors);
    if !errors.is_empty() {
        return Err(format!("{} broken link(s)", errors.len()));
    }
    eprintln!(
        "spec-links: {checked} relative link(s) in {} file(s), all resolve",
        sources.len()
    );
    Ok(())
}

/// What's wrong with one link, if anything.
fn problem(
    root: &Path,
    from: &str,
    link: &Link,
    target: Option<String>,
    anchors: &mut Anchors,
) -> Option<String> {
    let Some(target) = target else {
        return Some(format!(
            "the link `{}` climbs above the repo root",
            link.dest
        ));
    };
    if !root.join(&target).exists() {
        return Some(format!(
            "the link `{}` names {target}, which doesn't exist",
            link.dest
        ));
    }
    let fragment = link.fragment().filter(|f| !f.is_empty())?;
    if !target.ends_with(".md") {
        return None;
    }
    let found = anchors.of(root, &target);
    if found.contains(fragment) {
        return None;
    }
    let place = if target == from {
        "this file".to_string()
    } else {
        target.clone()
    };
    let near: Vec<&str> = found
        .iter()
        .filter(|a| a.contains(fragment) || fragment.contains(a.as_str()))
        .map(String::as_str)
        .take(3)
        .collect();
    let hint = if near.is_empty() {
        String::new()
    } else {
        format!("; did you mean `#{}`?", near.join("`, `#"))
    };
    Some(format!(
        "the link `{}` names the fragment `#{fragment}`, but no heading in {place} has that anchor{hint}",
        link.dest
    ))
}

/// The heading anchors of each Markdown file, read once.
#[derive(Default)]
struct Anchors(HashMap<String, BTreeSet<String>>);

impl Anchors {
    fn of(&mut self, root: &Path, path: &str) -> &BTreeSet<String> {
        self.0.entry(path.to_string()).or_insert_with(|| {
            let text = std::fs::read_to_string(root.join(path)).unwrap_or_default();
            links::headings(&text)
                .into_iter()
                .map(|heading| heading.anchor)
                .collect()
        })
    }
}

/// Collects the `.md` files under `relative`, as repo-relative paths.
fn markdown_files(root: &Path, relative: &str, out: &mut Vec<String>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root.join(relative))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{relative}/{name}");
        if entry.file_type()?.is_dir() {
            markdown_files(root, &path, out)?;
        } else if name.ends_with(".md") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = std::env::temp_dir()
            .join(format!("xtask-spec-links-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root
    }

    fn link(dest: &str) -> Link {
        Link {
            dest: dest.into(),
            line: 1,
        }
    }

    fn check_link(root: &Path, from: &str, dest: &str) -> Option<String> {
        let link = link(dest);
        let target = links::resolve(from, link.path());
        problem(root, from, &link, target, &mut Anchors::default())
    }

    #[test]
    fn links_and_fragments() {
        let root = tree(
            "fragments",
            &[
                (
                    "spec/reference/effects.md",
                    "# Effects\n\n## Handlers\n\n## The `?` operator\n",
                ),
                ("spec/README.md", "# Spec\n"),
                ("docs/plan.txt", "plan\n"),
            ],
        );
        let from = "spec/README.md";
        assert_eq!(check_link(&root, from, "reference/effects.md"), None);
        assert_eq!(
            check_link(&root, from, "reference/effects.md#handlers"),
            None
        );
        assert_eq!(
            check_link(&root, from, "reference/effects.md#the--operator"),
            None
        );
        assert_eq!(check_link(&root, from, "../docs/plan.txt#L3"), None);
        assert_eq!(check_link(&root, from, "reference"), None);
        assert_eq!(check_link(&root, from, "#spec"), None);
        assert_eq!(
            check_link(&root, from, "reference/missing.md").as_deref(),
            Some(
                "the link `reference/missing.md` names spec/reference/missing.md, which doesn't exist"
            )
        );
        assert_eq!(
            check_link(&root, from, "reference/effects.md#handler").as_deref(),
            Some(
                "the link `reference/effects.md#handler` names the fragment `#handler`, but no heading in spec/reference/effects.md has that anchor; did you mean `#handlers`?"
            )
        );
        assert_eq!(
            check_link(&root, from, "#nope").as_deref(),
            Some(
                "the link `#nope` names the fragment `#nope`, but no heading in this file has that anchor"
            )
        );
        assert_eq!(
            check_link(&root, from, "../../up.md").as_deref(),
            Some("the link `../../up.md` climbs above the repo root")
        );
    }

    #[test]
    fn checks_every_spec_file_and_links_into_the_spec() {
        let root = tree(
            "whole",
            &[
                (
                    "spec/README.md",
                    "# Spec\n\n[Ok](reference/a.md) [Bad](reference/b.md)\n",
                ),
                ("spec/reference/a.md", "# A\n\n[Up](../README.md#spec)\n"),
                (
                    "std/API.md",
                    "# API\n\n[Spec](../spec/reference/a.md#nope) [Elsewhere](missing.md)\n",
                ),
            ],
        );
        let err = check(&root).unwrap_err();
        assert_eq!(err, "2 broken link(s)");
    }

    #[test]
    fn real_spec_links_resolve() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        assert_eq!(check(&root), Ok(()));
    }
}
