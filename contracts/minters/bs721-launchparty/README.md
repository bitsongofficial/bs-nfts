# LaunchParty Fixed

A smart contract to create an incredible launch party for NFTs sale. This contract act as an orchestrator for the [bs721-base](../bs721-base/) and the [bs721-royalties](../bs721-royalties/) contracts.

## Instantiate

Upon instantiation, the contract automatically creates new instances of the bs721-base and bs721-royalties contracts.

## Execute

The contract handles the following state-changing messages:

* __Mint__: Users can mint an NFT during an active party. The mint message allows users to provide a referral address. To successfully mint a token, the user must send the exact amount of tokens required by the party. The contract then utilizes the bs721-base to mint the token from the base contract.

## Tests

The contract has been designed to have most of the logic encapsulated in standalone functions. This allows to have more simple and short multi-test since single functions can be tested with unit tests.

Tests are organized in the following way:

* _unit tests_: Unit tests are placed at the bottom of each file for which the logic is tested.

* _multi tests_: Multi tests are placed in the [multitest](./src/multitest/) folder and are designed to use a suite to simplify environment creation and interaction.

## License

This project is licensed under the Apache License - see the LICENSE-APACHE file for details.
