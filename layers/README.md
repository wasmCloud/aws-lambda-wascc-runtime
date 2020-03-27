# AWS Lambda Layers for the waSCC Runtime

Build and deploy Lambda layers for the waSCC runtime.

* `wascc-slim` contains the custom runtime binary (`bootstrap`) and the [standard waSCC logging provider](https://github.com/wascc/logging-provider).

### Deploy

```console
$ terraform init
```

Set AWS environment variables for your authenticated session.

```console
$ make
```
