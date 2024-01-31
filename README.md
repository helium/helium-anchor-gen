[![Rust](https://github.com/lthiery/helium-anchor-gen/actions/workflows/rust.yml/badge.svg)](https://github.com/lthiery/helium-anchor-gen/actions/workflows/rust.yml)

# helium-anchor-gen

Generates a Rust CPI client for the Helium Program Library. This is intended to avoid the strict dependencies declared in the solana-program-library that gets inherited by the helium-program-library.

## IDL Updates

The files in the `idl` directory are output from build `helium-program-library`.

With `anchor-cli 0.26`, run `anchor build` in the `helium-program-library`. The output from `target/idl` may be copied over to the `idl` directory here.

## Installing anchor-cli 0.26

```
sh -c "$(curl -sSfL https://release.solana.com/v1.17.15/install)"
ln -s ~/.local/share/solana/install/active_release/bin/cargo-build-bpf ~/.cargo/bin/
ln -s ~/.local/share/solana/install/active_release/bin/cargo-build-sbf ~/.cargo/bin/
rustup toolchain install 1.70
cargo +1.70-x86_64-unknown-linux-gnu install --bin anchor --git https://github.com/coral-xyz/anchor --tag v0.26.0 anchor-cli --force
```

## Other notes

The crates under `programs` exist to generate the CPI crates. Do not add code there!

For those programs that benefit from importing of some functions from `helium-program-library`, the CPI types are re-exported and the helpful functions are defined under `src`.
