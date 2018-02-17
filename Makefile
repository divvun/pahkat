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
install:
	$(CARGO_HOME)/bin/cargo build $(FLAGS)
clean:
	$(CARGO_HOME)/bin/cargo clean
