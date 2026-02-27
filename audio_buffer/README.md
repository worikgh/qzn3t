# Qzn3t/AudioBuffer

A file backed audio buffer

## Testing

Coverage
---

```sh
sudo aptitude install grcov
export RUSTFLAGS="-C instrument-coverage"
export LLVM_PROFILE_FILE="target/coverage/%p-%m.profraw"
export CARGO_INCREMENTAL=0
cargo test --tests
llvm-profdata merge -sparse target/coverage/*.profraw -o target/coverage/merged.profdata
grcov . -s . --binary-path target/debug/ -t html --branch --llvm -o target/coverage/html
```
