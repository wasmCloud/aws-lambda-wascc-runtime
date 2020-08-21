# waSCC Runtime for AWS Lambda

:construction: :construction: **This project is highly experimental.** :construction: :construction:

It is not yet ready for production. Use at your own risk.

A [waSCC](https://wascc.dev/) [runtime for AWS Lambda](https://docs.aws.amazon.com/lambda/latest/dg/lambda-runtimes.html).

This workspace includes multiple crates:

* [`codec`](codec/README.md) is a common set of types and other primitives
* [`provider`](provider/README.md) is a waSCC native capability provider that interacts with the AWS Lambda runtime interface
* [`runtime`](runtime/README.md) is the AWS Lambda runtime

## Build

Build a binary suitable for running on Amazon Linux 2 using a [builder image](https://hub.docker.com/repository/docker/ewbankkit/rust-amazonlinux):

```console
$ make release
```

## Lambda Layers

Instructions for building [AWS Lambda Layers](https://docs.aws.amazon.com/lambda/latest/dg/configuration-layers.html) containing the waSCC runtime are in [`layers`](layers/README.md).

## Examples

Examples are in [`examples`](examples/README.md).
