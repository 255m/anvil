# ============================================================
#  anvil — Makefile
# ============================================================
#  Workflow:
#    make                 → build release (as user)
#    sudo make install    → copy to /usr/local/bin (system-wide)
#    sudo make uninstall  → remove from /usr/local/bin
# ============================================================

BIN_NAME    := anvil
CARGO       := cargo
PREFIX      ?= /usr/local
BINDIR      := $(PREFIX)/bin
TARGET      := target/release/$(BIN_NAME)
CONFIG_PATH := $(HOME)/.config/anvil/config.ini
EDITOR      ?= nvim

.PHONY: all build run dev install uninstall clean check fmt clippy test \
        config reset-cfg doc tree size help

# ---------- default ----------
all: build

# ---------- build (as user) ----------
build:
	$(CARGO) build --release

dev:
	$(CARGO) run

run: build
	./$(TARGET)

# ---------- install (needs sudo) ----------
install:
	@if [ ! -f $(TARGET) ]; then \
		echo "✗ binary not found at $(TARGET)"; \
		echo "  run 'make' first (as your user) to build it"; \
		exit 1; \
	fi
	@mkdir -p $(BINDIR)
	@install -m 0755 $(TARGET) $(BINDIR)/$(BIN_NAME)
	@echo "✓ installed → $(BINDIR)/$(BIN_NAME)"

uninstall:
	@rm -f $(BINDIR)/$(BIN_NAME)
	@echo "✓ removed $(BINDIR)/$(BIN_NAME)"

# ---------- quality ----------
check:
	$(CARGO) check --release

clippy:
	$(CARGO) clippy --release -- -W clippy::all

fmt:
	$(CARGO) fmt

test:
	$(CARGO) test --release

# ---------- clean ----------
clean:
	$(CARGO) clean
	@echo "✓ cleaned build artifacts"

# ---------- config ----------
config:
	@mkdir -p $(dir $(CONFIG_PATH))
	@if [ ! -f $(CONFIG_PATH) ]; then \
		echo "; anvil config (empty — will be filled on first run)" > $(CONFIG_PATH); \
		echo "✓ created $(CONFIG_PATH)"; \
	fi
	@$(EDITOR) $(CONFIG_PATH)

reset-cfg:
	@rm -f $(CONFIG_PATH)
	@echo "✓ removed $(CONFIG_PATH) — will regenerate on next run"

# ---------- info ----------
doc:
	$(CARGO) doc --no-deps --open

tree:
	@find src -name '*.rs' | sort | xargs wc -l

size: build
	@ls -lh $(TARGET) | awk '{print "binary: "$$5}'

# ---------- help ----------
help:
	@echo "anvil — available targets:"
	@echo "  build        build release binary (as user)"
	@echo "  dev          run debug build"
	@echo "  run          build release + run"
	@echo "  install      copy binary to $(BINDIR)  [use sudo]"
	@echo "  uninstall    remove $(BINDIR)/$(BIN_NAME)  [use sudo]"
	@echo "  check        cargo check"
	@echo "  clippy       run clippy lints"
	@echo "  fmt          format sources"
	@echo "  test         run tests"
	@echo "  clean        clean build artifacts"
	@echo "  config       open $(CONFIG_PATH) in \$$EDITOR"
	@echo "  reset-cfg    delete $(CONFIG_PATH)"
	@echo "  doc          build + open rustdoc"
	@echo "  tree         show line counts per file"
	@echo "  size         show binary size"
	@echo "  help         show this message"
