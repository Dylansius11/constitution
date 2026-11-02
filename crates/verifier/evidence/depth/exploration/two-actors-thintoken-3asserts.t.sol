// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {StdInvariant} from "forge-std/StdInvariant.sol";
import {ERC1967Proxy} from "@openzeppelin/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {VaultV1} from "src/demo/VaultV1.sol";
import {Candidate} from "candidate.sol";

contract ThinToken {
    mapping(address => uint256) public balanceOf;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address, uint256) external pure returns (bool) {
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

contract Actor {
    VaultV1 internal immutable vault;

    constructor(VaultV1 v) {
        vault = v;
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
    ThinToken internal token;
    Actor internal alice;
    Actor internal bob;

    function setUp() public {
        token = new ThinToken();
        address implementation = address(new Candidate());
        vault = VaultV1(address(new ERC1967Proxy(implementation, abi.encodeCall(VaultV1.initialize, (IERC20(address(token)))))));
        (bool initialized,) = address(vault).call(abi.encodeWithSignature("initializeV2(uint256)", uint256(1000 ether)));
        require(initialized);

        token.mint(address(this), VICTIM_DEPOSIT);
        vault.deposit(VICTIM_DEPOSIT);

        alice = new Actor(vault);
        bob = new Actor(vault);
        token.mint(address(alice), ACTOR_FUNDS);
        token.mint(address(bob), ACTOR_FUNDS);

        targetContract(address(alice));
        targetContract(address(bob));
    }

    function invariant_solvency() public view {
        assert(vault.totalDeposits() <= token.balanceOf(address(vault)));
        assert(token.balanceOf(address(alice)) <= ACTOR_FUNDS);
        assert(token.balanceOf(address(bob)) <= ACTOR_FUNDS);
    }
}
