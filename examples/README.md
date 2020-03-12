# waSCC Runtime for AWS Lambda Examples

A simple example of using the [waSCC](https://wascc.dev/) runtime for AWS Lambda.

### Requirements

* [Rust](https://www.rust-lang.org/) version 1.41.0 (or later) to build the example waSCC actor
* [Terraform](https://www.terraform.io/downloads.html) version 0.12.19 (or later) to deploy the AWS Lambda

### Build

Build the [sample waSCC actor](actor/README.md).

### Deploy

Set AWS environment variables for your authenticated session.

```console
$ make
```

### Test

```console
$ aws --region us-west-2 lambda invoke --function-name waSCC-example --payload '{"input": "Hello world"}' output.json
```

`output.json` should contain the input text converted to uppercase.

### Known Issues

It works on my machine!

The public key of the actor in `manifest.yaml` is the value I use and will have to be changed when you generate your own keys.
