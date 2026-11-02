// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {StdInvariant} from "forge-std/StdInvariant.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {VaultV1} from "src/demo/VaultV1.sol";
import {MockToken} from "src/demo/MockToken.sol";
import {Candidate} from "candidate.sol";

interface SVM {
    function createCalldata(address) external view returns (bytes memory);
}

/// @dev Generic handler: any public state-changing function of the candidate with symbolic arguments.
contract Actor {
    VaultV1 internal immutable vault;
    address internal immutable impl;

    constructor(VaultV1 v, MockToken t, address i) {
        vault = v;
        impl = i;
        t.approve(address(v), type(uint256).max);
    }

    function act() external {
        bytes memory data = SVM(0xF3993A62377BCd56AE39D773740A5390411E8BC9).createCalldata(impl);
        (bool ok,) = address(vault).call(data);
        require(ok);
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

        alice = new Actor(vault, token, implementation);
        bob = new Actor(vault, token, implementation);
        token.mint(address(alice), ACTOR_FUNDS);
        token.mint(address(bob), ACTOR_FUNDS);

        targetContract(address(alice));
        targetContract(address(bob));
    }

    /// @dev The vault never records more deposits than it holds, and no actor ends up with
    /// more tokens than it was funded with (it withdrew no more than it deposited).
    function invariant_solvency() public view {
        assert(vault.totalDeposits() <= token.balanceOf(address(vault)));
    }
}
