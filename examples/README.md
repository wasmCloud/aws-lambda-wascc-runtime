# waSCC Runtime for AWS Lambda Examples

Examples of using the [waSCC](https://wascc.dev/) runtime for AWS Lambda.

### Requirements

* [Rust](https://www.rust-lang.org/) version 1.41.0 (or later) to build the example waSCC actors
* [Terraform](https://www.terraform.io/downloads.html) version 0.12.19 (or later) to deploy AWS resources

## Examples

* [`custom`](custom/README.md) A simple example that processes a custom Lambda event
* [`apigw`](apigw/README.md) An example that receives an HTTP request via API Gateway

## Notes

Actors hosted in the waSCC Runtime must be [signed](https://github.com/wascc/wascap) with at least the `wascc:logging` and `awslambda:runtime` or `wascc:http_server` capabilities.
