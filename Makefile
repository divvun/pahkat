ifdef CARGO_FEATURES
FLAGS += --features $(CARGO_FEATURES)
endif

ifdef CARGO_BIN
FLAGS += --bin $(CARGO_BIN)
endif

ifeq "$(CONFIGURATION)" "Release"
FLAGS += --release
endif

all:
	$(CARGO_HOME)/bin/cargo build $(FLAGS)
	rm -f target/$(CONFIGURATION)/libpahkat_client.dylib
install:
	$(CARGO_HOME)/bin/cargo build $(FLAGS)
	rm -f target/$(CONFIGURATION)/libpahkat_client.dylib
clean:
	$(CARGO_HOME)/bin/cargo clean
