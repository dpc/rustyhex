include Makefile.defs

default: $(DEFAULT_TARGET)

.PHONY: run test build doc clean
run test build doc clean:
	cargo $@

.PHONY: docview
docview: doc
	xdg-open target/doc/$(PKG_NAME)/index.html
