fn main() {
    // Tracks SQL migration changes so `sqlx::migrate!` is rebuilt.
    println!("cargo:rerun-if-changed=migrations");
}
