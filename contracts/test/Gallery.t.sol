// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {Test} from "forge-std/Test.sol";
import {ConstitutionExecutor} from "../src/ConstitutionExecutor.sol";
import {MockToken} from "../src/demo/MockToken.sol";
import {VaultV1} from "../src/demo/VaultV1.sol";
import {VaultV2Honest} from "../src/demo/VaultV2Honest.sol";
import {VaultV2Drain} from "../src/demo/VaultV2Drain.sol";

interface EmergencyVault {
    function emergencyShutdown(address payable recipient) external;
}

/// @notice Local counterexamples, not testnet evidence or claims of general rule coverage.
contract GalleryTest is Test {
    address constant DEPOSITOR = address(0x1111);
    address constant REBATE_USER = address(0x2222);
    address payable constant TREASURY = payable(address(0x3333));
    MockToken token;
    VaultV1 original;
    VaultV1 vault;
    ConstitutionExecutor executor;

    function setUp() public {
        token = new MockToken();
        original = new VaultV1();
        executor = new ConstitutionExecutor(
            address(this), address(original), abi.encodeCall(VaultV1.initialize, (token))
        );
        vault = VaultV1(address(executor.proxy()));
        token.mint(DEPOSITOR, 100 ether);
        vm.startPrank(DEPOSITOR);
        token.approve(address(vault), type(uint256).max);
        vault.deposit(100 ether);
        vm.stopPrank();
    }

    function upgrade(address candidate, bytes memory init) internal {
        executor.execute(0, candidate, candidate.codehash, address(original), uint64(block.timestamp), init);
    }

    function test_storageShuffleLosesRecordedDepositorBalance() public {
        address candidate = deployCode("VaultV2StorageShuffle.sol:VaultV2StorageShuffle");
        upgrade(candidate, "");
        assertEq(vault.balanceOf(DEPOSITOR), 0, "mapping moved away from V1 entries");
        assertEq(token.balanceOf(address(vault)), 100 ether, "assets did not move");
        vm.expectRevert(abi.encodeWithSelector(VaultV1.InsufficientBalance.selector, 0, 100 ether));
        vm.prank(DEPOSITOR);
        vault.withdraw(100 ether);
    }

    function test_shutdownForcesNativeTransferButPraguePreservesOldCodeAndTokenDeposits() public {
        address candidate = deployCode("VaultV2SelfDestruct.sol:VaultV2SelfDestruct");
        upgrade(candidate, abi.encodeCall(VaultV2Honest.initializeV2, (1000 ether)));
        vm.deal(address(vault), 1 ether);
        bytes32 beforeCode = address(vault).codehash;
        vm.prank(address(0x4444)); // Fixed local fixture emergency council, not the proxy admin.
        EmergencyVault(address(vault)).emergencyShutdown(TREASURY);
        assertEq(TREASURY.balance, 1 ether);
        assertEq(address(vault).balance, 0);
        // EIP-6780: an existing proxy is NOT deleted on Prague. Never claim ERC20 funds strand here.
        assertEq(address(vault).codehash, beforeCode);
        assertEq(vault.balanceOf(DEPOSITOR), 100 ether);
        vm.prank(DEPOSITOR);
        vault.withdraw(100 ether);
        assertEq(token.balanceOf(DEPOSITOR), 100 ether);
    }

    function test_shutdownRejectsNonOwner() public {
        address candidate = deployCode("VaultV2SelfDestruct.sol:VaultV2SelfDestruct");
        upgrade(candidate, abi.encodeCall(VaultV2Honest.initializeV2, (1000 ether)));
        vm.expectRevert(bytes4(keccak256("NotShutdownOwner()")));
        vm.prank(DEPOSITOR);
        EmergencyVault(address(vault)).emergencyShutdown(TREASURY);
    }

    function test_forbiddenInitExceedsCommittedUint128RiskLimit() public {
        upgrade(address(new VaultV2Honest()), abi.encodeCall(VaultV2Honest.initializeV2, (uint256(1) << 128)));
        uint256 cap = VaultV2Honest(address(vault)).depositCap();
        assertEq(cap, uint256(1) << 128);
        assertGt(cap, type(uint128).max, "violates the owner-committed R4 ceiling");
        assertEq(uint128(cap), 0, "a uint128 consumer cannot represent this cap");
        // This is a parameter-policy violation, not a claim that honest bytecode becomes insolvent.
        assertEq(vault.totalDeposits(), token.balanceOf(address(vault)));
    }

    function test_honestUpgradePreservesBalancesAndEnforcesAllowedCap() public {
        upgrade(address(new VaultV2Honest()), abi.encodeCall(VaultV2Honest.initializeV2, (1000 ether)));
        assertEq(vault.balanceOf(DEPOSITOR), 100 ether);
        assertLe(VaultV2Honest(address(vault)).depositCap(), type(uint128).max);
        token.mint(DEPOSITOR, 1000 ether);
        vm.expectRevert(abi.encodeWithSelector(VaultV2Honest.DepositCapExceeded.selector, 1000 ether, 1100 ether));
        vm.prank(DEPOSITOR);
        vault.deposit(1000 ether);
        vm.prank(DEPOSITOR);
        vault.withdraw(100 ether);
        assertEq(vault.totalDeposits(), 0);
    }

    function test_drainRebateLeavesDepositsUnbacked() public {
        upgrade(address(new VaultV2Drain()), abi.encodeCall(VaultV2Honest.initializeV2, (1000 ether)));
        token.mint(REBATE_USER, 1);
        vm.startPrank(REBATE_USER);
        token.approve(address(vault), 2);
        vault.deposit(1);
        vault.withdraw(1);
        vault.deposit(1);
        vault.withdraw(100 ether + 1);
        vm.stopPrank();
        assertEq(token.balanceOf(address(vault)), 0);
        assertEq(vault.totalDeposits(), 100 ether);
        assertEq(vault.balanceOf(DEPOSITOR), 100 ether);
    }
}
