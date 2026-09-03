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

## SquashFS v1 and v2 test images

No public source publishes SquashFS v1 or v2 images, so they are built from the
original tools by `generate-v1-v2-images.sh`:

```
$ ./backhand-test/generate-v1-v2-images.sh ./images
```

The script downloads `squashfs1.3r3` and `squashfs2.2-r2`, plus the LZMA SDK and
the Freetz patch that produces the AVM minor-version-76 variant. Those sources
are from 2004 and do not build on a current toolchain; the script applies the
corrections, each with a comment on why it is needed.

Publish the output and record each file's sha256 in `test-assets.toml`. Check
the gzip images first with a current `unsquashfs`, which still reads v1 and v2:

```
$ unsquashfs -d /tmp/out ./images/squashfs_v2_le.sqfs
```

A current `unsquashfs` cannot read the minor-version-76 LZMA images. Use
`sasquatch` for those, or compare them against the gzip image of the same tree.
