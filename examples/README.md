# Examples

### Requirements

* [Terraform](https://www.terraform.io/downloads.html) version 0.12.19 (or later)

### To Build

Install the `x86_64-unknown-linux-musl` build target

```console
$ sudo apt-get install musl musl-dev musl-tools
$ rustup target list
$ rustup target install x86_64-unknown-linux-musl
```

```console
$ cd aws-lambda-wascc-runtime
$ cargo build --release --target x86_64-unknown-linux-musl
$ cp ./target/x86_64-unknown-linux-musl/release/aws-lambda-wascc-runtime ./examples/bootstrap
```

### To Deploy

```console
$ cd examples
$ terraform init
$ terraform apply
```

### To Run

```console
$ aws --region us-west-2 lambda invoke --function-name waSCC-example --payload '{"input": "Hello world"}' output.json
```
