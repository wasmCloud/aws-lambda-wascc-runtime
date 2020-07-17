.PHONY: all apply

all: apply

apply: slim.zip
	terraform apply

slim.zip: ../target/release/bootstrap
	zip -j $@ $^
