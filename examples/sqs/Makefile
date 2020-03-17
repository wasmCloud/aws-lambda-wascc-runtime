.PHONY: all apply zip

all: apply

apply: zip
	terraform12 apply

zip: bootstrap
	zip -j app.zip manifest.yaml actor/target/wasm32-unknown-unknown/release/wascc_actor_lambda_sqs_signed.wasm ../../target/release/libaws_lambda_runtime_provider.so ../../../../ewbankkit/amazon-sqs-messaging-provider/target/release/libamazon_sqs_messaging_provider.so bootstrap

# The executable run by Lambda must be named 'bootstrap'.
bootstrap: ../../target/release/aws-lambda-wascc-runtime
	cp $< $@
