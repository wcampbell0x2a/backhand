# Release

## Bump Versions
```
$ cargo release version [LEVEL] -p backhand -p backhand-cli --execute
$ cargo release replace -p backhand -p backhand-cli --execute
```

## Update `CHANGELOG.md`
## Update `BENCHMARK.md`

## Create MR / Merge Into Master

## Tag Release
Create tag and push to github. This will run the `.github/workflows/binaries.yml` job and create
a [Release](https://github.com/wcampbell0x2a/backhand/releases) if the CI passes.

## Publish to `crates.io`
```
$ git clean -xdf
$ cargo publish --locked -p backhand
$ cargo publish --locked -p backhand-cli
````

