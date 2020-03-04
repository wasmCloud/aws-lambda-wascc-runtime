//
// waSCC AWS Lambda Runtime Codec
//

fn main() {
    prost_build::compile_protos(&["src/lambda.proto"], &["src/"]).unwrap();
}
