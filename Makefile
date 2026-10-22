CARGO ?= cargo
BINARY := typelua
PREFIX ?= /usr/local
DESTDIR ?=
ARGS ?=

.PHONY: all build release run test check fmt lint clean install

all: build

build:
	$(CARGO) build --locked

release:
	$(CARGO) build --locked --release

run:
	$(CARGO) run --locked -- $(ARGS)

test:
	$(CARGO) test --locked

check:
	$(CARGO) check --locked

fmt:
	$(CARGO) fmt

lint:
	$(CARGO) clippy --all-targets

clean:
	$(CARGO) clean

install: release
	install -Dm755 target/release/$(BINARY) $(DESTDIR)$(PREFIX)/bin/$(BINARY)
