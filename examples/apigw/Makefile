.PHONY: all apply

all: apply

apply: app.zip
	terraform apply

app.zip: manifest.yaml actor/target/wasm32-unknown-unknown/release/uppercase.wasm
	zip -j $@ $^
