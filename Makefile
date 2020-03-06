COLOR ?= always # Valid COLOR options: {always, auto, never}
CARGO = cargo --color $(COLOR)
BUILDER = rust:1.41.1-amazonlinux2018.03.0.20191219.0

.PHONY: all build builder-image check clean doc release test update

all: build

build:
	@docker run --volume $(PWD):/volume --rm --tty $(BUILDER) cargo build

builder-image:
	docker build --file Dockerfile --tag rust:1.41.1-amazonlinux .

check:
	@$(CARGO) check

clean:
	@$(CARGO) clean

doc:
	@$(CARGO) doc

release:
	@docker run --volume $(PWD):/volume --rm --tty $(BUILDER) cargo build --release

test: build
	@$(CARGO) test

update:
	@$(CARGO) update
