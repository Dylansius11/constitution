// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {Test} from "forge-std/Test.sol";
import {Initializable} from "@openzeppelin/contracts/proxy/utils/Initializable.sol";
import {ConstitutionExecutor} from "../src/ConstitutionExecutor.sol";
import {MockToken} from "../src/demo/MockToken.sol";
import {VaultV2Honest} from "../src/demo/VaultV2Honest.sol";
import {VaultV3Honest} from "../src/demo/VaultV3Honest.sol";

contract VaultV3Test is Test {
    function test_v2ToV3PreservesBalancesAndReportsCapacity() public {
        MockToken token = new MockToken();
        VaultV2Honest v2 = new VaultV2Honest();
        ConstitutionExecutor executor = new ConstitutionExecutor(address(this), address(v2), abi.encodeCall(VaultV2Honest.initialize, (token)));
        VaultV2Honest vault = VaultV2Honest(address(executor.proxy()));
        vault.initializeV2(1000);
        token.mint(address(this), 1000);
        token.approve(address(vault), 1000);
        vault.deposit(100);
        address depositor = address(0x1234);
        token.mint(depositor, 200);
        vm.startPrank(depositor);
        token.approve(address(vault), 200);
        vault.deposit(200);
        vm.stopPrank();
        VaultV3Honest v3 = new VaultV3Honest();
        executor.execute(bytes32(uint256(1)), address(v3), address(v3).codehash, address(v2), uint64(block.timestamp), "");
        VaultV3Honest upgraded = VaultV3Honest(address(vault));
        assertEq(upgraded.balanceOf(address(this)), 100);
        assertEq(upgraded.balanceOf(depositor), 200);
        assertEq(upgraded.totalDeposits(), 300);
        assertEq(upgraded.depositCap(), 1000);
        assertEq(upgraded.remainingCapacity(), 700);
        assertEq(token.balanceOf(address(upgraded)), 300);
        upgraded.deposit(700);
        assertEq(upgraded.remainingCapacity(), 0);
        vm.expectRevert(abi.encodeWithSelector(VaultV2Honest.DepositCapExceeded.selector, uint256(1000), uint256(1001)));
        upgraded.deposit(1);
        upgraded.withdraw(800);
        vm.prank(depositor);
        upgraded.withdraw(200);
        assertEq(upgraded.totalDeposits(), 0);
        assertEq(token.balanceOf(address(upgraded)), 0);
        assertEq(token.balanceOf(address(this)), 1000);
        assertEq(token.balanceOf(depositor), 200);
        assertEq(upgraded.remainingCapacity(), 1000);
        vm.expectRevert(Initializable.InvalidInitialization.selector);
        upgraded.initializeV2(2000);
    }

    function test_v3ImplementationCannotInitialize() public {
        VaultV3Honest v3 = new VaultV3Honest();
        vm.expectRevert(Initializable.InvalidInitialization.selector);
        v3.initializeV2(1000);
    }
}
