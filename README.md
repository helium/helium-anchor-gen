[![Rust](https://github.com/lthiery/helium-anchor-gen/actions/workflows/rust.yml/badge.svg)](https://github.com/lthiery/helium-anchor-gen/actions/workflows/rust.yml)

# helium-anchor-gen

Generates a Rust CPI client for the Helium Program Library. This is intended to avoid the strict dependencies declared 
in the solana-program-library that gets inherited by the helium-program-library.

## IDL Updates

There is a GitHub Action that runs every hour to check for updates to the IDL files in the `helium-program-library`. If
there are updates, the action will create a PR to update the IDL files in this repository.

If you want to modify them by hand locally, you can do so by running `anchor build` in the `helium-program-library` and
copying the output from `target/idl` to the `idl` directory here.


## Adding helper functions

The crates under `programs` exist to generate the CPI crates. Do not add code there!

For those programs that benefit from importing of some functions from `helium-program-library`, the CPI types are 
re-exported and the helpful functions are defined under `src`.

# Caveat

The downside of this approach is that the types are duplicated across programs. Each program has a local definition for
related types. This leads to identical types being defined in different programs. For example, `voter-stake-registry`
defines `PositionV0` and `helium-sub-daos` does as well (instead of sharing the definition from VSR as it does when 
importing HPL directly).