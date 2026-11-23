use super::*;
use std::io::Write;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

#[tokio::test]
async fn blocks_private_and_metadata_destinations() {
    for url in [
        "https://127.0.0.1/",
        "https://10.1.2.3/",
        "https://172.16.0.5/",
        "https://192.168.1.10/",
        "https://169.254.169.254/latest/meta-data/",
        "https://[::1]/",
        "https://[fc00::1]/",
        "https://[fe80::1]/",
    ] {
        assert!(validate_external_url(url).await.is_err(), "{url}");
    }
    assert!(
        validate_external_url("http://127.0.0.1:8080/")
            .await
            .is_ok()
    );
    assert!(
        validate_external_url("ftp://example.com/file")
            .await
            .is_err()
    );
}

#[tokio::test]
async fn blocks_a_redirect_to_a_private_address() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let port = listener.local_addr().expect("addr").port();
    let app = axum::Router::new().route(
        "/redirect",
        axum::routing::get(|| async {
            (
                axum::http::StatusCode::FOUND,
                [(axum::http::header::LOCATION, "https://169.254.169.254/")],
            )
        }),
    );
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let error = download_url(&format!("http://127.0.0.1:{port}/redirect"))
        .await
        .expect_err("redirect");
    assert!(error.to_string().contains("não é permitido"), "{error}");
}

fn zip_with(entries: &[(&str, &[u8])], compression: CompressionMethod) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(compression);
    for (name, content) in entries {
        writer.start_file(*name, options).expect("start");
        writer.write_all(content).expect("write");
    }
    writer.finish().expect("finish").into_inner()
}

#[test]
fn rejects_zip_slip_entries() {
    let bytes = zip_with(
        &[("../escape.md", b"# nope"), ("ok/SKILL.md", b"# ok")],
        CompressionMethod::Stored,
    );
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    collect_zip(&bytes, &mut items, &mut warnings).expect("zip");
    assert!(
        warnings
            .iter()
            .any(|warning| warning.contains("unsafe path"))
    );
    assert_eq!(items.len(), 1);
}

#[test]
fn rejects_a_zip_with_too_many_entries() {
    let contents = vec![b"x".to_vec(); MAX_ARCHIVE_ENTRIES + 1];
    let entries: Vec<(&str, &[u8])> = contents
        .iter()
        .enumerate()
        .map(|(index, content)| {
            let name: &'static str = Box::leak(format!("file-{index}.md").into_boxed_str());
            (name, content.as_slice())
        })
        .collect();
    let bytes = zip_with(&entries, CompressionMethod::Stored);
    let error =
        collect_zip(&bytes, &mut Vec::new(), &mut Vec::new()).expect_err("too many entries");
    assert!(error.to_string().contains("entries"));
}

#[test]
fn rejects_a_compression_bomb_and_handles_empty_files() {
    let payload = vec![b'a'; 200 * 1024];
    let bytes = zip_with(
        &[("bomb.md", payload.as_slice()), ("empty.md", b"")],
        CompressionMethod::Deflated,
    );
    let error = collect_zip(&bytes, &mut Vec::new(), &mut Vec::new()).expect_err("ratio");
    assert!(error.to_string().contains("compression ratio"), "{error}");
}

#[test]
fn enforces_the_total_uncompressed_boundary() {
    let bytes = zip_with(
        &[
            ("skill/SKILL.md", b"# ok"),
            ("skill/extra.md", b"0123456789"),
        ],
        CompressionMethod::Stored,
    );
    let error = collect_zip_with(
        &bytes,
        &mut Vec::new(),
        &mut Vec::new(),
        SafeArchiveLimits {
            max_uncompressed_bytes: 5,
            ..SafeArchiveLimits::default()
        },
    )
    .expect_err("total limit");
    assert!(error.to_string().contains("size limit"), "{error}");
}

#[test]
fn accepts_zero_byte_entries_without_dividing_by_zero() {
    let bytes = zip_with(
        &[("skill/SKILL.md", b"# ok"), ("skill/empty.md", b"")],
        CompressionMethod::Deflated,
    );
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    collect_zip(&bytes, &mut items, &mut warnings).expect("empty entry");
    assert_eq!(items.len(), 1);
    assert!(items[0].files.iter().any(|file| file.path == "empty.md"));
}

#[test]
fn ignores_symlinks_and_paths_outside_a_local_root() {
    let temp = tempfile::TempDir::new().expect("temp");
    let root = temp.path().join("root");
    std::fs::create_dir_all(root.join("skill")).expect("dir");
    std::fs::write(root.join("skill/SKILL.md"), "# ok").expect("file");
    let outside = temp.path().join("outside.md");
    std::fs::write(&outside, "# outside").expect("outside");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, root.join("skill/escape.md")).expect("symlink");

    let mut items = Vec::new();
    let mut warnings = Vec::new();
    collect_local_dir(&root, &mut items, &mut warnings).expect("local");
    assert_eq!(items.len(), 1);
    assert!(
        !items[0]
            .files
            .iter()
            .any(|file| file.path.contains("escape"))
    );
}
