fn main() {
    let proto_file = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("proto/pahkat.proto");
    let out_dir = std::env::var("OUT_DIR").unwrap();

    tonic_build::configure()
        .out_dir(&out_dir)
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .compile(&[proto_file], &[])
        .expect("tonic should build protos, but");

    let gen_path = std::path::Path::new(&out_dir).join("pahkat.rs");
    let data = std::fs::read_to_string(&gen_path).expect("`pahkat.rs` should exist, but");
    let data = data.replace(
        "pub enum Value {",
        "#[serde(tag = \"type\")] pub enum Value {",
    );
    std::fs::write(gen_path, data).unwrap();
}
