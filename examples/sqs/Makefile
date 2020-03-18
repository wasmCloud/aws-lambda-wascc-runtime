.PHONY: all apply

all: apply

apply: app.zip
	terraform12 apply

app.zip: manifest.yaml actor/target/wasm32-unknown-unknown/release/wascc_actor_lambda_sqs_signed.wasm ../../target/release/bootstrap ../../../amazon-sqs-messaging-provider/target/release/libamazon_sqs_messaging_provider.so
	zip -j $@ $^
