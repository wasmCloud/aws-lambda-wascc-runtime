# waSCC Runtime for AWS Lambda

A [waSCC](https://wascc.dev/) [runtime for AWS Lambda](https://docs.aws.amazon.com/lambda/latest/dg/lambda-runtimes.html).

This workspace includes multiple crates:

* [`codec`](codec/README.md) is a common set of types and other primitives
* [`provider`](provider/README.md) is a waSCC native capability provider that interacts with the AWS Lambda runtime interface
* [`runtime`](runtime/README.md) is the AWS Lambda runtime
