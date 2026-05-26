host := `rustc -vV | sed -n 's/^host: //p'`

build:
    cd contract && cargo build --release
b: build

test: build
    cd tests && cargo test -- --nocapture
t: test
