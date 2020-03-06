.PHONY: all apply zip

all: apply

apply: zip
	terraform12 apply

zip: bootstrap
	zip -j app.zip actor/target/wasm32-unknown-unknown/release/wascc_actor_hello_lambda_signed.wasm ../target/release/libaws_lambda_runtime_provider.so bootstrap

# The executable run by Lambda must be named 'bootstrap'.
bootstrap: ../target/release/aws-lambda-wascc-runtime
	cp $< $@
