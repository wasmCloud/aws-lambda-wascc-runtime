# AWS Lambda Layers for the waSCC Runtime

Build and deploy Lambda layers for the waSCC runtime.

* `wascc-slim` contains just the custom runtime binary (`bootstrap`). This layer is compatible with the [custom runtime](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html) based on Amazon Linux 2.

### Deploy

```console
$ terraform init
```

Set AWS environment variables for your authenticated session.

```console
$ make
```
