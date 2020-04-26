fn main() {
    tonic_build::compile_protos("proto/pahkat.proto").unwrap();

    let gen_path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("pahkat.rs");
    let data = std::fs::read_to_string(&gen_path).unwrap();
    let data = data.replace("::prost::Message)", "::prost::Message, ::serde::Serialize)");
    let data = data.replace("::prost::Oneof)", "::prost::Oneof, ::serde::Serialize)");
    std::fs::write(gen_path, data).unwrap();
}
