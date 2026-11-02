// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {Test} from "forge-std/Test.sol";
import {IERC20} from "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import {Initializable} from "@openzeppelin/contracts/proxy/utils/Initializable.sol";
import {ConstitutionExecutor} from "../src/ConstitutionExecutor.sol";
import {MockToken} from "../src/demo/MockToken.sol";
import {VaultV1} from "../src/demo/VaultV1.sol";
import {VaultV2Honest} from "../src/demo/VaultV2Honest.sol";
import {VaultV2Drain} from "../src/demo/VaultV2Drain.sol";

// Expose struct types to forge inspect without changing production storage.
contract V1Layout is VaultV1 { VaultStorage internal layout; }
contract V2Layout is VaultV2Honest { VaultV2Storage internal layout; }

contract VaultTest is Test {
    address constant VICTIM = address(0x1111);
    address constant ATTACKER = address(0x2222);
    MockToken token;
    VaultV1 v1;

    function setUp() public {
        token = new MockToken();
        v1 = new VaultV1();
    }

    function _vault(address implementation) internal returns (VaultV1 vault) {
        ConstitutionExecutor executor = new ConstitutionExecutor(address(this), address(v1), abi.encodeCall(VaultV1.initialize, (token)));
        vault = VaultV1(address(executor.proxy()));
        token.mint(VICTIM, 100 ether);
        vm.startPrank(VICTIM);
        token.approve(address(vault), type(uint256).max);
        vault.deposit(100 ether);
        vm.stopPrank();
        executor.execute(0, implementation, implementation.codehash, address(v1), uint64(block.timestamp), abi.encodeCall(VaultV2Honest.initializeV2, (1000 ether)));
        token.mint(ATTACKER, 1);
        vm.prank(ATTACKER);
        token.approve(address(vault), type(uint256).max);
    }

    function _firstThree(VaultV1 vault) internal {
        vm.startPrank(ATTACKER);
        vault.deposit(1);
        vault.withdraw(1);
        vault.deposit(1);
        vm.stopPrank();
    }

    function test_drainSequenceViolatesR2AtDepthFour() public {
        VaultV1 vault = _vault(address(new VaultV2Drain()));
        _firstThree(vault);
        uint256 recorded = vault.balanceOf(ATTACKER);
        uint256 assets = token.balanceOf(address(vault));
        assertEq(recorded, 1);
        assertEq(assets, 100 ether + 1);
        vm.prank(ATTACKER);
        vault.withdraw(assets);
        assertGt(token.balanceOf(ATTACKER), recorded);
        assertEq(token.balanceOf(address(vault)), 0);
        assertEq(vault.balanceOf(VICTIM), 100 ether);
        assertEq(vault.totalDeposits(), 100 ether);
    }

    function test_honestSameSequencePreservesSolvency() public {
        VaultV1 vault = _vault(address(new VaultV2Honest()));
        _firstThree(vault);
        vm.expectRevert(abi.encodeWithSelector(VaultV1.InsufficientBalance.selector, uint256(1), uint256(100 ether + 1)));
        vm.prank(ATTACKER);
        vault.withdraw(100 ether + 1);
        assertEq(token.balanceOf(address(vault)), vault.totalDeposits());
        assertEq(vault.balanceOf(ATTACKER), 1);
        vm.prank(ATTACKER);
        vault.withdraw(1);
        vm.prank(VICTIM);
        vault.withdraw(100 ether);
        assertEq(vault.totalDeposits(), 0);
        assertEq(token.balanceOf(VICTIM), 100 ether);
    }

    function test_drainCannotSkipSequenceStages() public {
        VaultV1 vault = _vault(address(new VaultV2Drain()));
        vm.startPrank(ATTACKER);
        vm.expectRevert(abi.encodeWithSelector(VaultV1.InsufficientBalance.selector, uint256(0), uint256(100 ether)));
        vault.withdraw(100 ether);
        vault.deposit(1);
        vm.expectRevert(abi.encodeWithSelector(VaultV1.InsufficientBalance.selector, uint256(1), uint256(100 ether + 1)));
        vault.withdraw(100 ether + 1);
        vault.withdraw(1);
        vm.expectRevert(abi.encodeWithSelector(VaultV1.InsufficientBalance.selector, uint256(0), uint256(100 ether)));
        vault.withdraw(100 ether);
        vm.stopPrank();
    }

    function test_drainRuntimePassesR3OpcodeAndSlotFilter() public {
        bytes memory code = address(new VaultV2Drain()).code;
        for (uint256 pc; pc < code.length; ++pc) {
            uint8 opcode = uint8(code[pc]);
            assertTrue(opcode != 0xff && opcode != 0xf2 && opcode != 0xf4, "forbidden executable opcode");
            if (opcode >= 0x60 && opcode <= 0x7f) pc += opcode - 0x5f;
        }
        bytes32 implementationSlot = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
        bytes32 adminSlot = 0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103;
        bytes32 beaconSlot = bytes32(uint256(keccak256("eip1967.proxy.beacon")) - 1);
        for (uint256 i; i + 32 <= code.length; ++i) {
            bytes32 word;
            assembly ("memory-safe") { word := mload(add(add(code, 32), i)) }
            assertTrue(word != implementationSlot && word != adminSlot && word != beaconSlot, "ERC-1967 slot constant");
        }
    }

    function test_depositCapAndInitializerProtection() public {
        VaultV2Honest implementation = new VaultV2Honest();
        vm.expectRevert(Initializable.InvalidInitialization.selector);
        implementation.initialize(token);
        vm.expectRevert(Initializable.InvalidInitialization.selector);
        implementation.initializeV2(1);
        VaultV1 vault = _vault(address(implementation));
        token.mint(address(this), 1000 ether);
        token.approve(address(vault), type(uint256).max);
        vm.expectRevert(abi.encodeWithSelector(VaultV2Honest.DepositCapExceeded.selector, uint256(1000 ether), uint256(1100 ether)));
        vault.deposit(1000 ether);
        vault.deposit(900 ether);
        assertEq(vault.totalDeposits(), 1000 ether);
        vm.expectRevert(Initializable.InvalidInitialization.selector);
        VaultV2Honest(address(vault)).initializeV2(2000 ether);
    }

    function test_tokenMintLimit() public {
        token.mint(address(this), 1000 ether);
        vm.expectRevert(abi.encodeWithSelector(MockToken.MintLimitExceeded.selector, uint256(1000 ether + 1)));
        token.mint(address(this), 1000 ether + 1);
        assertEq(token.symbol(), "tUSD");
        assertEq(token.decimals(), 18);
    }

    function test_zeroTokenRejected() public {
        vm.expectRevert(VaultV1.ZeroToken.selector);
        new ConstitutionExecutor(address(this), address(v1), abi.encodeCall(VaultV1.initialize, (IERC20(address(0)))));
    }
}
