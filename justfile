# Matches build-test-native
build:
    cargo build --release --bins
test: build
    cargo nextest run --release
bench:
    cargo bench
lint:
    cargo fmt
    cargo clippy

# Matches .github/workflows/coverage.yml
coverage:
    cargo llvm-cov run --bin replace-backhand --no-clean --release || true
    cargo llvm-cov run --bin add-backhand --no-clean --release || true
    cargo llvm-cov run --bin unsquashfs-backhand --no-clean --release || true
    cargo llvm-cov nextest --workspace --codecov --output-path codecov.json --features __test_unsquashfs --release --no-clean
