#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::{fs, path::Path, time::Duration};

use crownos_files::{
    mime::{MimeDatabase, MimeTable},
    model::{kind::FileKind, mime_id::MimeId},
};

fn name(mime: MimeId) -> Option<&'static str> {
    MimeTable::name(mime)
}

fn system() -> Option<&'static MimeDatabase> {
    Path::new("/usr/share/mime/globs2")
        .exists()
        .then(MimeDatabase::system)
}

#[test]
fn a_private_database_detects_names_content_and_hierarchy() {
    let dir = tempfile::tempdir().unwrap();
    let mime = dir.path().join("mime");
    fs::create_dir(&mime).unwrap();
    fs::write(
        mime.join("globs2"),
        "50:text/rust:*.rs\n50:image/png:*.png\n",
    )
    .unwrap();
    fs::write(mime.join("aliases"), "text/x-rust text/rust\n").unwrap();
    fs::write(mime.join("subclasses"), "text/rust text/plain\n").unwrap();
    let mut magic = b"MIME-Magic\0\n[50:image/png]\n>0=\0\x04\x89PNG\n".to_vec();
    magic.extend_from_slice(b"[40:text/x-private-shebang]\n>0=\0\x02#!\n");
    fs::write(mime.join("magic"), magic).unwrap();
    let db = MimeDatabase::load(&[mime]);

    assert_eq!(name(db.mime_of_name("lib.RS")), Some("text/rust"));
    assert_eq!(db.mime_of_name("README"), MimeId::UNKNOWN);
    assert_eq!(name(db.sniff_data(b"\x89PNG\r\n")), Some("image/png"));
    assert_eq!(
        name(db.sniff_data(b"#!/bin/sh\n")),
        Some("text/x-private-shebang")
    );
    assert_eq!(name(db.sniff_data(b"just words\n")), Some("text/plain"));
    assert_eq!(
        name(db.sniff_data(&[0, 1, 2, 3])),
        Some("application/octet-stream")
    );
    assert_eq!(name(db.sniff_data(b"")), Some("application/x-zerosize"));

    let alias = MimeId::intern("text/x-rust");
    assert_eq!(
        db.ancestors(alias),
        [MimeId::intern("text/rust"), MimeId::intern("text/plain")]
    );
    assert_eq!(db.kind_of(alias), FileKind::Code);
    assert_eq!(
        db.refine_kind(FileKind::Other, MimeId::intern("image/png")),
        FileKind::Image
    );
    assert_eq!(
        db.refine_kind(FileKind::Video, MimeId::intern("text/rust")),
        FileKind::Video
    );
    assert!(db.is_previewable(MimeId::intern("image/png")));
}

#[test]
fn the_system_database_classifies_common_files() {
    let Some(db) = system() else {
        eprintln!("skipped: no shared-mime-info database");
        return;
    };
    assert_eq!(name(db.mime_of_name("photo.JPG")), Some("image/jpeg"));
    assert_eq!(
        name(db.mime_of_name("src.tar.gz")),
        Some("application/x-compressed-tar")
    );
    assert_eq!(name(db.mime_of_name("Makefile")), Some("text/x-makefile"));
    assert_eq!(db.kind_of(db.mime_of_name("main.rs")), FileKind::Code);
    assert_eq!(db.kind_of(db.mime_of_name("report.pdf")), FileKind::Pdf);
    assert_eq!(db.kind_of(db.mime_of_name("notes.md")), FileKind::Markdown);
}

#[test]
fn sniffing_reads_content_and_never_blocks_on_special_files() {
    let Some(db) = system() else {
        eprintln!("skipped: no shared-mime-info database");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("picture");
    image::save_buffer(
        dir.path().join("p.png"),
        &[0, 0, 0, 255],
        1,
        1,
        image::ExtendedColorType::Rgba8,
    )
    .unwrap();
    fs::rename(dir.path().join("p.png"), &png).unwrap();
    assert_eq!(name(db.sniff(&png).unwrap()), Some("image/png"));

    let script = dir.path().join("run");
    fs::write(&script, "#!/bin/sh\necho hi\n").unwrap();
    let shell = db.unalias(MimeId::intern("application/x-shellscript"));
    assert_eq!(db.sniff(&script).unwrap(), shell);
    assert_eq!(db.kind_of(shell), FileKind::Code);

    let fifo = dir.path().join("pipe");
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::from_raw_mode(0o600),
        0,
    )
    .unwrap();
    let sniffed = std::thread::spawn(move || db.sniff(&fifo).map(name));
    std::thread::sleep(Duration::from_millis(100));
    assert!(sniffed.is_finished(), "sniffing a FIFO blocked");
    assert_eq!(sniffed.join().unwrap().unwrap(), Some("inode/fifo"));

    assert_eq!(name(db.sniff(dir.path()).unwrap()), Some("inode/directory"));
    assert!(db.sniff(&dir.path().join("missing")).is_err());
}
