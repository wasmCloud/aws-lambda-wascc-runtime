# SQS Lambda Invocation

### Build

Build the [sample waSCC actor](actor/README.md).

```console
$ cd actor
$ make release
$ cd ..
```

### Deploy

```console
$ terraform init
```

Set AWS environment variables for your authenticated session.

```console
$ make
```

### Test

```console
$ aws sqs send-message --queue-url https://sqs.us-west-2.amazonaws.com/12345689012/waSCC-example-sqs-request --message-body "Testing" --message-attributes '{"ReplyQueueUrl":{"DataType":"String","StringValue":"https://sqs.us-west-2.amazonaws.com/12345689012/waSCC-example-sqs-reply"}}'
$ aws sqs receive-message --queue-url https://sqs.us-west-2.amazonaws.com/12345689012/waSCC-example-sqs-reply
```

### Known Issues

It works on my machine!

The public key of the actor in `manifest.yaml` is the value I use and will have to be changed when you generate your own keys.
