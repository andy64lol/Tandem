CARGO ?= cargo
BINARY := tandem
PREFIX ?= /usr/local
DESTDIR ?=
ARGS ?=
VERSION := $(shell sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n 1)
ARCH ?= $(shell uname -m)
TARGET ?=
RELEASE_DIR := target/$(if $(TARGET),$(TARGET)/,)
RELEASE_BIN := $(RELEASE_DIR)release/$(BINARY)
BINARY_PATH ?= $(RELEASE_BIN)
DIST_DIR ?= dist
PACKAGE_NAME := $(BINARY)-v$(VERSION)-linux-$(ARCH)
PACKAGE_DIR := $(DIST_DIR)/$(PACKAGE_NAME)
ARCHIVE := $(DIST_DIR)/$(PACKAGE_NAME).tar.gz

.PHONY: all build release run test check fmt lint clean install package-arch package-arch-binary

all: build

build:
	$(CARGO) build --locked

release:
	$(CARGO) build --locked --release $(if $(TARGET),--target $(TARGET),)

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
	install -Dm755 $(RELEASE_BIN) $(DESTDIR)$(PREFIX)/bin/$(BINARY)

package-arch: release
	@$(MAKE) package-arch-binary BINARY_PATH=$(RELEASE_BIN)

package-arch-binary:
	rm -rf $(PACKAGE_DIR)
	mkdir -p $(PACKAGE_DIR)
	install -Dm755 $(BINARY_PATH) $(PACKAGE_DIR)/$(BINARY)
	install -Dm644 README.md $(PACKAGE_DIR)/README.md
	install -Dm644 LICENSE $(PACKAGE_DIR)/LICENSE
	tar -C $(DIST_DIR) -czf $(ARCHIVE) $(PACKAGE_NAME)
	rm -rf $(PACKAGE_DIR)
	@echo "created $(ARCHIVE)"
