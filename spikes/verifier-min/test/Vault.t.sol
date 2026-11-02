// SPDX-License-Identifier: MIT
pragma solidity 0.8.38;

/// @custom:oz-upgrades
contract VaultV1 {
    mapping(address => uint256) public balances;
    uint256 public assets;

    function deposit(uint128 amount) external {
        balances[msg.sender] += amount;
        assets += amount;
    }

    function withdraw(uint128 amount) external virtual returns (uint256 paid) {
        require(amount <= balances[msg.sender]);
        balances[msg.sender] -= amount;
        assets -= amount;
        return amount;
    }
}

/// @custom:oz-upgrades
contract VaultV2 is VaultV1 {
    uint256 public fee;
}

contract BugVault is VaultV1 {
    function withdraw(uint128 amount) external override returns (uint256 paid) {
        require(amount <= balances[msg.sender]);
        balances[msg.sender] -= amount;
        // Bug: the payout is one unit larger than the recorded debit.
        paid = uint256(amount) + 1;
        assets -= paid;
    }
}

contract PassVault {
    function check_solvency(uint128 depositAmount, uint128 withdrawal) public {
        VaultV1 vault = new VaultV1();
        vault.deposit(depositAmount);
        require(withdrawal <= depositAmount);
        uint256 paid = vault.withdraw(withdrawal);
        assert(paid <= withdrawal);
        assert(vault.assets() == vault.balances(address(this)));
    }
}

contract FailVault {
    function check_solvency(uint128 depositAmount, uint128 withdrawal) public {
        BugVault vault = new BugVault();
        vault.deposit(depositAmount);
        require(withdrawal < depositAmount);
        uint256 paid = vault.withdraw(withdrawal);
        assert(paid <= withdrawal);
        assert(vault.assets() == vault.balances(address(this)));
    }
}

contract TimeoutVault {
    // Additional solvency precondition deliberately hard for a 1 ms solver.
    function check_solvency(uint128 x, uint128 y) public {
        VaultV1 vault = new VaultV1();
        vault.deposit(y);
        assert(vault.assets() == vault.balances(address(this)));
        unchecked {
            uint256 product = uint256(x) * uint256(y);
            assert(product / (uint256(x) | 1) <= uint256(y));
        }
    }
}

