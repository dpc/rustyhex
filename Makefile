include Makefile.defs

default: $(DEFAULT_TARGET)

.PHONY: run test build doc clean
test doc clean:
	cargo $@

run build:
	cargo $@ --release

.PHONY: docview
docview: doc
	xdg-open target/doc/$(PKG_NAME)/index.html
