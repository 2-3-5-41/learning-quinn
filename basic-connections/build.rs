fn main() -> std::io::Result<()> {
    prost_build::compile_protos(&["src/protobufs/plaintext.proto"], &["src/protobufs/"])?;
    Ok(())
}
