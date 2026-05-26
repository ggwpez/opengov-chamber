build:
    cd contract && cargo build --release
b: build

test: build
    cd tests && cargo test
t: test
