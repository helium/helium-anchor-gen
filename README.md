# helium-anchor-gen

Generates a Rust CPI client for the Helium Program Library. This is intended to avoid the strict dependencies declared in the solana-program-library that gets inherited by the helium-program-library.

## IDL Updates

The files in the `idl` directory are output from build `helium-program-library`.

With `anchor-cli 0.26`, run `anchor build` in the `helium-program-library`. The output from `target/idl` may be copied over to the `idl` directory here.

## Other notes

The crates under `programs` exist to generate the CPI crates. Do not add code there!

For those programs that benefit from importing of some functions from `helium-program-library`, the CPI types are re-exported and the helpful functions are defined under `src`.
