# EVM Simulation with Tracing on Forked Ethereum Mainnet

###  Answer for Paradigm Fellowship Question One 
## Overview


This implements an EVM simulation with tracing, running against a forked Ethereum mainnet.

The core of this implementation is a custom minimal forked database that utilizes RPC calls to fetch state.


## Usage

Run the simulation with:

```
cargo run --release
```

This will execute a series of predefined interactions with WETH on the forked Ethereum mainnet.


