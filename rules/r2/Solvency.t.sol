// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {StdInvariant} from "forge-std/StdInvariant.sol";
import {Vm} from "forge-std/Vm.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {VaultV1} from "src/demo/VaultV1.sol";
import {MockToken} from "src/demo/MockToken.sol";
import {Candidate, CandidateConfig} from "candidate.sol";

/// @dev One fixed vault account. The only state-changing entry points the explorer may call are
/// these two handlers, so every explored sequence is a sequence of vault calls by one of the two
/// fixed actors. Amounts are symbolic (Halmos) or fuzzed (Foundry) and bounded to 96 bits.
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
    Vm internal constant vm = Vm(0x7109709ECfa91a80626fF3989D68f67F5b1DD12D);
    bytes32 internal constant IMPLEMENTATION_SLOT =
        0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
    bytes32 internal expectedImplementation;

    VaultV1 internal vault;
    MockToken internal token;
    Actor internal alice;
    Actor internal bob;

    function setUp() public {
        vm.chainId(84532);
        token = new MockToken();
        address implementation = address(new Candidate());
        require(keccak256(implementation.code) == CandidateConfig.EXPECTED_RUNTIME_HASH, "candidate runtime mismatch");
        expectedImplementation = bytes32(uint256(uint160(implementation)));
        vault = VaultV1(address(new ERC1967Proxy(implementation, abi.encodeCall(VaultV1.initialize, (token)))));
        (bool initialized,) = address(vault).call(abi.encodeWithSignature("initializeV2(uint256)", uint256(1000 ether)));
        require(initialized);

        // The victim is this contract: it deposits once and never appears in a handler.
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

    /// @dev Bounded aggregate solvency and implementation-slot preservation.
    /// The invariant is checked after every explored handler call.
    function invariant_solvency() public view {
        assert(vm.load(address(vault), IMPLEMENTATION_SLOT) == expectedImplementation);
        assert(vault.totalDeposits() <= token.balanceOf(address(vault)));
    }
}
