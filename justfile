# Matches build-test-native (plus the versions behind features)
build:
    cargo build --release --bins --features v1,v2,v2_lzma,v3,v3_lzma,v4_lzma
test *args: build
    cargo nextest run --release --features v1,v2,v2_lzma,v3,v3_lzma,v4_lzma {{args}}
quick-test *args: build
    cargo nextest run --release --features v1,v2,v2_lzma,v3,v3_lzma,v4_lzma -E 'not (test(large_files) | test(/slow/))' {{args}}
test_large_files *args: build
    cargo nextest run --release --features v1,v2,v2_lzma,v3,v3_lzma,v4_lzma -E 'test(large_files)' {{args}}
bench:
    cargo build --bins --release --workspace
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
