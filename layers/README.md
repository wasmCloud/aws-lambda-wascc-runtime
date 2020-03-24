# AWS Lambda Layers for the waSCC Runtime

Build and deploy Lambda layers for the waSCC runtime.

* `wascc-slim` contains just the custom runtime binary (`bootstrap`)

### Deploy

```console
$ terraform init
```

Set AWS environment variables for your authenticated session.

```console
$ make
```
