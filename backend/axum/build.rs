fn main() {
    // `sqlx::migrate!` embeds the migrations at compile time: a new file must rebuild the crate.
    println!("cargo:rerun-if-changed=migrations");
}
