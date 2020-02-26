# Examples

### Requirements

* [Terraform](https://www.terraform.io/downloads.html) version 0.12.19 (or later)

### To Build

```console
% rustup target list
% rustup target install x86_64-unknown-linux-gnu
```

```console
$ cd aws-lambda-wascc-runtime
$ cargo build --release --target x86_64-unknown-linux-gnu
$ cp ./target/release/aws-lambda-wascc-runtime ./examples/bootstrap
```

### To Deploy

```console
$ terraform init
$ terraform apply
```

### To Run

```console
$ aws --region us-west-2 lambda invoke --function-name waSCC-example --payload '{"firstName": "world"}' output.json
```
