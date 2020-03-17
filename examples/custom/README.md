# Custom Lambda Invocation

### Build

Build the [sample waSCC actor](actor/README.md).

```console
$ cd actor
$ make release
$ cd ..
```

### Deploy

Set AWS environment variables for your authenticated session.

```console
$ make
```

### Test

```console
$ aws lambda invoke --function-name waSCC-example --payload '{"input": "Hello world"}' output.json
```

`output.json` should contain the input text converted to uppercase.

### Known Issues

It works on my machine!

The public key of the actor in `manifest.yaml` is the value I use and will have to be changed when you generate your own keys.
