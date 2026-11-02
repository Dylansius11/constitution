// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {ERC20} from "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/// @notice Disposable testnet token. Anyone can mint up to 1,000 tokens per call.
contract MockToken is ERC20 {
    error MintLimitExceeded(uint256 amount);

    constructor() ERC20("Test USD", "tUSD") {}

    function mint(address to, uint256 amount) external {
        if (amount > 1000 ether) revert MintLimitExceeded(amount);
        _mint(to, amount);
    }
}
