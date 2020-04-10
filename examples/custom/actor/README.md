# Sample waSCC Actor

A sample [waSCC](https://wascc.dev/) actor that uses the AWS Lambda runtime capability provider.

It is based on the [Krustlet uppercase demo](https://github.com/deislabs/krustlet/tree/master/demos/wascc/uppercase), converting its input to uppercase.

Input should be a custom Lambda event with payload a JSON document of the form

```json
{"input": "Hello world"}
```

and the corresponding response will be a JSON document of the form

```json
{"output": "HELLO WORLD"}
```

## Build

#### Install [NKeys](https://github.com/encabulators/nkeys)

```console
cargo install nkeys --features "cli"
```

#### Generate Keys

```console
make keys
```

#### Add `wasm32-unknown-unknown` Compilation Target

```console
rustup target add wasm32-unknown-unknown
```

#### Install [WASCAP](https://github.com/wascc/wascap)

```console
cargo install wascap --features "cli"
```

#### Build Actor

```console
make release
```
