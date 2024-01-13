# Testing
This package contains the testing both for `backhand` and `backhand-cli`.

First, build the binaries that will be tested along with unit tests.
```
$ cargo build --release --bins
```
Then, run the tests:
```
$ cargo test --workspace --release --all-features
```

## Cross platform testing
You can also use `cargo-cross` to test on other architectures.
See [ci](.github/workflows/main.yml) for an example of testing. We currently test the following in CI:
- x86_64-unknown-linux-musl
- aarch64-unknown-linux-musl
- arm-unknown-linux-musleabi
- armv7-unknown-linux-musleabi

## Coverage
```
$ cargo llvm-cov run --bin replace --no-clean --release
$ cargo llvm-cov run --bin add --no-clean --release
$ cargo llvm-cov run --bin unsquashfs --no-clean --release
$ cargo llvm-cov --html --workspace --all-features --release --no-clean -- --skip slow
```
