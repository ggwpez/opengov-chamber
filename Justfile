build:
    cd contract && cargo build --release

test:
    cd tests && cargo test
t: test
