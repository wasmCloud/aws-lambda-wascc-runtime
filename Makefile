COLOR ?= auto
CARGO = cargo --color $(COLOR)
BUILDER = ewbankkit/rust-amazonlinux:1.41.1-2018.03.0.20191219.0

.PHONY: all build check clean doc fmt release test update

all: build

build:
	$(CARGO) build

check:
	$(CARGO) check

clean:
	$(CARGO) clean

doc:
	$(CARGO) doc

fmt:
	$(CARGO) fmt

release:
	docker run --user "$(id -u)":"$(id -g)" --volume $(PWD):/volume --rm --tty $(BUILDER) cargo build --release

test: build
	$(CARGO) fmt --all -- --check
	$(CARGO) clippy --workspace
	$(CARGO) test --workspace --lib

update:
	$(CARGO) update
