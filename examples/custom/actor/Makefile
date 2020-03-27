COLOR ?= always # Valid COLOR options: {always, auto, never}
CARGO = cargo --color $(COLOR)
TARGET = target/wasm32-unknown-unknown
DEBUG = $(TARGET)/debug
RELEASE = $(TARGET)/release
KEYDIR ?= .keys

.PHONY: all bench build check clean doc test update keys keys-account keys-module

all: build

bench:
	@$(CARGO) bench

build:
	@$(CARGO) build --target wasm32-unknown-unknown
	wascap sign $(DEBUG)/wascc_actor_hello_lambda.wasm $(DEBUG)/wascc_actor_hello_lambda_signed.wasm --issuer $(KEYDIR)/account.nk --subject $(KEYDIR)/module.nk --cap awslambda:runtime --cap wascc:logging --name wascc_actor_hello_lambda

check:
	@$(CARGO) check

clean:
	@$(CARGO) clean

doc:
	@$(CARGO) doc

test: build
	@$(CARGO) test

update:
	@$(CARGO) update

release:
	@$(CARGO) build --release --target wasm32-unknown-unknown
	wascap sign $(RELEASE)/wascc_actor_hello_lambda.wasm $(RELEASE)/wascc_actor_hello_lambda_signed.wasm --issuer $(KEYDIR)/account.nk --subject $(KEYDIR)/module.nk --cap awslambda:runtime --cap wascc:logging --name wascc_actor_hello_lambda

keys: keys-account
keys: keys-module

keys-account:
	@mkdir -p $(KEYDIR)
	nk gen account > $(KEYDIR)/account.txt
	awk '/Seed/{ print $$2 }' $(KEYDIR)/account.txt > $(KEYDIR)/account.nk

keys-module:
	@mkdir -p $(KEYDIR)
	nk gen module > $(KEYDIR)/module.txt
	awk '/Seed/{ print $$2 }' $(KEYDIR)/module.txt > $(KEYDIR)/module.nk
