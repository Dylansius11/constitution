// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {Test, Vm} from "forge-std/Test.sol";
import {ProxyAdmin} from "@openzeppelin/contracts/proxy/transparent/ProxyAdmin.sol";
import {ITransparentUpgradeableProxy} from "@openzeppelin/contracts/proxy/transparent/TransparentUpgradeableProxy.sol";
import {Initializable} from "@openzeppelin/contracts/proxy/utils/Initializable.sol";
import {ConstitutionExecutor} from "../src/ConstitutionExecutor.sol";
import {MockToken} from "../src/demo/MockToken.sol";
import {VaultV1} from "../src/demo/VaultV1.sol";
import {VaultV2Honest} from "../src/demo/VaultV2Honest.sol";

contract ExecutorTest is Test {
    bytes32 constant IMPLEMENTATION_SLOT = 0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc;
    bytes32 constant ADMIN_SLOT = 0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103;
    address constant SIGNER = address(0x5151);
    MockToken token;
    VaultV1 v1;
    VaultV2Honest v2;
    ConstitutionExecutor executor;

    event Upgraded(address indexed implementation);
    event UpgradeExecuted(bytes32 indexed certId, address indexed previousImplementation, address indexed newImplementation, bytes32 newImplementationCodehash);
    event SignerRotated(bytes32 indexed changeId, address indexed previousSigner, address indexed newSigner);

    function setUp() public {
        token = new MockToken();
        v1 = new VaultV1();
        v2 = new VaultV2Honest();
        executor = new ConstitutionExecutor(SIGNER, address(v1), abi.encodeCall(VaultV1.initialize, (token)));
    }

    function _execute(bytes32 id, bytes memory init) internal {
        vm.prank(SIGNER);
        executor.execute(id, address(v2), address(v2).codehash, address(v1), uint64(block.timestamp), init);
    }

    function _slot() internal view returns (address) {
        return address(uint160(uint256(vm.load(address(executor.proxy()), IMPLEMENTATION_SLOT))));
    }

    function test_selectors() public pure {
        assertEq(ConstitutionExecutor.execute.selector, bytes4(0x90c70cd8));
        assertEq(ConstitutionExecutor.rotateSigner.selector, bytes4(0xfa2e9b8f));
        assertEq(ProxyAdmin.upgradeAndCall.selector, bytes4(0x9623609d));
        assertEq(VaultV2Honest.initializeV2.selector, bytes4(0x5b4e128c));
    }

    function test_T_EXE_1_nonSigner() public {
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.NotSigner.selector, address(this)));
        executor.execute(0, address(v2), address(v2).codehash, address(v1), 0, "");
    }

    function test_T_EXE_2_reusedCertificate() public {
        _execute(bytes32(uint256(1)), "");
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.CertificateAlreadyUsed.selector, bytes32(uint256(1))));
        _execute(bytes32(uint256(1)), "");
    }

    function test_T_EXE_2_expiredDeadline() public {
        vm.warp(100);
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.DeadlinePassed.selector, uint64(99), uint256(100)));
        vm.prank(SIGNER);
        executor.execute(0, address(v2), address(v2).codehash, address(v1), 99, "");
    }

    function test_T_EXE_2_noCode() public {
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.ImplementationHasNoCode.selector, address(0x1234)));
        vm.prank(SIGNER);
        executor.execute(0, address(0x1234), 0, address(v1), uint64(block.timestamp), "");
    }

    function test_T_EXE_2_wrongCodehash() public {
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.CodehashMismatch.selector, bytes32(0), address(v2).codehash));
        vm.prank(SIGNER);
        executor.execute(0, address(v2), 0, address(v1), uint64(block.timestamp), "");
    }

    function test_T_EXE_2_wrongCurrentImplementation() public {
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.CurrentImplementationMismatch.selector, address(v2), address(v1)));
        vm.prank(SIGNER);
        executor.execute(0, address(v2), address(v2).codehash, address(v2), uint64(block.timestamp), "");
    }

    function test_T_EXE_3_emptyInitialization() public {
        vm.expectEmit(true, false, false, true, address(executor.proxy()));
        emit Upgraded(address(v2));
        vm.expectEmit(true, true, true, true, address(executor));
        emit UpgradeExecuted(0, address(v1), address(v2), address(v2).codehash);
        _execute(0, "");
        assertEq(_slot(), address(v2));
        assertEq(executor.currentImplementation(), address(v2));
        assertTrue(executor.usedCertificate(0));
    }

    function test_T_EXE_3_initializeV2PreservesDeposits() public {
        VaultV1 vault = VaultV1(address(executor.proxy()));
        token.mint(address(this), 10 ether);
        token.approve(address(vault), 10 ether);
        vault.deposit(10 ether);
        vm.expectEmit(true, false, false, true, address(vault));
        emit Upgraded(address(v2));
        vm.expectEmit(true, true, true, true, address(executor));
        emit UpgradeExecuted(0, address(v1), address(v2), address(v2).codehash);
        _execute(0, abi.encodeCall(VaultV2Honest.initializeV2, (100 ether)));
        assertEq(_slot(), address(v2));
        assertEq(VaultV2Honest(address(vault)).depositCap(), 100 ether);
        assertEq(vault.balanceOf(address(this)), 10 ether);
        assertEq(vault.totalDeposits(), 10 ether);
        vault.withdraw(10 ether);
        assertEq(token.balanceOf(address(this)), 10 ether);
    }

    function test_T_EXE_3_revertingInitializerRollsBack() public {
        _execute(0, abi.encodeCall(VaultV2Honest.initializeV2, (100 ether)));
        VaultV2Honest next = new VaultV2Honest();
        vm.expectRevert(Initializable.InvalidInitialization.selector);
        vm.prank(SIGNER);
        executor.execute(bytes32(uint256(2)), address(next), address(next).codehash, address(v2), uint64(block.timestamp), abi.encodeCall(VaultV2Honest.initializeV2, (200 ether)));
        assertFalse(executor.usedCertificate(bytes32(uint256(2))));
        assertEq(executor.currentImplementation(), address(v2));
        assertEq(_slot(), address(v2));
    }

    function test_T_EXE_4_atomicBootstrap() public {
        vm.recordLogs();
        ConstitutionExecutor deployed = new ConstitutionExecutor(SIGNER, address(v1), abi.encodeCall(VaultV1.initialize, (token)));
        Vm.Log[] memory logs = vm.getRecordedLogs();
        address admin;
        bool found;
        for (uint256 i; i < logs.length; ++i) {
            if (logs[i].emitter == address(deployed.proxy()) && logs[i].topics[0] == keccak256("AdminChanged(address,address)")) {
                (address previous, address emittedAdmin) = abi.decode(logs[i].data, (address, address));
                assertEq(previous, address(0));
                admin = emittedAdmin;
                found = true;
            }
        }
        assertTrue(found);
        address computed = address(uint160(uint256(keccak256(abi.encodePacked(hex"d694", address(deployed.proxy()), hex"01")))));
        assertEq(admin, computed);
        assertEq(address(deployed.proxyAdmin()), computed);
        assertEq(address(uint160(uint256(vm.load(address(deployed.proxy()), ADMIN_SLOT)))), computed);
        ProxyAdmin admin_ = deployed.proxyAdmin();
        ITransparentUpgradeableProxy proxy_ = deployed.proxy();
        assertEq(admin_.owner(), address(deployed));
        // Cache the getters: expectRevert applies to the very next external call.
        vm.expectRevert(abi.encodeWithSignature("OwnableUnauthorizedAccount(address)", address(this)));
        admin_.upgradeAndCall(proxy_, address(v2), "");
        vm.expectRevert(abi.encodeWithSignature("OwnableUnauthorizedAccount(address)", address(this)));
        admin_.transferOwnership(address(this));
    }

    function test_T_EXE_4_zeroBootstrapAddresses() public {
        vm.expectRevert(ConstitutionExecutor.ZeroAddress.selector);
        new ConstitutionExecutor(address(0), address(v1), "");
        vm.expectRevert(ConstitutionExecutor.ZeroAddress.selector);
        new ConstitutionExecutor(SIGNER, address(0), "");
    }

    function test_rotateSignerChecksAndSingleUse() public {
        address next = address(0x5252);
        vm.expectEmit(true, true, true, true, address(executor));
        emit SignerRotated(0, SIGNER, next);
        vm.prank(SIGNER);
        executor.rotateSigner(next, 0, uint64(block.timestamp));
        assertEq(executor.signer(), next);
        assertTrue(executor.usedChange(0));
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.NotSigner.selector, SIGNER));
        vm.prank(SIGNER);
        executor.rotateSigner(SIGNER, 0, 0);
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.ChangeAlreadyUsed.selector, bytes32(0)));
        vm.prank(next);
        executor.rotateSigner(SIGNER, 0, 0);
    }

    function test_rotateSignerRejectsExpiredAndZero() public {
        vm.warp(100);
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.DeadlinePassed.selector, uint64(99), uint256(100)));
        vm.prank(SIGNER);
        executor.rotateSigner(address(0), 0, 99);
        vm.expectRevert(ConstitutionExecutor.ZeroAddress.selector);
        vm.prank(SIGNER);
        executor.rotateSigner(address(0), 0, 100);
        assertFalse(executor.usedChange(0));
        assertEq(executor.signer(), SIGNER);
    }

    function testFuzz_T_EXE_5_onlySignerChangesAuthority(address caller, bytes32 id, address impl, bytes32 hash, address expected, uint64 deadline, bytes calldata init, address newSigner) public {
        vm.assume(caller != SIGNER);
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.NotSigner.selector, caller));
        vm.prank(caller);
        executor.execute(id, impl, hash, expected, deadline, init);
        vm.expectRevert(abi.encodeWithSelector(ConstitutionExecutor.NotSigner.selector, caller));
        vm.prank(caller);
        executor.rotateSigner(newSigner, id, deadline);
        assertEq(_slot(), address(v1));
        assertEq(executor.signer(), SIGNER);
        assertFalse(executor.usedCertificate(id));
        assertFalse(executor.usedChange(id));
    }

    function testFuzz_T_EXE_5_signerCanUpgradeAndRotate(bytes32 id, address newSigner) public {
        vm.assume(newSigner != address(0));
        _execute(id, "");
        assertEq(_slot(), address(v2));
        vm.prank(SIGNER);
        executor.rotateSigner(newSigner, id, uint64(block.timestamp));
        assertEq(executor.signer(), newSigner);
    }
}
