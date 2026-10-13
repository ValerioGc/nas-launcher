use super::*;
use crate::services::{
    database::open_in_memory,
    packages::catalogue::{self, CatalogueApp, Provider},
};

const SVG: &[u8] = br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>"#;

fn entry(id: &str) -> CatalogueApp {
    CatalogueApp {
        id: id.to_owned(),
        name: "Photos".to_owned(),
        info: String::new(),
        provider: Provider::Github,
        repository: "example/photos".to_owned(),
        asset: "photos-{version}.tar.gz".to_owned(),
        private: false,
        system: false,
        preinstall: false,
        logo: String::new(),
        category: None,
    }
}

#[test]
fn the_format_is_read_from_the_bytes_and_not_from_what_the_browser_says() {
    assert_eq!(format(SVG), Some("image/svg+xml"));
    assert_eq!(format(b"\xef\xbb\xbf <svg/>"), Some("image/svg+xml"));
    assert_eq!(format(b"\x89PNG\r\n\x1a\nrest"), Some("image/png"));
    assert_eq!(format(b"\xff\xd8\xff\xe0rest"), Some("image/jpeg"));
    assert_eq!(format(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
    assert_eq!(format(b"<html><body></body></html>"), None);
    assert_eq!(format(b"GIF89a"), None);
    assert_eq!(format(b"\x00\x01"), None);
}

#[tokio::test]
async fn an_uploaded_logo_is_served_until_it_is_removed_or_its_application_leaves() {
    let db = open_in_memory().await.expect("database");
    catalogue::save(&db, vec![entry("photos"), entry("disks")])
        .await
        .expect("catalogue");

    assert_eq!(
        store(&db, "unknown", SVG).await.unwrap_err().0.as_u16(),
        404
    );
    assert_eq!(
        store(&db, "photos", b"<html/>")
            .await
            .unwrap_err()
            .0
            .as_u16(),
        400
    );
    let large = [SVG, &vec![b' '; MAX_BYTES]].concat();
    assert_eq!(
        store(&db, "photos", &large).await.unwrap_err().0.as_u16(),
        400
    );

    store(&db, "photos", SVG).await.expect("stored");
    store(&db, "disks", SVG).await.expect("stored");
    assert_eq!(
        read(&db, "photos").await.expect("logo"),
        ("image/svg+xml".to_owned(), SVG.to_vec())
    );
    let found = addresses(&db).await;
    assert!(found["photos"].starts_with("/api/apps/photos/logo?v="));

    forget(&db, "photos").await.expect("removed");
    keep_only(&db, &["photos"]).await.expect("kept");
    assert!(read(&db, "photos").await.is_err());
    assert!(read(&db, "disks").await.is_err());
    assert!(read(&db, "../photos").await.is_err());
}

#[tokio::test]
async fn the_logo_the_installation_put_on_the_machine_stands_in_for_an_uploaded_one() {
    let db = open_in_memory().await.expect("database");
    let directory = std::env::temp_dir().join(format!("nas-os-logos-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("directory");
    std::fs::write(directory.join("gallery.svg"), SVG).expect("logo");
    std::fs::write(directory.join("notes.txt"), "not a logo").expect("file");
    unsafe {
        std::env::set_var("NAS_LOGOS_DIR", &directory);
    }

    assert_eq!(read(&db, "gallery").await.expect("logo").0, "image/svg+xml");
    let found = addresses(&db).await;
    assert_eq!(
        found.get("gallery").map(String::as_str),
        Some("/api/apps/gallery/logo")
    );
    assert!(!found.contains_key("notes"));

    unsafe {
        std::env::remove_var("NAS_LOGOS_DIR");
    }
    std::fs::remove_dir_all(&directory).expect("cleanup");
}
