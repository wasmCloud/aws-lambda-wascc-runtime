.PHONY: all apply

all: apply

apply: slim.zip
	terraform12 apply

slim.zip: ../target/release/bootstrap ../../logging-provider/target/release/libwascc_log.so
	zip -j $@ $^
