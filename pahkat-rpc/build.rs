fn main() {
    tonic_build::compile_protos("proto/pahkat.proto").unwrap();
}
