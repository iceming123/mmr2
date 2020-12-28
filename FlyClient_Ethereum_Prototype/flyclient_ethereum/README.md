# Flyclient Implementation

This repository consists of an implementation of the
[Flyclient](https://eprint.iacr.org/2019/226) Protocol for the Ethereum network
written in Rust. The `MerkleTree` struct can be used to build a Merkle Mountain
Range tree. In combination with the `NonInteractiveProofVariableDifficulty`
struct proofs can be generated and verified with different parameter settings.
Therefore this library can be used on the server and client side.
