// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;
contract OsakaOpcode {
    function check_clz(uint256 x) public pure {
        uint256 zeros;
        assembly { zeros := clz(x) }
        assert(zeros <= 256);
    }
}
