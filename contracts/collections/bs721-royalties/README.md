# BS721 Royalties

A smart contract that enables the distribution of its balance among a list of contributors, proportionally based on their shares.

## Instantiate

To instantiate the contract, you need to provide the following parameters:

* __denom__: The native token's denomination that is used as contract royalties.

* __contributors__: The list of contributors that will participate in royalties distributions.

Each contributor is described by a custom-type `ContributorMsg` which consists of the following fields:

* __address__: The address of the contributor.

* __role__: The role of the contributor. This field cannot be left empty but can be passed an empty string.

* __shares__: Shares associated with the contributor. This value is used to compute the total shares by summing the shares field of each ContributorMsg. The percentage of royalties associated with each contributor is then determined based on their shares.

## Execute

The contract handles two state-changing messages:

* __Distribute__: This message causes the contract to distribute royalties proportionally among contributors. This logic can result in an error if there are not enough tokens to be distributed. Distributed royalties are not sent directly to contributors but are stored as a field within the contract's state. Any user can send this message.

* __Withdraw__: This message can be sent only by a contributor and results in a `BankMsg` to send the accrued royalties to the sender.

## Query

The contract supports the following queries:

* __ListContributors__: allows to retrieve information associated with each contributor.

* __WithdrawableAmount__: returns the sum of the royalties all contributors can withdraw.

* __DistributableAmount__: returns the difference between the contract's balance and the number of tokens that can be withdrawn as royalties.

## License

This project is licensed under the Apache License - see the LICENSE-APACHE file for details.
