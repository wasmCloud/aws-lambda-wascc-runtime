# API Gateway HTTP Request Invocation

This actor is identical to the [Krustlet Uppercase](https://github.com/deislabs/krustlet/tree/master/demos/wascc/uppercase) demo,
the only change being that the actor is signed with the `awslambda:runtime` capability instead of the standard HTTP server capability.

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
