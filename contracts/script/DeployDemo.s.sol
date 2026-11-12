// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

import {Script, console2} from "forge-std/Script.sol";
import {ConstitutionExecutor} from "../src/ConstitutionExecutor.sol";
import {MockToken} from "../src/demo/MockToken.sol";
import {VaultV1} from "../src/demo/VaultV1.sol";

/// @notice Separate bootstrap; never changes the original deployment or its E2E interface.
/// Gallery implementations are deployed by scripts/demo/deploy.sh from the gallery at runtime.
contract DeployDemo is Script {
    function run() external {
        address signer = vm.envAddress("SIGNER");
        require(block.chainid == 84532, "Demo requires Base Sepolia chain id");
        vm.startBroadcast();
        MockToken token = new MockToken();
        VaultV1 implementation = new VaultV1();
        ConstitutionExecutor executor = new ConstitutionExecutor(
            signer, address(implementation), abi.encodeCall(VaultV1.initialize, (token))
        );
        vm.stopBroadcast();
        require(executor.proxyAdmin().owner() == address(executor), "Unexpected ProxyAdmin owner");
        console2.log("token", address(token));
        console2.log("vaultV1Implementation", address(implementation));
        console2.log("executor", address(executor));
        console2.log("proxy", address(executor.proxy()));
        console2.log("proxyAdmin", address(executor.proxyAdmin()));
    }
}
