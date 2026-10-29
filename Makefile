CARGO ?= cargo
BINARY := typelua
PREFIX ?= /usr/local
DESTDIR ?=
ARGS ?=
VERSION := $(shell sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
ARCH ?= $(shell uname -m)
DIST_DIR ?= dist
PACKAGE_NAME := $(BINARY)-v$(VERSION)-linux-$(ARCH)
PACKAGE_DIR := $(DIST_DIR)/$(PACKAGE_NAME)
ARCHIVE := $(DIST_DIR)/$(PACKAGE_NAME).tar.gz

.PHONY: all build release run test check fmt lint clean install package-arch

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

package-arch: release
	rm -rf $(PACKAGE_DIR)
	mkdir -p $(PACKAGE_DIR)
	install -Dm755 target/release/$(BINARY) $(PACKAGE_DIR)/$(BINARY)
	install -Dm644 README.md $(PACKAGE_DIR)/README.md
	install -Dm644 LICENSE $(PACKAGE_DIR)/LICENSE
	tar -C $(DIST_DIR) -czf $(ARCHIVE) $(PACKAGE_NAME)
	rm -rf $(PACKAGE_DIR)
	@echo "created $(ARCHIVE)"
