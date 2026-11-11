//! Reads the Quince language spec: its chapter manifest, rule IDs and grammar blocks.
//!
//! The formats it checks are defined in this crate's `README.md`.

pub mod grammar;
pub mod links;
pub mod manifest;
pub mod rules;

use manifest::Chapter;
use std::io::ErrorKind;
use std::path::Path;

/// The folder that holds the spec, relative to the repo root. Chapter files in the manifest are
/// relative to it.
pub const SPEC_DIR: &str = "spec";
/// The chapter manifest, relative to the repo root.
pub const CHAPTERS_PATH: &str = "spec/chapters.txt";
/// The spec's index, which holds its version header, relative to the repo root.
pub const README_PATH: &str = "spec/README.md";
/// Rule IDs that must never be used again, relative to the repo root.
pub const RETIRED_PATH: &str = "spec/retired-rules.txt";
/// The allowed first segments of rule IDs, relative to the repo root.
pub const TOPICS_PATH: &str = "spec/rule-topics.txt";
/// The grammar assembled from the spec's `ebnf` blocks, relative to the repo root.
pub const GRAMMAR_PATH: &str = "spec/grammar.ebnf";
/// The folders whose Markdown files must all be chapters listed in the manifest, relative to
/// [`SPEC_DIR`].
pub const CHAPTER_DIRS: [&str; 2] = ["reference", "tooling"];

/// A problem found in a spec file, with its 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

/// A problem in a named file. Line 0 means the problem has no line, such as a missing file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileError {
    /// Relative to the repo root.
    pub path: String,
    pub line: usize,
    pub message: String,
}

impl FileError {
    /// Puts `error`, found in the file at `path`, in that file.
    pub fn new(path: &str, error: SpecError) -> FileError {
        FileError {
            path: path.to_string(),
            line: error.line,
            message: error.message,
        }
    }
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.line {
            0 => write!(f, "{}: {}", self.path, self.message),
            line => write!(f, "{}:{line}: {}", self.path, self.message),
        }
    }
}

/// One chapter file, read from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChapterFile {
    /// The chapter's ID prefix, or [`manifest::LEGACY_PREFIX`].
    pub prefix: String,
    /// The chapter's title from the manifest.
    pub title: String,
    /// Relative to the repo root, such as `spec/reference/lexical.md`.
    pub path: String,
    /// As listed in the manifest: relative to [`SPEC_DIR`], such as `reference/lexical.md`.
    pub file: String,
    pub text: String,
}

/// The spec as the manifest describes it: every chapter, and the text of every chapter file
/// that exists, in manifest order.
#[derive(Debug, Clone)]
pub struct Spec {
    pub chapters: Vec<Chapter>,
    pub files: Vec<ChapterFile>,
    /// The text of [`README_PATH`].
    pub readme: String,
}

