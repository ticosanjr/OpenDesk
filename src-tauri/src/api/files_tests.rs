use super::archive::archive_entries;
use super::paths::{resolve_path, should_skip};
use super::raw::read_raw;
use super::save::save_to_destination;
use super::search::search_root;

fn workspace() -> tempfile::TempDir {
    let temp = tempfile::TempDir::new().expect("temp");
    std::fs::create_dir_all(temp.path().join("input")).expect("input");
    std::fs::write(temp.path().join("input/brief.md"), "# Brief\n").expect("file");
    temp
}

#[test]
fn resolves_relative_paths_inside_the_workspace() {
    let temp = workspace();
    let root = std::fs::canonicalize(temp.path()).expect("root");
    let path = resolve_path(&root, Some("input/brief.md")).expect("path");
    assert!(path.ends_with("input/brief.md"));
}

#[test]
fn rejects_path_traversal_and_absolute_paths() {
    let temp = workspace();
    let root = std::fs::canonicalize(temp.path()).expect("root");
    assert!(resolve_path(&root, Some("../etc/passwd")).is_err());
    assert!(resolve_path(&root, Some("/etc/passwd")).is_err());
    assert!(resolve_path(&root, Some("input\\brief.md")).is_err());
}

#[test]
fn searches_recursively_and_ignores_node_modules() {
    let temp = workspace();
    std::fs::create_dir_all(temp.path().join("output/2026-11/os")).expect("output");
    std::fs::write(temp.path().join("output/2026-11/os/relatorio.docx"), b"x").expect("report");
    std::fs::create_dir_all(temp.path().join("node_modules/pkg")).expect("nm");
    std::fs::write(temp.path().join("node_modules/pkg/relatorio.js"), b"x").expect("nm file");
    let root = std::fs::canonicalize(temp.path()).expect("root");
    let results = search_root(&root, "relatorio", 50);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].path, "output/2026-11/os/relatorio.docx");
}

#[test]
fn skips_heavy_directories() {
    assert!(should_skip(".git"));
    assert!(should_skip("node_modules"));
    assert!(should_skip("target"));
    assert!(!should_skip("input"));
}

#[test]
fn lists_zip_entries_without_extracting() {
    use std::io::Write;
    let dir = tempfile::TempDir::new().expect("temp");
    let path = dir.path().join("pacote.zip");
    let file = std::fs::File::create(&path).expect("zip");
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    zip.add_directory("docs/", options).expect("dir");
    zip.start_file("docs/brief.md", options).expect("file");
    zip.write_all(b"# Brief").expect("write");
    zip.finish().expect("finish");

    let entries = archive_entries(&path).expect("entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "docs/");
    assert!(entries[0].is_dir);
    assert_eq!(entries[1].name, "docs/brief.md");
    assert_eq!(entries[1].size, 7);
}

#[test]
fn rejects_non_zip_files() {
    let dir = tempfile::TempDir::new().expect("temp");
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, b"plain").expect("file");
    assert!(archive_entries(&path).is_err());
}

#[test]
fn saves_copies_to_an_absolute_destination() {
    let dir = tempfile::TempDir::new().expect("temp");
    let source = dir.path().join("origem.md");
    std::fs::write(&source, b"conteudo").expect("source");
    let destination = dir.path().join("destino.md");
    save_to_destination(&source, &destination.to_string_lossy()).expect("save");
    assert_eq!(
        std::fs::read_to_string(&destination).expect("read"),
        "conteudo"
    );

    let relative = save_to_destination(&source, "destino-relativo.md");
    assert!(relative.is_err());
}

#[test]
fn read_raw_honors_ranges_and_rejects_bad_ones() {
    let dir = tempfile::TempDir::new().expect("temp");
    let path = dir.path().join("data.txt");
    std::fs::write(&path, b"0123456789").expect("file");
    let (all, total, range) = read_raw(&path, None).expect("all");
    assert_eq!(all, b"0123456789");
    assert_eq!(total, 10);
    assert!(range.is_none());
    let (chunk, total, range) = read_raw(&path, Some("bytes=2-5")).expect("range");
    assert_eq!(chunk, b"2345");
    assert_eq!(total, 10);
    assert_eq!(range, Some((2, 5)));
    let (tail, _, range) = read_raw(&path, Some("bytes=7-")).expect("tail");
    assert_eq!(tail, b"789");
    assert_eq!(range, Some((7, 9)));
    let (whole, _, range) = read_raw(&path, Some("items=0-1")).expect("invalid range");
    assert_eq!(whole, b"0123456789");
    assert!(range.is_none());
    let (whole, _, range) = read_raw(&path, Some("bytes=20-30")).expect("past end");
    assert_eq!(whole, b"0123456789");
    assert!(range.is_none());
}
