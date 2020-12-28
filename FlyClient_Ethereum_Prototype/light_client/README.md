# Light client implementation

This application can connect to a light_server application, fetch a proof for
the current state of the blockchain and verify the proof. It uses the `Parity`
implementation of `Ethash`, so that block header verification is possible.
