COLOR ?= always # Valid COLOR options: {always, auto, never}
CARGO = cargo --color $(COLOR)
MUSLRUST = clux/muslrust:1.41.1-stable

.PHONY: all build check clean doc pull release test update

all: build

build:
	@$(CARGO) build

check:
	@$(CARGO) check

clean:
	@$(CARGO) clean

doc:
	@$(CARGO) doc

pull:
	docker pull $(MUSLRUST)

# release:
# 	@$(CARGO) build --release
release: pull
	docker run --volume $(PWD):/volume --rm --tty $(MUSLRUST) ln -s "/usr/bin/g++" "/usr/bin/musl-g++" && cargo build --release

test: build
	@$(CARGO) test

update:
	@$(CARGO) update
