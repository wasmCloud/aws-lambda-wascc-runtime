.PHONY: all apply

all: apply

apply: slim.zip
	terraform12 apply

slim.zip: ../target/release/bootstrap
	zip -j $@ $^
