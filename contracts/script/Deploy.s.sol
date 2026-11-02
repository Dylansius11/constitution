// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {Script, console2} from "forge-std/Script.sol";
import {ConstitutionExecutor} from "../src/ConstitutionExecutor.sol";
import {MockToken} from "../src/demo/MockToken.sol";
import {VaultV1} from "../src/demo/VaultV1.sol";

/// @notice Deploys MockToken, the VaultV1 implementation and the executor, which creates the proxy.
/// Usage: SIGNER=<gate-derived EOA> forge script script/Deploy.s.sol --rpc-url <url> --account deployer --broadcast
contract Deploy is Script {
    function run() external returns (ConstitutionExecutor executor) {
        address signer = vm.envAddress("SIGNER");
        vm.startBroadcast();
        MockToken token = new MockToken();
        VaultV1 implementation = new VaultV1();
        executor = new ConstitutionExecutor(signer, address(implementation), abi.encodeCall(VaultV1.initialize, (token)));
        vm.stopBroadcast();

        require(executor.proxyAdmin().owner() == address(executor), "ProxyAdmin owner is not the executor");
        console2.log("signer", signer);
        console2.log("token", address(token));
        console2.log("vaultV1Implementation", address(implementation));
        console2.log("executor", address(executor));
        console2.log("proxy", address(executor.proxy()));
        console2.log("proxyAdmin", address(executor.proxyAdmin()));
    }
}
