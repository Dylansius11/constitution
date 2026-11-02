// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {TransparentUpgradeableProxy, ITransparentUpgradeableProxy} from "@openzeppelin/contracts/proxy/transparent/TransparentUpgradeableProxy.sol";
import {ProxyAdmin} from "@openzeppelin/contracts/proxy/transparent/ProxyAdmin.sol";

/// @notice Upgrade authority for one proxy. Only the gate-derived signer can act.
contract ConstitutionExecutor {
    ITransparentUpgradeableProxy public immutable proxy;
    ProxyAdmin public immutable proxyAdmin;
    address public signer;
    address public currentImplementation;
    mapping(bytes32 => bool) public usedCertificate;
    mapping(bytes32 => bool) public usedChange;

    event UpgradeExecuted(bytes32 indexed certId, address indexed previousImplementation, address indexed newImplementation, bytes32 newImplementationCodehash);
    event SignerRotated(bytes32 indexed changeId, address indexed previousSigner, address indexed newSigner);

    error NotSigner(address caller);
    error CertificateAlreadyUsed(bytes32 certId);
    error ChangeAlreadyUsed(bytes32 changeId);
    error DeadlinePassed(uint64 deadline, uint256 timestamp);
    error ImplementationHasNoCode(address implementation);
    error CodehashMismatch(bytes32 expected, bytes32 actual);
    error CurrentImplementationMismatch(address expected, address actual);
    error ZeroAddress();
    error BootstrapMismatch(address computedAdmin);

    constructor(address signer_, address initialImplementation, bytes memory initData) {
        if (signer_ == address(0) || initialImplementation == address(0)) revert ZeroAddress();
        ITransparentUpgradeableProxy deployed = ITransparentUpgradeableProxy(address(new TransparentUpgradeableProxy(initialImplementation, address(this), initData)));
        // EIP-161 starts a new contract's CREATE nonce at one. OZ creates its admin first.
        address computedAdmin = address(uint160(uint256(keccak256(abi.encodePacked(hex"d694", address(deployed), hex"01")))));
        ProxyAdmin admin = ProxyAdmin(computedAdmin);
        if (admin.owner() != address(this)) revert BootstrapMismatch(computedAdmin);
        proxy = deployed;
        proxyAdmin = admin;
        currentImplementation = initialImplementation;
        signer = signer_;
    }

    function execute(bytes32 certId, address newImpl, bytes32 newImplCodehash, address expectedCurrentImpl, uint64 deadline, bytes calldata initCalldata) external {
        if (msg.sender != signer) revert NotSigner(msg.sender);
        if (usedCertificate[certId]) revert CertificateAlreadyUsed(certId);
        if (block.timestamp > deadline) revert DeadlinePassed(deadline, block.timestamp);
        if (newImpl.code.length == 0) revert ImplementationHasNoCode(newImpl);
        bytes32 actualCodehash = newImpl.codehash;
        if (actualCodehash != newImplCodehash) revert CodehashMismatch(newImplCodehash, actualCodehash);
        address previous = currentImplementation;
        if (previous != expectedCurrentImpl) revert CurrentImplementationMismatch(expectedCurrentImpl, previous);
        usedCertificate[certId] = true;
        currentImplementation = newImpl;
        proxyAdmin.upgradeAndCall(proxy, newImpl, initCalldata);
        emit UpgradeExecuted(certId, previous, newImpl, newImplCodehash);
    }

    function rotateSigner(address newSigner, bytes32 changeId, uint64 deadline) external {
        if (msg.sender != signer) revert NotSigner(msg.sender);
        if (usedChange[changeId]) revert ChangeAlreadyUsed(changeId);
        if (block.timestamp > deadline) revert DeadlinePassed(deadline, block.timestamp);
        if (newSigner == address(0)) revert ZeroAddress();
        usedChange[changeId] = true;
        address previous = signer;
        signer = newSigner;
        emit SignerRotated(changeId, previous, newSigner);
    }
}
