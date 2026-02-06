// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "utils/Test.sol";

contract GetStylusInitCodeTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function testGetStylusInitCode() public {
        bytes memory initCode = vm.getStylusInitCode("fixtures/Stylus/foundry_stylus_program.wasm");

        // Init code should be non-empty
        assertTrue(initCode.length > 0);

        // Init code should NOT start with the Stylus discriminant directly
        // (it should be wrapped in EVM init code)
        bytes4 first4;
        assembly {
            first4 := mload(add(initCode, 32))
        }
        // The init code starts with 0x6080... (EVM bytecode header)
        assertEq(first4, hex"60806040");
    }

    function testGetStylusInitCodeDeploysCorrectRuntime() public {
        bytes memory initCode = vm.getStylusInitCode("fixtures/Stylus/foundry_stylus_program.wasm");
        bytes memory stylusCode = vm.getStylusCode("fixtures/Stylus/foundry_stylus_program.wasm");

        // Deploy using CREATE with the init code
        address deployed;
        assembly {
            deployed := create(0, add(initCode, 32), mload(initCode))
        }
        assertTrue(deployed != address(0), "CREATE should succeed");

        // The deployed runtime code should match getStylusCode output
        bytes memory deployedCode = deployed.code;
        assertEq(keccak256(deployedCode), keccak256(stylusCode), "Runtime code should match getStylusCode");
    }
}