impl Spec {
    /// Reads the manifest, the README and every chapter file. A listed file may be missing only
    /// while the `legacy` chapter's file exists. Every Markdown file under the
    /// [`CHAPTER_DIRS`] must be listed. Returns every problem found.
    pub fn load(root: &Path) -> Result<Spec, Vec<FileError>> {
        let mut errors = Vec::new();
        let read = |path: &str| std::fs::read_to_string(root.join(path));
        let manifest_text = read(CHAPTERS_PATH).map_err(|e| vec![missing(CHAPTERS_PATH, e)])?;
        let (chapters, manifest_errors) = manifest::parse(&manifest_text);
        errors.extend(
            manifest_errors
                .into_iter()
                .map(|e| FileError::new(CHAPTERS_PATH, e)),
        );
        let readme = match read(README_PATH) {
            Ok(text) => text,
            Err(e) => {
                errors.push(missing(README_PATH, e));
                String::new()
            }
        };

        let legacy_exists = chapters.iter().any(|chapter| {
            chapter.prefix == manifest::LEGACY_PREFIX
                && root.join(spec_path(&chapter.file)).is_file()
        });
        let mut files = Vec::new();
        for chapter in &chapters {
            let path = spec_path(&chapter.file);
            match read(&path) {
                Ok(text) => files.push(ChapterFile {
                    prefix: chapter.prefix.clone(),
                    title: chapter.title.clone(),
                    path,
                    file: chapter.file.clone(),
                    text,
                }),
                Err(e) if e.kind() == ErrorKind::NotFound && legacy_exists => {}
                Err(e) if e.kind() == ErrorKind::NotFound => errors.push(FileError {
                    path: CHAPTERS_PATH.to_string(),
                    line: chapter.line,
                    message: format!(
                        "chapter `{}` lists {path}, which doesn't exist; create it or fix the path",
                        chapter.prefix
                    ),
                }),
                Err(e) => errors.push(missing(&path, e)),
            }
        }

        for dir in CHAPTER_DIRS {
            let mut found = Vec::new();
            if let Err(e) = markdown_files(&root.join(SPEC_DIR).join(dir), dir, &mut found) {
                errors.push(FileError {
                    path: format!("{SPEC_DIR}/{dir}"),
                    line: 0,
                    message: format!("can't read the folder: {e}"),
                });
            }
            found.sort();
            for file in found {
                if !chapters.iter().any(|chapter| chapter.file == file) {
                    // A chapter file sits directly in its folder, so a nested one can't be listed.
                    let fix = if file[dir.len() + 1..].contains('/') {
                        format!(
                            "move it directly into {SPEC_DIR}/{dir}/ and list it, or move it out"
                        )
                    } else {
                        format!("list it as a chapter, or move it out of {SPEC_DIR}/{dir}")
                    };
                    errors.push(FileError {
                        path: spec_path(&file),
                        line: 0,
                        message: format!("this file isn't listed in {CHAPTERS_PATH}; {fix}"),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(Spec {
                chapters,
                files,
                readme,
            })
        } else {
            Err(errors)
        }
    }

    /// The chapter file whose path is `path`, either relative to the repo root
    /// (`spec/reference/lexical.md`) or as the manifest lists it (`reference/lexical.md`).
    pub fn file(&self, path: &str) -> Option<&ChapterFile> {
        self.files
            .iter()
            .find(|file| file.path == path || file.file == path)
    }
}

/// A manifest file path made relative to the repo root.
pub fn spec_path(file: &str) -> String {
    format!("{SPEC_DIR}/{file}")
}

fn missing(path: &str, error: std::io::Error) -> FileError {
    FileError {
        path: path.to_string(),
        line: 0,
        message: format!("can't read the file: {error}"),
    }
}

/// Collects the `.md` files under `dir`, as paths that start with `relative`. A missing folder
/// holds no files.
fn markdown_files(dir: &Path, relative: &str, out: &mut Vec<String>) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{relative}/{name}");
        if entry.file_type()?.is_dir() {
            markdown_files(&entry.path(), &path, out)?;
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

    /// A fresh temporary folder holding `files`, for one test.
    fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root = std::env::temp_dir()
            .join(format!("quince_spec-tests-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        root
    }

    fn messages(errors: Vec<FileError>) -> Vec<String> {
        errors.into_iter().map(|e| e.to_string()).collect()
    }

    #[test]
    fn loads_chapters_in_manifest_order() {
        let root = tree(
            "order",
            &[
                (
                    CHAPTERS_PATH,
                    "lex  reference/lexical.md  Lexical structure\n\
                     dep  tooling/dependencies.md  Dependencies and publishing\n",
                ),
                (README_PATH, "# Spec\n"),
                ("spec/reference/lexical.md", "# Lexical structure\n"),
                ("spec/tooling/dependencies.md", "# Dependencies\n"),
            ],
        );
        let spec = Spec::load(&root).unwrap();
        let paths: Vec<_> = spec.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(
            paths,
            ["spec/reference/lexical.md", "spec/tooling/dependencies.md"]
        );
        assert_eq!(spec.files[1].title, "Dependencies and publishing");
        assert_eq!(
            spec.file("reference/lexical.md").map(|f| f.prefix.as_str()),
            Some("lex")
        );
        assert_eq!(spec.readme, "# Spec\n");
    }

    #[test]
    fn missing_files_are_errors_without_the_legacy_file() {
        let root = tree(
            "missing",
            &[
                (
                    CHAPTERS_PATH,
                    "legacy  quince-spec.md  Being split\nlex  reference/lexical.md  Lexical structure\n",
                ),
                (README_PATH, "# Spec\n"),
            ],
        );
        assert_eq!(
            messages(Spec::load(&root).unwrap_err()),
            [
                "spec/chapters.txt:1: chapter `legacy` lists spec/quince-spec.md, which doesn't exist; create it or fix the path",
                "spec/chapters.txt:2: chapter `lex` lists spec/reference/lexical.md, which doesn't exist; create it or fix the path",
            ]
        );
    }

    #[test]
    fn missing_files_are_allowed_while_the_legacy_file_exists() {
        let root = tree(
            "transition",
            &[
                (
                    CHAPTERS_PATH,
                    "legacy  quince-spec.md  Being split\nlex  reference/lexical.md  Lexical structure\n",
                ),
                (README_PATH, "# Spec\n"),
                ("spec/quince-spec.md", "# Spec\n"),
            ],
        );
        let spec = Spec::load(&root).unwrap();
        assert_eq!(spec.files.len(), 1);
        assert_eq!(spec.chapters.len(), 2);
    }

    #[test]
    fn unlisted_chapter_files_are_errors() {
        let root = tree(
            "unlisted",
            &[
                (
                    CHAPTERS_PATH,
                    "lex  reference/lexical.md  Lexical structure\n",
                ),
                (README_PATH, "# Spec\n"),
                ("spec/reference/lexical.md", "# Lexical structure\n"),
                ("spec/reference/extra/stray.md", "# Stray\n"),
                ("spec/tooling/extra.md", "# Extra\n"),
                ("spec/tooling/notes.txt", "not Markdown\n"),
            ],
        );
        assert_eq!(
            messages(Spec::load(&root).unwrap_err()),
            [
                "spec/reference/extra/stray.md: this file isn't listed in spec/chapters.txt; move it directly into spec/reference/ and list it, or move it out",
                "spec/tooling/extra.md: this file isn't listed in spec/chapters.txt; list it as a chapter, or move it out of spec/tooling",
            ]
        );
    }

    #[test]
    fn missing_manifest_and_readme() {
        let root = tree("empty", &[]);
        let errors = messages(Spec::load(&root).unwrap_err());
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].starts_with("spec/chapters.txt: can't read the file"));
        let root = tree(
            "no-readme",
            &[(CHAPTERS_PATH, "lex  reference/lexical.md  Lexical\n")],
        );
        let errors = messages(Spec::load(&root).unwrap_err());
        assert!(errors[0].starts_with("spec/README.md: can't read the file"));
    }

    #[test]
    fn loads_the_real_spec() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let spec = Spec::load(&root).unwrap();
        assert!(!spec.files.is_empty());
    }
}
