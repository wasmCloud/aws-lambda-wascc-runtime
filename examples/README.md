# Examples

### Requirements

* [Terraform](https://www.terraform.io/downloads.html) version 0.12.19 (or later)

### To Build

```console
$ cd aws-lambda-wascc-runtime
$ cargo build --release
$ cp ./target/release/aws-lambda-wascc-runtime ./examples/bootstrap
```

### To Deploy

```console
$ terraform init
```
