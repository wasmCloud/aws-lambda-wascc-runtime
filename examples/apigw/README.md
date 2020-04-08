# API Gateway HTTP Request Invocation

### Build

Build the [sample waSCC actor](actor/README.md).

```console
$ cd actor
$ make release
$ cd ..
```

### Deploy

This examples uses the `wascc-slim` Lambda layer.
See [`layers`](../../layers/README/md) for instructions on building the waSCC runtime Lambda layers.

```console
$ terraform init
```

Set AWS environment variables for your authenticated session.

```console
$ make
```

### Test


### Known Issues

It works on my machine!

The public key of the actor in `manifest.yaml` is the value I use and will have to be changed when you generate your own keys.
