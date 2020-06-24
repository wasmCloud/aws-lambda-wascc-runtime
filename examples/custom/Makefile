.PHONY: all apply

all: apply

apply: app.zip
	terraform apply

app.zip: manifest.yaml actor/target/wasm32-unknown-unknown/release/wascc_actor_hello_lambda_signed.wasm
	zip -j $@ $^
