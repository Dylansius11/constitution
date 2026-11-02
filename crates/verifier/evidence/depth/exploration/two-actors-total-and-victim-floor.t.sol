// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {StdInvariant} from "forge-std/StdInvariant.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {VaultV1} from "src/demo/VaultV1.sol";
import {MockToken} from "src/demo/MockToken.sol";
import {Candidate} from "candidate.sol";

/// @dev One fixed vault account. The only state-changing entry points Halmos may call are
/// these two handlers, so every explored sequence is a sequence of vault calls by one of
/// the two fixed actors. Amounts are symbolic and bounded to 96 bits.
contract Actor {
    VaultV1 internal immutable vault;

    constructor(VaultV1 v, MockToken t) {
        vault = v;
        t.approve(address(v), type(uint256).max);
    }

    function deposit(uint96 amount) external {
        vault.deposit(amount);
    }

    function withdraw(uint96 amount) external {
        vault.withdraw(amount);
    }
}

contract Solvency is StdInvariant {
    uint256 internal constant VICTIM_DEPOSIT = 100 ether;
    uint256 internal constant ACTOR_FUNDS = 10 ether;

    VaultV1 internal vault;
    MockToken internal token;
    Actor internal alice;
    Actor internal bob;

    function setUp() public {
        token = new MockToken();
        address implementation = address(new Candidate());
        vault = VaultV1(address(new ERC1967Proxy(implementation, abi.encodeCall(VaultV1.initialize, (token)))));
        (bool initialized,) = address(vault).call(abi.encodeWithSignature("initializeV2(uint256)", uint256(1000 ether)));
        require(initialized);

        token.mint(address(this), VICTIM_DEPOSIT);
        token.approve(address(vault), type(uint256).max);
        vault.deposit(VICTIM_DEPOSIT);

        alice = new Actor(vault, token);
        bob = new Actor(vault, token);
        token.mint(address(alice), ACTOR_FUNDS);
        token.mint(address(bob), ACTOR_FUNDS);

        targetContract(address(alice));
        targetContract(address(bob));
    }

    /// @dev The vault never records more deposits than it holds, and no actor ends up with
    /// more tokens than it was funded with (it withdrew no more than it deposited).
    function invariant_solvency() public view {
        assert(vault.totalDeposits() <= token.balanceOf(address(vault)));
        assert(token.balanceOf(address(vault)) >= VICTIM_DEPOSIT);
    }
}
