# Light Server implementation

This application can connect to an Ethereum node like `Geth` or `Parity` via the
JSON RPC interface. It then spawns a server, which builds the corresponding
Merkle Mountain Range tree for the blockchain. Clients can connect to this
server and fetch a proof for the latest state of the blockchain.
